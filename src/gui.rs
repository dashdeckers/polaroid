//! The visual editor: the pipeline as a graph of cards, each the form of its node.

mod card;
mod child;
mod drawer;
mod form;
mod known;
mod layout;

use std::collections::BTreeMap;
use std::sync::Arc;

use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::widget::{
    Button,
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
use polaroid_iced::{Menu, Surface, menu, split, surface};
use serde_json::Value;

use self::card::Edit;
use self::drawer::Drawer;
use self::known::Known;
use crate::polars::prelude::{DataFrame, DataType, PolarsResult, polars_err};
use crate::{Catalog, Described, Node, Pipeline, Registry, schema};

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

/// The sizes of text: of a name to type, of an argument, and of what is said beside them.
const TITLE: f32 = 14.0;
const TEXT: f32 = 13.0;
const NOTE: f32 = 12.0;

/// A node's columns and their types, or its error, as the `check` command prints them.
type Columns = Result<Vec<(String, DataType)>, String>;

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
        false => Pipeline::default(),
    };
    layout::layout(&mut pipeline);
    // the window's own chatter would drown the log
    log::set_max_level(log::LevelFilter::Warn);
    let registry = Arc::new(registry);
    let path = path.to_owned();
    // DirectX on Windows, where Intel's Vulkan driver shows frames it has not finished drawing
    #[cfg(windows)]
    if std::env::var_os("WGPU_BACKEND").is_none() {
        // SAFETY: on Windows the environment may be set from any thread
        unsafe { std::env::set_var("WGPU_BACKEND", "dx12") };
    }

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
    catalog: Catalog,
    path: String,
    pipeline: Pipeline,
    is_dirty: bool,
    /// Where the menu of blocks is open: on the surface, and among the cards.
    menu: Option<(Point, Point)>,
    /// How each node is wired, and what a Check and the Runs found of it.
    known: Known,
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
        let docs = catalog
            .blocks
            .iter()
            .map(|(block, entry)| {
                let schema = schema::resolve(&catalog.defs, &entry.args);
                let doc = match schema["description"].as_str() {
                    Some(described) => format!("`{block}`: {described}"),
                    None => format!("`{block}`"),
                };
                ((*block).to_owned(), markdown::Content::parse(&doc))
            })
            .collect();
        let mut editor = Self {
            catalog,
            registry,
            path,
            drawer: Drawer::default(),
            pipeline,
            is_dirty: false,
            menu: None,
            known: Known::default(),
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
            Message::Move(id, position) => self.lay(&id, |node| layout::place(node, position)),
            Message::Resize(id, size) => self.lay(&id, |node| layout::resize(node, size)),
            Message::Copy(text) => return clipboard::write(text).discard(),
            Message::Menu(at, point) => self.menu = Some((at, point)),
            Message::CloseMenu => self.menu = None,
            Message::Add(block, point) => self.add(block, point),
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
            Message::Named(Some(path)) => self.replace(path, Pipeline::default(), true),
            Message::Loaded(None) | Message::Named(None) => {}
            Message::Save => self.save(),
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
                    Ok(checks) => self
                        .known
                        .checked(checks),
                    Err(problem) => self.problem = Some(problem),
                }
            }
            Message::Run => return self.run(false),
            Message::Rerun => return self.run(true),
            Message::Ran(targets, Ok(frames)) => {
                self.job = None;
                self.known
                    .ran(&self.pipeline, &targets, frames);
            }
            Message::Ran(targets, Err(e)) => {
                self.job = None;
                self.known
                    .failed(targets, &e);
            }
            Message::Stop => self.job = None,
            Message::Drawer(message) => {
                let change = self
                    .drawer
                    .update(message, &mut self.pipeline);
                match change {
                    drawer::Change::Nothing => return Task::none(),
                    drawer::Change::Pipeline => {}
                    // what is known of a node holds for the expressions it was computed with
                    drawer::Change::Expression(name) => self.forget(|node| node.uses(&name)),
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

    /// Has `lay` change where the card of node `id` is or how large, which is then to be saved.
    fn lay(
        &mut self,
        id: &str,
        lay: impl FnOnce(&mut Node),
    ) {
        let node = self
            .pipeline
            .nodes
            .iter_mut()
            .find(|node| node.id == id);
        if let Some(node) = node {
            lay(node);
            self.is_dirty = true;
        }
    }

    /// Adds a node of `block` at `point` among the cards, under an id that is free.
    fn add(
        &mut self,
        block: String,
        point: Point,
    ) {
        self.menu = None;
        let described = self
            .catalog
            .blocks
            .get(block.as_str());
        let Some(described) = described else {
            return;
        };
        let nodes = &mut self.pipeline.nodes;
        let id = (1..)
            .map(|n| format!("{block}_{n}"))
            .find(|id| {
                nodes
                    .iter()
                    .all(|node| node.id != *id)
            })
            .expect("some number is free");
        let mut node = Node {
            id,
            block,
            inputs: vec![
                String::new();
                described
                    .inputs
                    .len()
            ],
            args: schema::default(&self.catalog.defs, &described.args),
            ui: Value::Null,
        };
        layout::place(&mut node, point);
        nodes.push(node);
        self.is_dirty = true;
        self.check_wiring();
    }

    /// Writes the pipeline to its file.
    fn save(&mut self) {
        let pipeline = serde_json::to_string_pretty(&self.pipeline).expect("a pipeline serializes");
        let written = std::fs::write(&self.path, pipeline + "\n");
        self.is_dirty = written.is_err();
        // what is wrong with the pipeline itself stays to be seen
        self.check_wiring();
        if let Err(e) = written {
            self.problem = Some(format!("{}: {e}", self.path));
        }
    }

    /// Starts a Run of what is not saved yet, or of everything if it is a Rerun.
    fn run(
        &mut self,
        is_rerun: bool,
    ) -> Task<Message> {
        let (pipeline, targets, read) = self
            .known
            .plan(&self.pipeline, is_rerun);
        let run = child::run(pipeline, read);
        self.start(Task::perform(run, Message::Ran.with(targets)))
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
        self.known = Known::default();
        self.job = None;
        self.menu = None;
        self.is_dirty = is_dirty;
        self.drawer = Drawer::default();
        self.check_wiring();
    }

    /// What the catalog says of the block of a name, if there is such a block.
    fn block(
        &self,
        name: &str,
    ) -> Option<&Described> {
        self.catalog
            .blocks
            .get(name)
    }

    /// Notes how the nodes are wired as of the latest edit, and what is wrong with the pipeline.
    fn check_wiring(&mut self) {
        self.problem = self
            .known
            .wire(&self.pipeline, &self.registry, &self.catalog);
    }

    /// Forgets what is known of the nodes that were edited and of all that read them; stops a job.
    fn forget(
        &mut self,
        is_edited: impl Fn(&Node) -> bool,
    ) {
        self.known
            .forget(&self.pipeline, is_edited);
        self.job = None;
    }

    fn view(&self) -> impl Widget<Message> {
        let expressions = &self
            .pipeline
            .expressions;
        // every column that a Check found, for the expressions to name
        let columns = self
            .known
            .names(&self.pipeline);
        let drawer = self
            .drawer
            .view(&self.catalog.defs, expressions, &columns)
            .map(Message::Drawer);
        let graph = stack![self.graph(), self.menu()];
        let problem = self
            .problem
            .as_deref()
            .map(|problem| container(error(problem)).padding(10));

        column![
            self.toolbar(),
            rule::horizontal(1),
            split(drawer, graph).shown(self.is_drawer_shown),
            problem,
        ]
    }

    /// The drawer's button and the help, then the jobs and the files.
    fn toolbar(&self) -> impl Widget<Message> + use<> {
        let save = self
            .is_dirty
            .then_some(Message::Save);
        // a Check goes before a Run: its button covers theirs until every node is sound
        let is_checked = self
            .known
            .is_checked(&self.pipeline);
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
        row![
            button("Expressions")
                .style(button::secondary)
                .on_press(Message::ToggleDrawer),
            tooltip(container("Help").padding(5), help).position(tooltip::Position::Bottom),
            space::horizontal(),
            stack![runs, over],
            files,
        ]
        .spacing(10)
        .padding(10)
    }

    /// The nodes as cards on a surface, wired as they read each other.
    fn graph(&self) -> Surface<'_, String, Message> {
        let mut graph = surface()
            .on_move(Message::Move)
            .on_resize(Message::Resize)
            .on_wire(|from, to, slot| Message::Edit(to, Edit::Wire(slot, from)))
            .on_unwire(|to, slot| Message::Edit(to, Edit::Wire(slot, String::new())))
            .on_open(|id| Message::Edit(id, Edit::Toggle))
            .on_menu(Message::Menu);
        for node in &self.pipeline.nodes {
            let inputs = self
                .block(&node.block)
                .map_or(0, |block| block.inputs.len());
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
        graph
    }

    /// The menu of blocks under their categories, each to add a node of it where was pressed.
    fn menu(&self) -> Option<Menu<Message>> {
        self.menu
            .map(|(at, point)| {
                let catalog = &self.catalog;
                let groups = catalog
                    .categories
                    .iter()
                    .map(|category| {
                        let blocks = catalog
                            .blocks
                            .iter()
                            .filter(|(_, block)| block.category == *category)
                            .map(|(block, _)| (*block).to_owned())
                            .map(|block| (block.clone(), Message::Add(block, point)))
                            .collect();
                        ((*category).to_owned(), blocks)
                    })
                    .collect();
                menu(at, groups).on_close(Message::CloseMenu)
            })
    }
}

/// The button that lays something open, or closes it if it is open.
fn toggle<'a, Message: Clone + 'a>(
    is_open: bool,
    message: Message,
) -> Button<'a, Message, &'static str> {
    let sign = match is_open {
        true => "−",
        false => "+",
    };
    button(sign)
        .style(button::text)
        .padding([0, 6])
        .on_press(message)
}

/// The button that deletes something.
fn cross<'a, Message: Clone + 'a>(message: Message) -> Button<'a, Message, &'static str> {
    button("×")
        .style(button::danger)
        .padding([0, 6])
        .on_press(message)
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
