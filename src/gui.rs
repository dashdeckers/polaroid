//! The visual editor: the pipeline as a graph, and a form for the selected node.

mod child;
mod form;
mod graph;
mod inspector;
mod table;

use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::widget::{
    Text,
    button,
    canvas,
    center,
    column,
    container,
    mouse_area,
    pick_list,
    pin,
    row,
    rule,
    scrollable,
    space,
    stack,
    text,
};
use iced::{Center, Fill, Function, Point, Subscription, Task, Widget, clipboard, task};
use serde_json::Value;

use self::inspector::Edit;
use crate::polars::prelude::{DataFrame, DataType, PolarsResult, df, polars_err};
use crate::{Node, Pipeline, Registry, is_sink, polars, schema};

const HELP: &str = "\
Drag a node to move it, and the background to pan. Scroll to zoom.

Drag from the dot on a node's right to a dot on another's left to wire them. Right-click that dot \
to unwire it, and the background to add a node there.

Tick \"save output\" on a node and Run to keep its frame. A Run reads again only what was edited \
since, and a Rerun everything.

Double-click the id of a node to lay its card open, and to close it. An open card shows the saved \
frame, or else the columns that Check found. Scroll over it, with shift for sideways. Drag its \
corner to resize it, and across its cells to select them for Ctrl+C.";

/// A node's columns and their types, or its error, as the `check` command prints them.
type Columns = Result<Vec<(String, DataType)>, String>;

/// Opens the editor on the pipeline at `path`, which need not exist yet.
pub fn open(
    registry: Registry,
    path: &str,
) -> PolarsResult<()> {
    let mut pipeline = match std::fs::exists(path)? {
        true => Pipeline::load(path)?,
        false => Pipeline {
            polars: polars::VERSION.to_owned(),
            nodes: vec![],
            outputs: vec![],
        },
    };
    graph::layout(&mut pipeline);
    // the window's own chatter would drown the log
    log::set_max_level(log::LevelFilter::Warn);
    let registry = Arc::new(registry);
    let path = path.to_owned();

    iced::application(
        move || Editor::new(registry.clone(), path.clone(), pipeline.clone()),
        Editor::update,
        Editor::view,
    )
    .title(Editor::title)
    .subscription(Editor::subscription)
    .run()
    .map_err(|e| polars_err!(ComputeError: "{e}"))
}

struct Editor {
    registry: Arc<Registry>,
    catalog: Value,
    path: String,
    pipeline: Pipeline,
    is_dirty: bool,
    selected: Option<usize>,
    /// Where the menu of blocks is open: on the canvas, and in the graph.
    menu: Option<(Point, Point)>,
    /// What the form of the selected node shows beside its args.
    form: form::State,
    /// What is wrong with the wiring of each node, as of the latest edit.
    wiring: BTreeMap<String, String>,
    /// The nodes that write their frame, as to a file, which every Run does.
    writes: Vec<String>,
    known: BTreeMap<String, Known>,
    /// What is wrong with the pipeline as a whole, or with the latest Save.
    problem: Option<String>,
    /// The Check or Run under way, whose child is killed when this is dropped.
    job: Option<task::Handle>,
}

/// What a Check and the Runs found of a node, until it or one it reads is edited.
#[derive(Default)]
struct Known {
    columns: Option<Columns>,
    /// Its frame, if a Run saved it.
    frame: Option<DataFrame>,
    /// Why the latest Run failed, if it was to save this node or have it write.
    failed: Option<String>,
    /// Whether a Run computed it.
    is_run: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Graph(graph::Event),
    /// A node of this block, at this point of the graph or else beside the selected node.
    Add(String, Option<Point>),
    CloseMenu,
    Edit(Edit),
    Save,
    Check,
    Checked(Result<BTreeMap<String, Columns>, String>),
    Run,
    /// A Run of everything, whatever was saved before.
    Rerun,
    /// What a Run made of the nodes it was to save or have write.
    Ran(Vec<String>, Result<Vec<(String, DataFrame)>, String>),
    Stop,
}

impl Editor {
    fn new(
        registry: Arc<Registry>,
        path: String,
        pipeline: Pipeline,
    ) -> Self {
        let mut editor = Self {
            catalog: registry.catalog(),
            registry,
            path,
            pipeline,
            is_dirty: false,
            selected: None,
            menu: None,
            form: form::State::default(),
            wiring: BTreeMap::new(),
            writes: Vec::new(),
            known: BTreeMap::new(),
            problem: None,
            job: None,
        };
        editor.check_wiring();
        editor
    }

    fn title(&self) -> String {
        let dirty = if self.is_dirty { "*" } else { "" };
        format!("{}{dirty} - Polaroid", self.path)
    }

    /// Escape closes the menu of blocks.
    fn subscription(&self) -> Subscription<Message> {
        match self.menu {
            Some(_) => keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed {
                    key: Key::Named(Named::Escape),
                    ..
                } => Some(Message::CloseMenu),
                _ => None,
            }),
            None => Subscription::none(),
        }
    }

    fn update(
        &mut self,
        message: Message,
    ) -> Task<Message> {
        match message {
            Message::Graph(graph::Event::Select(index)) => self.select(Some(index)),
            Message::Graph(graph::Event::Move(index, position)) => {
                graph::place(&mut self.pipeline.nodes[index], position);
                self.is_dirty = true;
            }
            Message::Graph(graph::Event::Resize(index, size)) => {
                graph::resize(&mut self.pipeline.nodes[index], size);
                self.is_dirty = true;
            }
            Message::Graph(graph::Event::Toggle(index)) => {
                graph::toggle(&mut self.pipeline.nodes[index]);
                self.is_dirty = true;
            }
            Message::Graph(graph::Event::Save(index)) => {
                let is_saved = self
                    .pipeline
                    .outputs
                    .contains(&self.pipeline.nodes[index].id);
                // as if selected and ticked in its panel
                self.select(Some(index));
                return self.update(Message::Edit(Edit::Output(!is_saved)));
            }
            Message::Graph(graph::Event::Copy(text)) => return clipboard::write(text).discard(),
            Message::Graph(graph::Event::Wire { to, slot, from }) => {
                let source = from.map_or(String::new(), |from| {
                    self.pipeline.nodes[from]
                        .id
                        .clone()
                });
                // as if selected and wired in its panel
                self.select(Some(to));
                return self.update(Message::Edit(Edit::Wire(slot, source)));
            }
            Message::Graph(graph::Event::Menu { at, point }) => self.menu = Some((at, point)),
            Message::CloseMenu => self.menu = None,
            Message::Add(block, point) => {
                self.menu = None;
                let nodes = &mut self.pipeline.nodes;
                let entry = &self.catalog["blocks"][&block];
                let id = (1..)
                    .map(|n| format!("{block}_{n}"))
                    .find(|id| {
                        nodes
                            .iter()
                            .all(|node| node.id != *id)
                    })
                    .expect("some number is free");
                let inputs = entry["inputs"]
                    .as_array()
                    .map_or(0, Vec::len);
                let mut node = Node {
                    id,
                    block,
                    inputs: vec![String::new(); inputs],
                    args: schema::default(&self.catalog["$defs"], &entry["args"]),
                    ui: Value::Null,
                };
                let position = match (point, self.selected) {
                    (Some(point), _) => point,
                    (None, Some(index)) => graph::beside(&nodes[index]),
                    (None, None) => Point::new(20.0, 20.0),
                };
                graph::place(&mut node, position);
                nodes.push(node);
                let added = nodes.len() - 1;
                self.select(Some(added));
                self.is_dirty = true;
                self.check_wiring();
            }
            Message::Edit(edit) => inspector::edit(self, edit),
            Message::Save => {
                let pipeline =
                    serde_json::to_string_pretty(&self.pipeline).expect("a pipeline serializes");
                self.problem = std::fs::write(&self.path, pipeline + "\n")
                    .err()
                    .map(|e| format!("{}: {e}", self.path));
                self.is_dirty = self
                    .problem
                    .is_some();
            }
            Message::Check => {
                let check = child::check(
                    self.pipeline
                        .clone(),
                );
                return self.start(Task::perform(check, Message::Checked));
            }
            Message::Checked(checks) => {
                self.job = None;
                match checks {
                    Ok(checks) => {
                        for (id, columns) in checks {
                            self.known_mut(id)
                                .columns = Some(columns);
                        }
                    }
                    Err(problem) => self.problem = Some(problem),
                }
            }
            Message::Rerun => {
                for known in self
                    .known
                    .values_mut()
                {
                    known.frame = None;
                    known.is_run = false;
                }
                return self.update(Message::Run);
            }
            Message::Run => {
                let mut pipeline = self
                    .pipeline
                    .clone();
                // a frame that is saved is not collected again
                pipeline
                    .outputs
                    .retain(|id| {
                        self.frame(id)
                            .is_none()
                    });
                // but every write is done again, as its file may be gone
                let targets: Vec<String> = pipeline
                    .outputs
                    .iter()
                    .chain(&self.writes)
                    .cloned()
                    .collect();
                // a read frame goes along, but not a write's, which it would stand in for
                let is_read = |id: &String| {
                    pipeline
                        .nodes
                        .iter()
                        .any(|node| {
                            node.inputs
                                .contains(id)
                        })
                };
                let known = self
                    .known
                    .iter()
                    .filter(|(id, _)| {
                        is_read(id)
                            && !self
                                .writes
                                .contains(id)
                    })
                    .filter_map(|(id, known)| {
                        Some((
                            id.clone(),
                            known
                                .frame
                                .clone()?,
                        ))
                    })
                    .collect();

                for known in self
                    .known
                    .values_mut()
                {
                    known.failed = None;
                }
                let run = child::run(pipeline, known);
                return self.start(Task::perform(run, Message::Ran.with(targets)));
            }
            Message::Ran(targets, Ok(frames)) => {
                self.job = None;
                for id in self.sources(&targets) {
                    self.known_mut(id)
                        .is_run = true;
                }
                for (id, mut frame) in frames {
                    // in one chunk, a cell is found in constant time
                    frame.rechunk_mut();
                    self.known_mut(id)
                        .frame = Some(frame);
                }
            }
            Message::Ran(targets, Err(e)) => {
                self.job = None;
                // Polars does not say which node failed
                for id in targets {
                    self.known_mut(id)
                        .failed = Some(e.clone());
                }
            }
            Message::Stop => self.job = None,
        }

        Task::none()
    }

    /// Has `job` run as the Check or Run under way, which Stop ends.
    fn start(
        &mut self,
        job: Task<Message>,
    ) -> Task<Message> {
        let (job, handle) = job.abortable();
        self.job = Some(handle.abort_on_drop());
        job
    }

    /// Selects a node, or none, and forgets what the form showed of another.
    fn select(
        &mut self,
        selected: Option<usize>,
    ) {
        if self.selected != selected {
            self.form = form::State::default();
        }
        self.selected = selected;
    }

    /// Notes what is wrong with the wiring of each node, and which nodes write.
    fn check_wiring(&mut self) {
        self.wiring.clear();
        self.writes.clear();
        self.problem = None;
        let frames = match self
            .pipeline
            .frames(&self.registry)
        {
            Ok(frames) => frames,
            Err(e) => return self.problem = Some(e.to_string()),
        };
        for (id, frame) in frames {
            match frame {
                Ok(frame) if is_sink(&frame) => self
                    .writes
                    .push(id.to_owned()),
                Ok(_) => {}
                Err(e) => drop(
                    self.wiring
                        .insert(id.to_owned(), e.to_string()),
                ),
            }
        }
    }

    /// Forgets what is known of node `index` and of every node that reads it, and stops a job.
    fn forget(
        &mut self,
        index: usize,
    ) {
        let nodes = &self.pipeline.nodes;
        let mut stale = HashSet::from([nodes[index]
            .id
            .as_str()]);
        // as many passes as there are nodes reach every reader
        for _ in nodes {
            for node in nodes {
                let is_reader = node
                    .inputs
                    .iter()
                    .any(|input| stale.contains(input.as_str()));
                if is_reader {
                    stale.insert(&node.id);
                }
            }
        }

        self.known
            .retain(|id, _| !stale.contains(id.as_str()));
        self.job = None;
    }

    /// The nodes `ids` and every node that they read, however indirectly.
    fn sources(
        &self,
        ids: &[String],
    ) -> HashSet<String> {
        let nodes = &self.pipeline.nodes;
        let mut sources: HashSet<String> = ids
            .iter()
            .cloned()
            .collect();
        // as many passes as there are nodes reach every source
        for _ in nodes {
            for node in nodes {
                if sources.contains(&node.id) {
                    sources.extend(
                        node.inputs
                            .iter()
                            .cloned(),
                    );
                }
            }
        }
        sources
    }

    /// What is known of node `id`, to add to.
    fn known_mut(
        &mut self,
        id: String,
    ) -> &mut Known {
        self.known
            .entry(id)
            .or_default()
    }

    /// The frame of node `id`, if a Run saved it.
    fn frame(
        &self,
        id: &str,
    ) -> Option<&DataFrame> {
        self.known
            .get(id)?
            .frame
            .as_ref()
    }

    /// The columns of node `id`, or what is wrong with its wiring or its columns.
    fn found(
        &self,
        id: &str,
    ) -> Option<Result<&Vec<(String, DataType)>, &String>> {
        match self.wiring.get(id) {
            Some(e) => Some(Err(e)),
            None => self
                .known
                .get(id)?
                .columns
                .as_ref()
                .map(Result::as_ref),
        }
    }

    /// What colours the card of node `id`: a Run before a Check.
    fn status(
        &self,
        id: &str,
    ) -> graph::Status {
        let known = self.known.get(id);
        let has_failed = known.is_some_and(|known| {
            known
                .failed
                .is_some()
        });
        let is_run = known.is_some_and(|known| known.is_run);
        match self.found(id) {
            _ if has_failed => graph::Status::Failed,
            Some(Err(_)) => graph::Status::Rejected,
            _ if is_run => graph::Status::Ran,
            Some(Ok(_)) => graph::Status::Checked,
            None => graph::Status::Unknown,
        }
    }

    /// The pipeline as a graph, with what is known of each node.
    fn graph(&self) -> graph::Graph<'_> {
        let nodes = &self.pipeline.nodes;
        // an open card shows the saved frame, or else the columns
        let table = |node: &Node| match (self.frame(&node.id), self.found(&node.id)) {
            (Some(frame), _) => Some(Cow::Borrowed(frame)),
            (None, Some(Ok(columns))) if graph::is_open(node) => Some(Cow::Owned(listed(columns))),
            _ => None,
        };
        graph::Graph {
            pipeline: &self.pipeline,
            blocks: &self.catalog["blocks"],
            selected: self.selected,
            status: nodes
                .iter()
                .map(|node| self.status(&node.id))
                .collect(),
            tables: nodes
                .iter()
                .map(table)
                .collect(),
            saved: nodes
                .iter()
                .map(|node| {
                    self.frame(&node.id)
                        .is_some()
                })
                .collect(),
        }
    }

    fn view(&self) -> impl Widget<Message> {
        let blocks: Vec<&String> = self.catalog["blocks"]
            .as_object()
            .into_iter()
            .flat_map(|blocks| blocks.keys())
            .collect();
        let jobs = match self.job {
            Some(_) => row![
                "Working…",
                button("Stop").on_press(Message::Stop)
            ],
            None => row![
                button("Check").on_press(Message::Check),
                button("Run").on_press(Message::Run),
                button("Rerun").on_press(Message::Rerun),
            ],
        };
        let save = self
            .is_dirty
            .then_some(Message::Save);
        let toolbar = row![
            pick_list(None::<&String>, blocks.clone(), ToString::to_string)
                .on_select(|block| Message::Add(block.clone(), None))
                .placeholder("Add a block…"),
            space::horizontal(),
            jobs.spacing(10)
                .align_y(Center),
            button("Save").on_press_maybe(save),
        ]
        .spacing(10)
        .padding(10);

        let graph = canvas(self.graph())
            .width(Fill)
            .height(Fill)
            .map(Message::Graph);
        let menu = self
            .menu
            .map(|(at, point)| {
                let blocks = blocks
                    .iter()
                    .map(|block| {
                        button(text(block.as_str()))
                            .style(button::text)
                            .width(Fill)
                            .on_press(Message::Add((*block).clone(), Some(point)))
                    });
                let list = container(scrollable(column(blocks)))
                    .width(200)
                    .height(320)
                    .style(container::bordered_box);
                // over the graph, so that a press beside the menu closes it
                let beside = container("")
                    .width(Fill)
                    .height(Fill);
                stack![
                    mouse_area(beside)
                        .on_press(Message::CloseMenu)
                        .on_right_press(Message::CloseMenu),
                    pin(list).position(at),
                ]
            });

        let node = self
            .selected
            .map(|index| &self.pipeline.nodes[index]);
        let inspector = match node {
            Some(node) => scrollable(inspector::view(self, node).map(Message::Edit))
                // the scrollbar beside the form, not over it
                .spacing(0)
                .boxed(),
            None => center(HELP)
                .padding(20)
                .boxed(),
        };
        let problem = self
            .problem
            .as_deref()
            .map(|problem| container(error(problem)).padding(10));

        column![
            toolbar,
            rule::horizontal(1),
            row![
                stack![graph, menu],
                rule::vertical(1),
                container(inspector).width(450)
            ],
            problem,
        ]
    }
}

/// The columns of a node and their types, as a table.
fn listed(columns: &[(String, DataType)]) -> DataFrame {
    let (names, types): (Vec<&str>, Vec<String>) = columns
        .iter()
        .map(|(name, dtype)| (name.as_str(), dtype.to_string()))
        .unzip();
    df!("column" => names, "type" => types).expect("two columns of one length")
}

/// An error, which can be selected to copy it.
fn error(message: &str) -> Text<'_> {
    text(message)
        .style(text::danger)
        .selectable(true)
}

#[cfg(test)]
#[path = "../tests/unit/editor.rs"]
mod tests;
