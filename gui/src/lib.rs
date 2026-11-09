//! The visual editor for `polaroid` pipelines.
//!
//! The pipeline is drawn as a graph; the picked node's args are a form built
//! from its block's JSON Schema, expressions included. Every change re-checks
//! the whole pipeline off the UI thread, so each node shows its columns or
//! what is wrong with it. A preview runs the pipeline up to the picked node in
//! a child process of this same program (its `run` command), which a stop
//! simply kills, so a long or failing run never takes the editor with it.

mod form;
mod graph;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use iced::widget::{
    Column,
    button,
    center,
    checkbox,
    column,
    container,
    pick_list,
    row,
    rule,
    scrollable,
    space,
    table,
    text,
    text_editor,
    text_input,
};
use iced::{Center, Element, Fill, Font, Size, Task, Theme, task};
use polaroid_core::polars::prelude::{DataFrame, Expr, ParquetReader, SerReader};
use polaroid_core::{Node, Pipeline, Registry};
use serde_json::{Value, json};

/// How many rows a preview shows.
const PREVIEW_ROWS: u32 = 100;

/// Opens the editor on the pipeline at `path`, which need not exist yet.
pub fn run(
    registry: Registry,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(path);
    let mut pipeline = match path.exists() {
        true => Pipeline::load(&path.to_string_lossy())?,
        false => Pipeline {
            polars: polaroid_core::polars::VERSION.to_owned(),
            nodes: vec![],
            outputs: vec![],
        },
    };
    graph::layout(&mut pipeline);
    let registry = Arc::new(registry);

    iced::application(
        move || {
            let mut app = App::new(registry.clone(), path.clone(), pipeline.clone());
            let check = app.check();
            (app, check)
        },
        App::update,
        App::view,
    )
    .title(App::title)
    .window_size(Size::new(1500.0, 950.0))
    .run()?;
    Ok(())
}

/// Every node's columns or error, or what is wrong with the whole pipeline.
type Checks = Result<BTreeMap<String, Result<Vec<String>, String>>, String>;

struct App {
    registry: Arc<Registry>,
    catalog: Value,
    path: PathBuf,
    pipeline: Pipeline,
    saved: bool,
    selected: Option<usize>,
    /// The chip picked in the args form, as a JSON pointer.
    focus: Option<String>,
    /// The args as JSON text, while editing them so.
    json: Option<text_editor::Content>,
    checks: Checks,
    /// The latest check, so that a stale one is dropped.
    checking: u64,
    /// The preview running, which is stopped when this is dropped.
    running: Option<task::Handle>,
    preview: Option<Result<(String, DataFrame), String>>,
}

#[derive(Debug, Clone)]
enum Message {
    Graph(graph::Event),
    Form(form::Message),
    Json(text_editor::Action),
    ToggleJson,
    Rename(String),
    Input(usize, String),
    Output(bool),
    Add(String),
    Delete,
    Save,
    Checked(u64, Checks),
    Preview,
    Stop,
    Previewed(Result<(String, DataFrame), String>),
}

impl App {
    fn new(
        registry: Arc<Registry>,
        path: PathBuf,
        pipeline: Pipeline,
    ) -> Self {
        Self {
            catalog: registry.catalog(),
            registry,
            path,
            pipeline,
            saved: true,
            selected: None,
            focus: None,
            json: None,
            checks: Err("checking…".to_owned()),
            checking: 0,
            running: None,
            preview: None,
        }
    }

    fn title(&self) -> String {
        let unsaved = if self.saved { "" } else { " •" };
        format!("{}{unsaved} — polaroid", self.path.display())
    }

    fn update(
        &mut self,
        message: Message,
    ) -> Task<Message> {
        match message {
            Message::Graph(graph::Event::Select(index)) => {
                self.selected = Some(index);
                self.focus = None;
                self.json = None;
                Task::none()
            }
            Message::Graph(graph::Event::Move(index, at)) => {
                graph::place(&mut self.pipeline.nodes[index], at);
                self.saved = false;
                Task::none()
            }
            Message::Graph(graph::Event::Connect { from, to, slot }) => {
                let id = self.pipeline.nodes[from]
                    .id
                    .clone();
                self.wire(to, slot, id)
            }
            Message::Graph(graph::Event::Disconnect { to, slot }) => {
                if let Some(input) = self.pipeline.nodes[to]
                    .inputs
                    .get_mut(slot)
                {
                    input.clear();
                }
                self.changed()
            }
            Message::Form(form::Message::Focus(focus)) => {
                self.focus = focus;
                Task::none()
            }
            Message::Form(form::Message::Edit(pointer, value)) => {
                let Some(node) = self.node_mut() else {
                    return Task::none();
                };
                form::apply(&mut node.args, &pointer, value);
                self.changed()
            }
            Message::ToggleJson => {
                self.json = match (&self.json, self.node()) {
                    (None, Some(node)) => Some(text_editor::Content::with_text(
                        &serde_json::to_string_pretty(&node.args).expect("JSON values serialize"),
                    )),
                    _ => None,
                };
                Task::none()
            }
            Message::Json(action) => {
                let Some(content) = &mut self.json else {
                    return Task::none();
                };
                let edit = action.is_edit();
                content.perform(action);
                // the args change once the text is JSON again
                let parsed = serde_json::from_str(&content.text());
                match (edit, parsed, self.selected) {
                    (true, Ok(args), Some(index)) => {
                        self.pipeline.nodes[index].args = args;
                        self.changed()
                    }
                    _ => Task::none(),
                }
            }
            Message::Rename(id) => {
                let Some(index) = self.selected else {
                    return Task::none();
                };
                let old = std::mem::replace(&mut self.pipeline.nodes[index].id, id.clone());
                let readers = self
                    .pipeline
                    .nodes
                    .iter_mut()
                    .flat_map(|node| {
                        node.inputs
                            .iter_mut()
                    });
                for name in readers.chain(
                    &mut self
                        .pipeline
                        .outputs,
                ) {
                    if *name == old {
                        name.clone_from(&id);
                    }
                }
                self.changed()
            }
            Message::Input(slot, id) => {
                let Some(index) = self.selected else {
                    return Task::none();
                };
                self.wire(index, slot, id)
            }
            Message::Output(on) => {
                let Some(id) = self
                    .node()
                    .map(|node| node.id.clone())
                else {
                    return Task::none();
                };
                self.pipeline
                    .outputs
                    .retain(|output| *output != id);
                if on {
                    self.pipeline
                        .outputs
                        .push(id);
                }
                self.changed()
            }
            Message::Add(block) => {
                let inputs = self
                    .roles(&block)
                    .len();
                let id = (1..)
                    .map(|n| format!("{block}_{n}"))
                    .find(|id| {
                        !self
                            .pipeline
                            .nodes
                            .iter()
                            .any(|node| node.id == *id)
                    })
                    .expect("some number is free");
                let args = form::default(
                    &self.catalog["blocks"][&block]["args"],
                    &self.catalog["$defs"],
                );
                let mut node = Node {
                    id,
                    block,
                    inputs: vec![String::new(); inputs],
                    args,
                    ui: Value::Null,
                };
                // beside the picked node, or at the top
                let at = self
                    .node()
                    .and_then(graph::position)
                    .map_or(iced::Point::ORIGIN, |at| at + iced::Vector::new(260.0, 0.0));
                graph::place(&mut node, at);
                self.pipeline
                    .nodes
                    .push(node);
                self.selected = Some(
                    self.pipeline
                        .nodes
                        .len()
                        - 1,
                );
                self.focus = None;
                self.json = None;
                self.changed()
            }
            Message::Delete => {
                let Some(index) = self.selected.take() else {
                    return Task::none();
                };
                let gone = self
                    .pipeline
                    .nodes
                    .remove(index);
                for input in self
                    .pipeline
                    .nodes
                    .iter_mut()
                    .flat_map(|node| {
                        node.inputs
                            .iter_mut()
                    })
                {
                    if *input == gone.id {
                        input.clear();
                    }
                }
                self.pipeline
                    .outputs
                    .retain(|output| *output != gone.id);
                self.json = None;
                self.changed()
            }
            Message::Save => {
                let text =
                    serde_json::to_string_pretty(&self.pipeline).expect("a pipeline serializes");
                match std::fs::write(&self.path, text + "\n") {
                    Ok(()) => self.saved = true,
                    Err(e) => self.checks = Err(format!("saving {}: {e}", self.path.display())),
                }
                Task::none()
            }
            Message::Checked(checking, checks) => {
                if checking == self.checking {
                    self.checks = checks;
                }
                Task::none()
            }
            Message::Preview => {
                let Some(node) = self.node() else {
                    return Task::none();
                };
                let id = node.id.clone();
                let (task, handle) = Task::perform(
                    preview(
                        self.pipeline
                            .clone(),
                        id,
                    ),
                    Message::Previewed,
                )
                .abortable();
                self.running = Some(handle.abort_on_drop());
                self.preview = None;
                task
            }
            Message::Stop => {
                self.running = None;
                Task::none()
            }
            Message::Previewed(preview) => {
                self.running = None;
                self.preview = Some(preview);
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let blocks: Vec<String> = self.catalog["blocks"]
            .as_object()
            .map(|blocks| {
                blocks
                    .keys()
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let toolbar = row![
            text(
                self.path
                    .display()
                    .to_string()
            )
            .size(14),
            space::horizontal(),
            pick_list(blocks, None::<String>, Message::Add).placeholder("+ add block"),
            button("Save").on_press_maybe((!self.saved).then_some(Message::Save)),
        ]
        .spacing(10)
        .padding(8)
        .align_y(Center);

        let cards = self
            .pipeline
            .nodes
            .iter()
            .map(|node| graph::Card {
                id: &node.id,
                block: &node.block,
                at: graph::position(node).unwrap_or(iced::Point::ORIGIN),
                roles: self.roles(&node.block),
                sources: node
                    .inputs
                    .iter()
                    .map(|input| {
                        self.pipeline
                            .nodes
                            .iter()
                            .position(|n| n.id == *input)
                    })
                    .collect(),
                failed: self
                    .status(&node.id)
                    .is_some_and(|check| check.is_err()),
                output: self
                    .pipeline
                    .outputs
                    .contains(&node.id),
            })
            .collect();
        let graph = graph::view(graph::Graph {
            cards,
            selected: self.selected,
        })
        .map(Message::Graph);

        let inspector = container(
            scrollable(
                self.inspector()
                    .padding(12),
            )
            .height(Fill),
        )
        .width(480)
        .height(Fill);

        column![
            toolbar,
            rule::horizontal(1),
            row![
                graph,
                rule::vertical(1),
                inspector
            ]
            .height(iced::FillPortion(3)),
            rule::horizontal(1),
            container(self.preview()).height(iced::FillPortion(2)),
        ]
        .into()
    }

    /// The picked node: its name, inputs, args and columns.
    fn inspector(&self) -> Column<'_, Message> {
        let problem = match &self.checks {
            Err(e) => Some(error(e.clone())),
            Ok(_) => None,
        };
        let Some(node) = self.node() else {
            return column![
                problem.unwrap_or_else(|| muted("Pick a node, or add a block.").into()),
                muted("Drag a node to move it, the background to pan, an output to an input to wire them; scroll to zoom, right-click an input to unwire it."),
            ]
            .spacing(12);
        };
        let index = self
            .selected
            .expect("a node is picked");
        let schema = self.resolve(&self.catalog["blocks"][&node.block]["args"]);

        let others: Vec<String> = self
            .pipeline
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != index)
            .map(|(_, n)| n.id.clone())
            .collect();
        let inputs = self
            .roles(&node.block)
            .into_iter()
            .enumerate()
            .map(|(slot, role)| {
                let current = node
                    .inputs
                    .get(slot)
                    .filter(|input| !input.is_empty())
                    .cloned();
                row![
                    muted(role).width(110),
                    pick_list(others.clone(), current, move |id| Message::Input(slot, id))
                        .placeholder("unwired")
                        .text_size(13),
                ]
                .spacing(6)
                .align_y(Center)
                .into()
            });

        let args: Element<'_, Message> = match &self.json {
            Some(content) => {
                let valid = serde_json::from_str::<Value>(&content.text()).is_ok();
                column![
                    text_editor(content)
                        .on_action(Message::Json)
                        .font(Font::MONOSPACE)
                        .size(12),
                ]
                .push((!valid).then(|| {
                    error("not JSON yet: the args keep their last valid value".to_owned())
                }))
                .spacing(6)
                .into()
            }
            None => form::view(
                &node.args,
                schema,
                &self.catalog["$defs"],
                self.focus
                    .as_deref(),
            )
            .map(Message::Form),
        };

        let columns: Element<'_, Message> = match self.status(&node.id) {
            None => muted("checking…").into(),
            Some(Err(e)) => error(e.clone()),
            Some(Ok(columns)) => Column::with_children(
                columns
                    .iter()
                    .map(|column| {
                        text(column)
                            .size(12)
                            .font(Font::MONOSPACE)
                            .into()
                    }),
            )
            .into(),
        };

        let preview = match &self.running {
            Some(_) => button("Stop").on_press(Message::Stop),
            None => button("Preview").on_press(Message::Preview),
        };

        column![
            row![
                text_input("id", &node.id)
                    .on_input(Message::Rename)
                    .size(16),
                preview,
            ]
            .spacing(8),
            muted(node.block.clone()),
        ]
        .push(
            schema
                .get("description")
                .and_then(Value::as_str)
                .map(|description| text(description).size(13)),
        )
        .push(problem)
        .extend(inputs)
        .push(
            checkbox(
                self.pipeline
                    .outputs
                    .contains(&node.id),
            )
            .label("output of a run")
            .on_toggle(Message::Output),
        )
        .push(rule::horizontal(1))
        .push(
            row![
                text("args").size(14),
                space::horizontal(),
                button(text(if self.json.is_some() { "form" } else { "json" }).size(12))
                    .style(button::text)
                    .on_press(Message::ToggleJson),
            ]
            .align_y(Center),
        )
        .push(args)
        .push(code(&node.args))
        .push(rule::horizontal(1))
        .push(text("columns").size(14))
        .push(columns)
        .push(rule::horizontal(1))
        .push(
            button(text("Delete node").size(12))
                .style(button::danger)
                .on_press(Message::Delete),
        )
        .spacing(10)
    }

    /// The last preview: a table of its first rows, or why there is none.
    fn preview(&self) -> Element<'_, Message> {
        match (&self.running, &self.preview) {
            (Some(_), _) => center(muted("running…")).into(),
            (None, None) => center(muted("Preview a node to see its first rows.")).into(),
            (None, Some(Err(e))) => scrollable(container(error(e.clone())).padding(12)).into(),
            (None, Some(Ok((id, frame)))) => {
                let columns = frame
                    .columns()
                    .iter()
                    .map(|column| {
                        table::column(
                            text(
                                column
                                    .name()
                                    .to_string(),
                            )
                            .size(13)
                            .font(Font::MONOSPACE),
                            move |row: usize| {
                                let cell = column
                                    .get(row)
                                    .map(|value| value.to_string())
                                    .unwrap_or_default();
                                let cell: String = cell
                                    .chars()
                                    .take(60)
                                    .collect();
                                text(cell)
                                    .size(12)
                                    .font(Font::MONOSPACE)
                            },
                        )
                    });
                column![
                    muted(format!("{id}: the first {} rows", frame.height())),
                    scrollable(table(columns, 0..frame.height()))
                        .direction(scrollable::Direction::Both {
                            vertical: scrollable::Scrollbar::default(),
                            horizontal: scrollable::Scrollbar::default(),
                        })
                        .width(Fill)
                        .height(Fill),
                ]
                .spacing(6)
                .padding(8)
                .into()
            }
        }
    }

    fn node(&self) -> Option<&Node> {
        self.pipeline
            .nodes
            .get(self.selected?)
    }

    fn node_mut(&mut self) -> Option<&mut Node> {
        self.pipeline
            .nodes
            .get_mut(self.selected?)
    }

    /// A block's input roles.
    fn roles(
        &self,
        block: &str,
    ) -> Vec<&str> {
        self.catalog["blocks"][block]["inputs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect()
    }

    fn resolve<'a>(
        &'a self,
        schema: &'a Value,
    ) -> &'a Value {
        match schema
            .get("$ref")
            .and_then(Value::as_str)
        {
            Some(path) => &self.catalog["$defs"][path.trim_start_matches("#/$defs/")],
            None => schema,
        }
    }

    fn status(
        &self,
        id: &str,
    ) -> Option<&Result<Vec<String>, String>> {
        self.checks
            .as_ref()
            .ok()?
            .get(id)
    }

    /// Node `index` reads node `id` as its input `slot`.
    fn wire(
        &mut self,
        index: usize,
        slot: usize,
        id: String,
    ) -> Task<Message> {
        let inputs = &mut self.pipeline.nodes[index].inputs;
        if inputs.len() <= slot {
            inputs.resize(slot + 1, String::new());
        }
        inputs[slot] = id;
        self.changed()
    }

    /// Marks the pipeline changed and checks it again, off the UI thread.
    fn changed(&mut self) -> Task<Message> {
        self.saved = false;
        self.check()
    }

    fn check(&mut self) -> Task<Message> {
        self.checking += 1;
        let checking = self.checking;
        let (registry, pipeline) = (
            self.registry
                .clone(),
            self.pipeline
                .clone(),
        );
        let check = async move {
            tokio::task::spawn_blocking(move || check(&registry, &pipeline))
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
        };
        Task::perform(check, move |checks| Message::Checked(checking, checks))
    }
}

fn check(
    registry: &Registry,
    pipeline: &Pipeline,
) -> Checks {
    let schemas = pipeline
        .schemas(registry)
        .map_err(|e| e.to_string())?;
    Ok(schemas
        .into_iter()
        .map(|(id, schema)| {
            let columns = schema
                .map(|schema| {
                    schema
                        .iter()
                        .map(|(name, dtype)| format!("{name}: {dtype}"))
                        .collect()
                })
                .map_err(|e| e.to_string());
            (id.to_owned(), columns)
        })
        .collect())
}

/// The first rows of node `id`, run by this program's `run` command in a
/// child process, which is killed if the preview is dropped.
async fn preview(
    mut pipeline: Pipeline,
    id: String,
) -> Result<(String, DataFrame), String> {
    let fail = |e: &dyn std::fmt::Display| e.to_string();
    let dir = std::env::temp_dir().join(format!("polaroid-preview-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| fail(&e))?;

    // only as many rows as are shown, so the run can stop early
    let limited = format!("{id} (preview)");
    pipeline
        .nodes
        .push(Node {
            id: limited.clone(),
            block: "limit".to_owned(),
            inputs: vec![id.clone()],
            args: json!({"n": PREVIEW_ROWS}),
            ui: Value::Null,
        });
    pipeline.outputs = vec![limited.clone()];
    let file = dir.join("pipeline.json");
    let text = serde_json::to_string(&pipeline).map_err(|e| fail(&e))?;
    std::fs::write(&file, text).map_err(|e| fail(&e))?;

    let program = std::env::current_exe().map_err(|e| fail(&e))?;
    let output = tokio::process::Command::new(program)
        .arg("run")
        .arg(&file)
        .arg(&dir)
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| fail(&e))?;
    if !output
        .status
        .success()
    {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }

    let parquet =
        std::fs::File::open(dir.join(format!("{limited}.parquet"))).map_err(|e| fail(&e))?;
    let frame = ParquetReader::new(parquet)
        .finish()
        .map_err(|e| fail(&e))?;
    Ok((id, frame))
}

/// The args' expressions as Polars prints them: each top-level field, or
/// element of one, that reads as an expression.
fn code(args: &Value) -> Element<'_, Message> {
    let as_expr = |value: &Value| serde_json::from_value::<Expr>(value.clone()).ok();
    let mut lines = vec![];
    for (name, value) in args
        .as_object()
        .into_iter()
        .flatten()
    {
        // a bare string reads as an expression too, but is not one here
        if value.is_string() {
            continue;
        }
        match (as_expr(value), value.as_array()) {
            (Some(expr), _) => lines.push(format!("{name}: {expr}")),
            (None, Some(elements)) => {
                for (i, element) in elements
                    .iter()
                    .enumerate()
                {
                    if let Some(expr) = as_expr(element) {
                        lines.push(format!("{name}[{i}]: {expr}"));
                    }
                }
            }
            _ => {}
        }
    }
    match lines.is_empty() {
        true => space().into(),
        false => container(
            text(lines.join("\n"))
                .size(12)
                .font(Font::MONOSPACE),
        )
        .padding(8)
        .width(Fill)
        .style(container::rounded_box)
        .into(),
    }
}

fn muted<'a>(content: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(content)
        .size(13)
        .style(|theme: &Theme| text::Style {
            color: Some(
                theme
                    .extended_palette()
                    .background
                    .strongest
                    .color,
            ),
        })
}

fn error<'a>(message: String) -> Element<'a, Message> {
    text(message)
        .size(12)
        .font(Font::MONOSPACE)
        .style(text::danger)
        .into()
}

#[cfg(test)]
mod tests {
    use iced_test::Simulator;
    use polaroid_core::polars::prelude::ParquetWriter;

    use super::*;

    /// The app as drawn in a window tall enough to show the whole inspector.
    fn window(app: &App) -> Simulator<'_, Message> {
        Simulator::with_size(
            iced::Settings::default(),
            Size::new(1500.0, 1400.0),
            app.view(),
        )
    }

    /// A pipeline over a small parquet file, its `loud` node picked.
    fn app() -> App {
        let path = std::env::temp_dir().join("polaroid-gui-test.parquet");
        let mut frame = polaroid_core::polars::df!(
            "vehicle" => ["a", "a", "b"],
            "peak" => [1.0, 3.0, 1.5],
        )
        .unwrap();
        ParquetWriter::new(std::fs::File::create(&path).unwrap())
            .finish(&mut frame)
            .unwrap();
        let mut pipeline: Pipeline = serde_json::from_value(json!({
            "polars": polaroid_core::polars::VERSION,
            "outputs": ["loud"],
            "nodes": [
                {"id": "raw", "block": "scan_parquet", "args": {"path": path, "columns": [["vehicle", "String"], ["peak", "Float64"]]}},
                {"id": "loud", "block": "filter", "inputs": ["raw"], "args": {"predicates": [
                    {"BinaryExpr": {"left": {"Column": "peak"}, "op": "Gt", "right": {"Literal": {"Dyn": {"Float": 2.0}}}}}
                ]}},
            ],
        }))
        .unwrap();
        graph::layout(&mut pipeline);

        let mut app = App::new(
            Arc::new(Registry::with_polars()),
            "test.json".into(),
            pipeline,
        );
        app.checks = check(&app.registry, &app.pipeline);
        app.selected = Some(1);
        app
    }

    #[test]
    fn a_picked_node_shows_its_expressions_as_a_tree_and_its_columns() {
        let app = app();
        let mut ui = window(&app);
        for shown in [
            "BinaryExpr",
            "Column",
            "Literal",
            "vehicle: str",
            r#"predicates[0]: [(col("peak")) > (dyn float: 2)]"#,
        ] {
            assert!(
                ui.find(shown)
                    .is_ok(),
                "{shown} is not shown"
            );
        }
    }

    #[test]
    fn a_picked_chip_offers_to_clear_it() {
        let mut app = app();
        let mut ui = window(&app);
        let _ = ui
            .click("BinaryExpr")
            .unwrap();
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert_eq!(app.focus.as_deref(), Some("/predicates/0"));

        let mut ui = window(&app);
        let _ = ui
            .click("clear")
            .unwrap();
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert_eq!(app.pipeline.nodes[1].args, json!({"predicates": [null]}));
    }
}
