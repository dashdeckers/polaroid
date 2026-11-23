//! The visual editor: the pipeline as a graph of cards, each the form of its node.

mod card;
mod child;
mod drawer;
mod form;
mod layout;
mod table;

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::widget::{
    Text,
    button,
    column,
    component,
    container,
    markdown,
    row,
    rule,
    space,
    stack,
    text,
    tooltip,
};
use iced::{
    Fill,
    Function,
    Point,
    Size,
    Subscription,
    Task,
    Theme,
    Widget,
    clipboard,
    system,
    task,
    theme,
};
use polaroid_iced::{menu, split, surface};
use serde_json::Value;

use self::card::{Edit, Status};
use self::drawer::Drawer;
use crate::polars::prelude::{DataFrame, DataType, PolarsResult, polars_err};
use crate::{Node, Pipeline, Registry, is_sink, polars, schema};

const HELP: &str = "\
Drag a closed card to move it, or any card by the dots in its corner. Drag the background to pan, \
and scroll to zoom.

Drag from the dot on a card's right to a dot on another's left to wire them. Right-click that dot \
to unwire it, and the background to add a node there.

Press the small table on a card and Run to keep its frame. A Run reads again only what was edited \
since, and a Rerun everything.

Double-click a closed card or press its plus to lay it open, and the minus to close it. An open \
card shows its args, and the saved frame or else the columns that Check found. Scroll over the \
table, with shift for sideways. Drag the corner of the card to resize it, and across cells to \
select them for Ctrl+C.

Type an id and press Enter to rename a node. The cross deletes it.

A card turns red at once when it is wired badly. What is wrong with a file shows on Check, which \
comes before a Run: its button covers Run and Rerun until every node that a Run computes is \
checked and sound.

Expressions are kept in the drawer on the left, each under a name, and a card picks them by \
that name. The Expressions button hides the drawer, and its edge is dragged to resize it.

Save is lit when there is something to save. Load asks for another pipeline and New for the name \
of an empty one; either takes the place of this pipeline, and drops what was not saved.";

/// A node's columns and their types, or its error, as the `check` command prints them.
type Columns = Result<Vec<(String, DataType)>, String>;

/// A pipeline of no nodes.
fn empty() -> Pipeline {
    Pipeline {
        polars: polars::VERSION.to_owned(),
        nodes: vec![],
        outputs: vec![],
        expressions: Default::default(),
    }
}

/// A dialog that asks for the file of a pipeline.
fn dialog() -> rfd::AsyncFileDialog {
    rfd::AsyncFileDialog::new().add_filter("pipeline", &["json"])
}

fn path_of(file: rfd::FileHandle) -> String {
    file.path()
        .to_string_lossy()
        .into_owned()
}

/// Opens the editor on the pipeline at `path`, which need not exist yet.
pub fn open(
    registry: Registry,
    path: &str,
) -> PolarsResult<()> {
    let mut pipeline = match std::fs::exists(path)? {
        true => Pipeline::load(path)?,
        false => empty(),
    };
    layout::layout(&mut pipeline);
    // the window's own chatter would drown the log
    log::set_max_level(log::LevelFilter::Warn);
    let registry = Arc::new(registry);
    let path = path.to_owned();

    iced::application(
        move || {
            let editor = Editor::new(registry.clone(), path.clone(), pipeline.clone());
            // light or dark, as the system is
            (editor, system::theme().map(Message::Tone))
        },
        Editor::update,
        Editor::view,
    )
    .title(Editor::title)
    .theme(|editor: &Editor| editor.theme.clone())
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
    /// Where the menu of blocks is open: on the surface, and among the cards.
    menu: Option<(Point, Point)>,
    /// What is wrong with the wiring of each node, as of the latest edit.
    wiring: BTreeMap<String, String>,
    /// The nodes that write their frame, as to a file, which every Run does.
    writes: Vec<String>,
    known: BTreeMap<String, Known>,
    /// What is wrong with the pipeline as a whole, or with the latest Save.
    problem: Option<String>,
    /// The Check or Run under way, whose child is killed when this is dropped.
    job: Option<task::Handle>,
    drawer: Drawer,
    is_drawer_shown: bool,
    /// What each block does, as the Markdown its description is written in.
    docs: BTreeMap<String, markdown::Content>,
    theme: Theme,
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
    Move(String, Point),
    /// The open card of this node is to be of this size.
    Resize(String, Size),
    /// The menu of blocks is asked for here on the surface, which is this point among the cards.
    Menu(Point, Point),
    /// A node of this block, at this point among the cards.
    Add(String, Point),
    CloseMenu,
    Edit(String, Edit),
    /// Text to put on the clipboard.
    Copy(String),
    Save,
    /// A file is asked for, to read a pipeline from in the place of this one.
    Load,
    /// The file that was picked, if one was.
    Loaded(Option<String>),
    /// A name is asked for, to start an empty pipeline under in the place of this one.
    New,
    /// The name that was given, if one was.
    Named(Option<String>),
    Check,
    Checked(Result<BTreeMap<String, Columns>, String>),
    Run,
    /// A Run of everything, whatever was saved before.
    Rerun,
    /// What a Run made of the nodes it was to save or have write.
    Ran(Vec<String>, Result<Vec<(String, DataFrame)>, String>),
    Stop,
    Drawer(drawer::Message),
    /// The drawer is shown, or hidden if it is shown.
    ToggleDrawer,
    /// The system is light or dark.
    Tone(theme::Mode),
}

impl Editor {
    fn new(
        registry: Arc<Registry>,
        path: String,
        pipeline: Pipeline,
    ) -> Self {
        let catalog = registry.catalog();
        // each block by its name as code, then what its description says
        let docs = catalog["blocks"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(block, entry)| {
                let schema = schema::resolve(&catalog["$defs"], &entry["args"]);
                let doc = match schema["description"].as_str() {
                    Some(described) => format!("`{block}`: {described}"),
                    None => format!("`{block}`"),
                };
                (block.clone(), markdown::Content::parse(&doc))
            })
            .collect();
        let mut editor = Self {
            catalog,
            registry,
            path,
            drawer: Drawer::new(&pipeline),
            pipeline,
            is_dirty: false,
            menu: None,
            wiring: BTreeMap::new(),
            writes: Vec::new(),
            known: BTreeMap::new(),
            problem: None,
            job: None,
            is_drawer_shown: true,
            docs,
            theme: Theme::Light,
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
        let escape = keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed {
                key: Key::Named(Named::Escape),
                ..
            } => Some(Message::CloseMenu),
            _ => None,
        });
        Subscription::batch([
            escape,
            system::theme_changes().map(Message::Tone),
        ])
    }

    fn update(
        &mut self,
        message: Message,
    ) -> Task<Message> {
        match message {
            Message::Move(id, position) => {
                if let Some(node) = self.node_mut(&id) {
                    layout::place(node, position);
                }
            }
            Message::Resize(id, size) => {
                if let Some(node) = self.node_mut(&id) {
                    layout::resize(node, size);
                }
            }
            Message::Copy(text) => return clipboard::write(text).discard(),
            Message::Menu(at, point) => self.menu = Some((at, point)),
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
                layout::place(&mut node, point);
                nodes.push(node);
                self.is_dirty = true;
                self.check_wiring();
            }
            Message::Edit(id, edit) => self.edit(&id, edit),
            Message::Load => {
                let file = dialog().pick_file();
                return Task::perform(file, |file| Message::Loaded(file.map(path_of)));
            }
            Message::New => {
                let file = dialog().save_file();
                return Task::perform(file, |file| Message::Named(file.map(path_of)));
            }
            Message::Loaded(Some(path)) => match Pipeline::load(&path) {
                Ok(pipeline) => self.replace(path, pipeline, false),
                Err(e) => self.problem = Some(e.to_string()),
            },
            // its file is made by the first Save
            Message::Named(Some(path)) => self.replace(path, empty(), true),
            Message::Loaded(None) | Message::Named(None) => {}
            Message::Save => {
                let pipeline =
                    serde_json::to_string_pretty(&self.pipeline).expect("a pipeline serializes");
                let written = std::fs::write(&self.path, pipeline + "\n");
                self.is_dirty = written.is_err();
                // what is wrong with the pipeline itself stays to be seen
                self.check_wiring();
                if let Err(e) = written {
                    self.problem = Some(format!("{}: {e}", self.path));
                }
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
                            let known = self.known_mut(id);
                            known.columns = Some(columns);
                            // why a Run failed before may no longer hold
                            known.failed = None;
                        }
                    }
                    Err(problem) => self.problem = Some(problem),
                }
            }
            Message::Run | Message::Rerun => {
                let is_rerun = matches!(message, Message::Rerun);
                for known in self
                    .known
                    .values_mut()
                {
                    known.failed = None;
                    if is_rerun {
                        known.frame = None;
                        known.is_run = false;
                    }
                }
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
                let known = pipeline
                    .nodes
                    .iter()
                    .flat_map(|node| &node.inputs)
                    .filter(|id| {
                        !self
                            .writes
                            .contains(id)
                    })
                    .filter_map(|id| {
                        Some((
                            id.clone(),
                            self.frame(id)?
                                .clone(),
                        ))
                    })
                    .collect();
                let run = child::run(pipeline, known);
                return self.start(Task::perform(run, Message::Ran.with(targets)));
            }
            Message::Ran(targets, Ok(frames)) => {
                self.job = None;
                for id in self.reached(targets) {
                    self.known_mut(id)
                        .is_run = true;
                }
                for (id, mut frame) in frames {
                    // one unticked while the Run was under way is not saved
                    let is_saved = self
                        .pipeline
                        .outputs
                        .contains(&id);
                    if !is_saved {
                        continue;
                    }
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
            Message::Drawer(message) => {
                let change = self
                    .drawer
                    .update(message, &mut self.pipeline);
                let name = match change {
                    drawer::Change::Nothing => return Task::none(),
                    drawer::Change::Pipeline => None,
                    drawer::Change::Expression(name) => Some(name),
                };
                // what is known of a node holds for the expressions it was computed with
                let users: Vec<usize> = self
                    .pipeline
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, node)| {
                        name.as_deref()
                            .is_some_and(|name| drawer::uses(&node.args, name))
                    })
                    .map(|(index, _)| index)
                    .collect();
                for index in users {
                    self.forget(index);
                }
                self.is_dirty = true;
                self.check_wiring();
            }
            Message::ToggleDrawer => self.is_drawer_shown = !self.is_drawer_shown,
            Message::Tone(mode) => {
                self.theme = match mode {
                    theme::Mode::Dark => Theme::Dark,
                    _ => Theme::Light,
                };
            }
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

    /// Puts `pipeline` in the place of this one, as that of the file at `path`.
    fn replace(
        &mut self,
        path: String,
        mut pipeline: Pipeline,
        is_dirty: bool,
    ) {
        layout::layout(&mut pipeline);
        self.path = path;
        self.pipeline = pipeline;
        self.known.clear();
        self.job = None;
        self.menu = None;
        self.is_dirty = is_dirty;
        self.drawer = Drawer::new(&self.pipeline);
        self.check_wiring();
    }

    /// The node of this id, whose card was moved or resized.
    fn node_mut(
        &mut self,
        id: &str,
    ) -> Option<&mut Node> {
        self.is_dirty = true;
        self.pipeline
            .nodes
            .iter_mut()
            .find(|node| node.id == id)
    }

    /// Notes what is wrong with the wiring of each node, and which nodes write.
    fn check_wiring(&mut self) {
        self.wiring.clear();
        self.writes.clear();
        self.problem = None;
        // a node that reads no other reads the disk, which is left to a Check: it is not built
        let sources = self
            .pipeline
            .nodes
            .iter()
            .filter(|node| {
                self.catalog["blocks"][&node.block]["inputs"]
                    .as_array()
                    .is_some_and(Vec::is_empty)
            })
            .map(|node| (node.id.clone(), DataFrame::empty()))
            .collect();
        let frames = match self
            .pipeline
            .frames_from(&self.registry, &sources)
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

    /// These nodes and all they read, which is what a Run of them computes.
    fn reached(
        &self,
        targets: impl IntoIterator<Item = String>,
    ) -> HashSet<String> {
        let mut reached: HashSet<String> = targets
            .into_iter()
            .collect();
        // as many passes as there are nodes reach every one that is read
        for _ in &self.pipeline.nodes {
            for node in &self.pipeline.nodes {
                if reached.contains(&node.id) {
                    reached.extend(
                        node.inputs
                            .iter()
                            .cloned(),
                    );
                }
            }
        }
        reached
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
    ) -> Status {
        let known = self.known.get(id);
        let has_failed = known.is_some_and(|known| {
            known
                .failed
                .is_some()
        });
        let is_run = known.is_some_and(|known| known.is_run);
        match self.found(id) {
            _ if has_failed => Status::Failed,
            Some(Err(_)) => Status::Rejected,
            _ if is_run => Status::Ran,
            Some(Ok(_)) => Status::Checked,
            None => Status::Unknown,
        }
    }

    fn view(&self) -> impl Widget<Message> {
        let save = self
            .is_dirty
            .then_some(Message::Save);
        // a Check goes before a Run: its button covers theirs until every node is sound
        let targets = self
            .pipeline
            .outputs
            .iter()
            .chain(&self.writes)
            .cloned();
        let reached = self.reached(targets);
        let is_checked = self
            .pipeline
            .nodes
            .iter()
            .all(|node| {
                let is_sound = matches!(self.status(&node.id), Status::Checked | Status::Ran);
                // no Check mends bad wiring, which is no matter in a node that no Run computes
                let is_aside = self
                    .wiring
                    .contains_key(&node.id)
                    && !reached.contains(&node.id);
                is_sound || is_aside
            });
        let over = match (&self.job, is_checked) {
            (Some(_), _) => Some(("Stop", Message::Stop)),
            (None, false) => Some(("Check", Message::Check)),
            (None, true) => None,
        }
        .map(|(label, message)| {
            button(
                text(label)
                    .width(Fill)
                    .center(),
            )
            .width(Fill)
            .height(Fill)
            .on_press(message)
        });
        let runs = row![
            button("Run").on_press(Message::Run),
            button("Rerun").on_press(Message::Rerun),
        ]
        .spacing(1);
        let files = row![
            button("Save").on_press_maybe(save),
            button("Load")
                .style(button::secondary)
                .on_press(Message::Load),
            button("New")
                .style(button::secondary)
                .on_press(Message::New),
        ]
        .spacing(1);
        let help = container(text(HELP))
            .width(460)
            .padding(10)
            .style(container::bordered_box);
        let toolbar = row![
            button("Expressions")
                .style(button::secondary)
                .on_press(Message::ToggleDrawer),
            tooltip(container("Help").padding(5), help).position(tooltip::Position::Bottom),
            space::horizontal(),
            stack![runs, over],
            files,
        ]
        .spacing(10)
        .padding(10);

        let mut graph = surface()
            .on_move(Message::Move)
            .on_resize(Message::Resize)
            .on_wire(|from, to, slot| Message::Edit(to, Edit::Wire(slot, from)))
            .on_unwire(|to, slot| Message::Edit(to, Edit::Wire(slot, String::new())))
            .on_open(|id| Message::Edit(id, Edit::Toggle))
            .on_menu(Message::Menu);
        for node in &self.pipeline.nodes {
            let inputs = self.catalog["blocks"][&node.block]["inputs"]
                .as_array()
                .map_or(0, Vec::len);
            let ports = (0..inputs).map(card::anchor);
            let is_open = layout::is_open(node);
            let tall = match is_open {
                true => layout::size(node).height,
                false => 0.0,
            };
            let id = node.id.clone();
            let card = polaroid_iced::card(id, layout::position(node), component(self.card(node)))
                .ports(ports)
                .output(card::output())
                .handle(card::handle())
                // an open card is moved and closed by its buttons alone, which leaves its text
                .loose(!is_open)
                .resizable(is_open)
                .tall(tall);
            graph = graph.push(card);
            let wired = node
                .inputs
                .iter()
                .enumerate()
                .filter(|(_, source)| !source.is_empty());
            for (slot, source) in wired {
                graph = graph.wire(source.clone(), node.id.clone(), slot);
            }
        }
        let menu = self
            .menu
            .map(|(at, point)| {
                // the blocks under their categories, each to add a node of it where was pressed
                let blocks = &self.catalog["blocks"];
                let groups = self.catalog["categories"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|category| {
                        let blocks = blocks
                            .as_object()
                            .into_iter()
                            .flatten()
                            .filter(|(_, block)| block["category"] == category)
                            .map(|(block, _)| (block.clone(), Message::Add(block.clone(), point)))
                            .collect();
                        (category.to_owned(), blocks)
                    })
                    .collect();
                menu(at, groups).on_close(Message::CloseMenu)
            });

        // every column that a Check found, once each, for the expressions to name
        let mut columns = Vec::new();
        let found = self
            .pipeline
            .nodes
            .iter()
            .filter_map(|node| {
                self.found(&node.id)?
                    .ok()
            })
            .flatten();
        for (name, _) in found {
            if !columns.contains(name) {
                columns.push(name.clone());
            }
        }
        let expressions = &self
            .pipeline
            .expressions;
        let drawer = self
            .drawer
            .view(&self.catalog["$defs"], expressions, &columns)
            .map(Message::Drawer);
        let panes = split(drawer, stack![graph, menu]).shown(self.is_drawer_shown);

        let problem = self
            .problem
            .as_deref()
            .map(|problem| container(error(problem)).padding(10));

        column![
            toolbar,
            rule::horizontal(1),
            panes,
            problem,
        ]
    }
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
