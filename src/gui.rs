//! The visual editor: the pipeline as a graph of cards, each the form of its node.

mod card;
mod child;
mod drawer;
mod form;
mod gallery;
mod known;
mod layout;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;

use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::widget::{
    Button,
    button,
    column,
    component,
    container,
    markdown,
    row,
    rule,
    scrollable,
    space,
    stack,
    text,
    text_input,
    tooltip,
};
use iced::{
    Element,
    Fill,
    Function,
    Point,
    Shrink,
    Size,
    Subscription,
    Task,
    Theme,
    Widget,
    clipboard,
    padding,
    system,
    task,
    theme,
};
use polaroid_iced::{Menu, Surface, menu, scrub, split, surface};
use serde_json::Value;

use self::card::Edit;
use self::drawer::Drawer;
use self::known::Known;
use crate::polars::prelude::{DataFrame, DataType, PolarsResult, polars_err};
use crate::{Catalog, Described, Node, Pipeline, Registry, schema};

/// What the editor is, and what is done in it that no button does; what the Help button says.
const INTRO: &str = "\
A pipeline is a chain of nodes, each a card: it reads the frames of the cards wired into it, and \
gives one on. Right-click the background to add a node. Drag from the dot on the right of a card \
to a dot on the left of another to wire them, and right-click that dot to unwire it. Drag the \
background to pan, and scroll to zoom. Drag a closed card anywhere, or any card by the dots in \
its corner; left on another for a moment, it swaps places with it when dropped, wiring and all.

While Help is on, everything says what it is for when the cursor rests on it. Press Help again \
to turn that off.";

/// Whether what the editor is made of says what it is for when hovered.
#[derive(Debug, Clone, Copy, Default)]
pub struct Help(bool);

impl Help {
    /// `content`, which `says` what it is for when hovered while help is on.
    pub fn of<'a, Message: 'a>(
        self,
        content: impl Widget<Message> + 'a,
        says: &str,
    ) -> Element<'a, Message> {
        let says = self
            .0
            .then(|| says.to_owned());
        // under it, where what Polars says of it, over it, is not hidden
        form::told_at(content, says, tooltip::Position::Bottom)
    }
}

/// The sizes of text: of a name to type, of an argument, and of what is said beside them.
const TITLE: f32 = 14.0;
const TEXT: f32 = 13.0;
const NOTE: f32 = 12.0;

/// How long Delay waits at the least for a change to be the last, and a step of a drag on it.
const STILL: Duration = Duration::from_millis(500);
const LONGER: Duration = Duration::from_millis(100);
/// How long a gallery that is played shows each image.
const FRAME: Duration = Duration::from_millis(200);

/// When a Check and a Run follow a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// At once.
    Live,
    /// Once no other change has come for a while.
    Delay,
    /// When their button is pressed.
    Manual,
}

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
    /// The process that checks and runs, which is kept between one job and the next.
    worker: child::Worker,
    /// The Check or Run under way, which is stopped, and its process killed, when this is dropped.
    job: Option<task::Handle>,
    mode: Mode,
    /// How long Delay waits for a change to be the last, before a Check and a Run follow it.
    still: Duration,
    /// Which of the images of its frame the card of a node shows, by its id.
    shown: BTreeMap<String, gallery::State>,
    drawer: Drawer,
    is_drawer_shown: bool,
    help: Help,
    /// How many times their size the cards are drawn, which a plot draws its pixels for.
    zoom: f32,
    /// What each block does, as the Markdown its description is written in.
    docs: BTreeMap<String, markdown::Content>,
    theme: Theme,
}

#[derive(Debug, Clone)]
enum Message {
    Move(String, Point),
    /// The first node was dragged from this point and dropped on the second, to swap with it.
    Swap(String, String, Point),
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
    Reload,
    Mode(Mode),
    /// Delay is to wait this many steps longer, or shorter if they are fewer than none.
    Wait(i32),
    /// What is asked of the gallery of a node.
    Gallery(String, gallery::Message),
    /// The galleries that are played show their next image.
    Tick,
    /// Why the image of a gallery was not written to a file, if it was not.
    Exported(Option<String>),
    /// What a Run made of the nodes it was to save or have write.
    Ran(Vec<String>, Result<Vec<(String, DataFrame)>, String>),
    Stop,
    Drawer(drawer::Message),
    /// The drawer is shown, or hidden if it is shown.
    ToggleDrawer,
    /// Help is shown, or no longer if it is.
    Help,
    /// The cards are drawn this many times their size.
    Zoom(f32),
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
            worker: child::Worker::default(),
            job: None,
            mode: Mode::Manual,
            still: STILL,
            shown: BTreeMap::new(),
            is_drawer_shown: true,
            help: Help::default(),
            zoom: 1.0,
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
        let is_played = self
            .shown
            .values()
            .any(|shown| shown.is_played);
        let tick = match is_played {
            true => iced::time::every(FRAME).map(|_| Message::Tick),
            false => Subscription::none(),
        };
        Subscription::batch([
            escape,
            tick,
            system::theme_changes().map(Message::Tone),
        ])
    }

    fn update(
        &mut self,
        message: Message,
    ) -> Task<Message> {
        match message {
            Message::Move(id, position) => self.lay(&id, |node| layout::place(node, position)),
            Message::Swap(dragged, under, from) => {
                if self.swap(&dragged, &under, from) {
                    return self.follow();
                }
            }
            Message::Resize(id, size) => self.lay(&id, |node| layout::resize(node, size)),
            Message::Copy(text) => return clipboard::write(text).discard(),
            Message::Menu(at, point) => self.menu = Some((at, point)),
            Message::CloseMenu => self.menu = None,
            Message::Add(block, point) => self.add(block, point),
            Message::Edit(id, edit) => {
                // a card laid open or closed is the same node
                let is_change = !matches!(edit, Edit::Toggle);
                self.edit(&id, edit);
                if is_change {
                    return self.follow();
                }
            }
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
            // their button is that of the mode in which nothing follows a change
            Message::Check => {
                self.mode = Mode::Manual;
                return self.check(Duration::ZERO, true);
            }
            Message::Checked(checks) => {
                self.job = None;
                match checks {
                    Ok(checks) => self
                        .known
                        .checked(checks),
                    Err(problem) => self.problem = Some(problem),
                }
                let is_sound = self
                    .known
                    .is_checked(&self.pipeline);
                if self.mode != Mode::Manual && is_sound {
                    return self.run(false);
                }
            }
            Message::Run => {
                self.mode = Mode::Manual;
                return self.run(false);
            }
            Message::Reload => return self.run(true),
            Message::Mode(mode) => {
                self.mode = mode;
                return self.follow();
            }
            Message::Wait(steps) => {
                let longer = LONGER * steps.unsigned_abs();
                self.still = match steps > 0 {
                    true => self.still + longer,
                    false => self
                        .still
                        .saturating_sub(longer)
                        .max(STILL),
                };
            }
            Message::Gallery(id, message) => return self.browse(id, message),
            Message::Tick => {
                for (id, shown) in &mut self.shown {
                    match self
                        .known
                        .gallery(id)
                    {
                        Some(gallery) if shown.is_played => {
                            shown.update(gallery::Message::Step, gallery)
                        }
                        // one whose frame is gone has nothing to play
                        _ => shown.is_played = false,
                    }
                }
            }
            Message::Exported(problem) => self.problem = problem.or(self.problem.take()),
            Message::Ran(targets, Ok(frames)) => {
                self.job = None;
                self.known
                    .ran(&self.pipeline, &self.registry, &targets, frames);
            }
            Message::Ran(targets, Err(e)) => {
                self.job = None;
                self.known
                    .failed(targets, &e);
            }
            Message::Stop => {
                self.mode = Mode::Manual;
                self.job = None;
            }
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
                return self.follow();
            }
            Message::ToggleDrawer => self.is_drawer_shown = !self.is_drawer_shown,
            Message::Help => self.help = Help(!self.help.0),
            Message::Zoom(zoom) => self.zoom = zoom,
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

    /// Starts a Check, once `wait` has passed: of every node, or of those alone that were edited
    /// since the columns of the others were found.
    fn check(
        &mut self,
        wait: Duration,
        is_whole: bool,
    ) -> Task<Message> {
        let found = match is_whole {
            true => HashMap::new(),
            false => self.known.found(),
        };
        let check = self
            .worker
            .clone()
            .check(
                self.pipeline
                    .clone(),
                found,
            );
        let check = async move {
            tokio::time::sleep(wait).await;
            check.await
        };
        self.start(Task::perform(check, Message::Checked))
    }

    /// Has a Check follow a change as the mode says, and a Run that Check if it finds all sound.
    ///
    /// None follows a change that leaves the pipeline or a node that a Run computes unsound.
    fn follow(&mut self) -> Task<Message> {
        let is_sound = self
            .known
            .is_wired(&self.pipeline);
        if self
            .problem
            .is_some()
            || !is_sound
        {
            return Task::none();
        }
        match self.mode {
            Mode::Live => self.check(Duration::ZERO, false),
            // a change that comes while it waits takes its place
            Mode::Delay => self.check(self.still, false),
            Mode::Manual => Task::none(),
        }
    }

    /// Does what is asked of the gallery of node `id`: shows another image, or writes the one shown.
    fn browse(
        &mut self,
        id: String,
        message: gallery::Message,
    ) -> Task<Message> {
        let saved = self
            .known
            .frame(&id)
            .zip(
                self.known
                    .gallery(&id),
            );
        let Some((frame, gallery)) = saved else {
            return Task::none();
        };
        let shown = self
            .shown
            .entry(id.clone())
            .or_default();
        let gallery::Message::Save = message else {
            shown.update(message, gallery);
            return Task::none();
        };
        let Some((image, kind)) = gallery.export(shown, frame) else {
            return Task::none();
        };
        let image = image.to_vec();
        let write = async move {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("image", &[kind])
                .set_file_name(format!("{id}.{kind}"))
                .save_file()
                .await?;
            let written = std::fs::write(file.path(), image);
            written
                .err()
                .map(|e| {
                    format!(
                        "{}: {e}",
                        file.path()
                            .display()
                    )
                })
        };
        Task::perform(write, Message::Exported)
    }

    /// Starts a Run of what is not saved yet, or of everything if it is a Reload.
    fn run(
        &mut self,
        is_reload: bool,
    ) -> Task<Message> {
        let (pipeline, targets, read) = self
            .known
            .plan(&self.pipeline, is_reload);
        let run = self
            .worker
            .clone()
            .run(pipeline, read);
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

    /// Whether two nodes can swap places: their blocks read as many frames.
    fn swaps(
        &self,
        one: &str,
        other: &str,
    ) -> bool {
        let reads = |id: &str| {
            let node = self
                .pipeline
                .nodes
                .iter()
                .find(|node| node.id == id)?;
            Some(
                self.block(&node.block)?
                    .inputs
                    .len(),
            )
        };
        one != other && reads(one).is_some() && reads(one) == reads(other)
    }

    /// Has the node that was dragged from `from` swap places with the one it was dropped on.
    ///
    /// Each takes the wiring of the other and where its card was; false if they cannot.
    fn swap(
        &mut self,
        dragged: &str,
        under: &str,
        from: Point,
    ) -> bool {
        if !self.swaps(dragged, under) {
            return false;
        }
        let place = self
            .pipeline
            .nodes
            .iter()
            .find(|node| node.id == under)
            .map(layout::position);
        let Some(place) = place else {
            return false;
        };
        // what they and their readers were found to be, they no longer are
        self.forget(|node| node.id == dragged || node.id == under);
        self.pipeline
            .swap(dragged, under);
        self.lay(dragged, |node| layout::place(node, place));
        self.lay(under, |node| layout::place(node, from));
        self.check_wiring();
        true
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
        let ids = self
            .pipeline
            .nodes
            .iter()
            .map(|node| &node.id);
        let columns = self
            .known
            .names(ids);
        let drawer = self
            .drawer
            .view(&self.catalog.defs, expressions, &columns, self.help)
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

    /// The drawer's button and the help, then the modes, Reload and the files.
    fn toolbar(&self) -> impl Widget<Message> + use<> {
        let save = self
            .is_dirty
            .then_some(Message::Save);
        // a Check goes before a Run: the button is a Check until every node is sound
        let is_checked = self
            .known
            .is_checked(&self.pipeline);
        let (manual, pressed) = match (&self.job, is_checked) {
            (Some(_), _) => ("Stop", Message::Stop),
            (None, false) => ("Check", Message::Check),
            (None, true) => ("Run", Message::Run),
        };
        let delay = format!(
            "Delay {:.1} s",
            self.still
                .as_secs_f32()
        );
        let mode = |label: String, mode, pressed| {
            let style = match self.mode == mode {
                true => button::primary,
                false => button::secondary,
            };
            button(text(label).center())
                .style(style)
                .on_press(pressed)
        };
        let help = self.help;
        let manual_says = match manual {
            "Stop" => "Stops the Check or Run that is under way.",
            "Check" => {
                "Manual: nothing follows a change. Check resolves the columns of every node, \
                 without reading any rows; once all are sound, this button is Run."
            }
            _ => {
                "Manual: nothing follows a change. Run computes the frames that are kept and \
                 writes the files; it reads again only what was edited since the last."
            }
        };
        let modes = row![
            help.of(
                mode("Live".to_owned(), Mode::Live, Message::Mode(Mode::Live)),
                "Live: a Check and a Run follow every change at once, if it leaves all sound.",
            ),
            // dragged up it waits longer, and down again shorter
            help.of(
                scrub(mode(delay, Mode::Delay, Message::Mode(Mode::Delay)).width(120))
                    .upward()
                    .on_scrub(Message::Wait),
                "Delay: a Check and a Run follow a change once there has been none for this \
                 long. Drag the button up to wait longer, and down for shorter.",
            ),
            // as wide whatever it says
            help.of(
                mode(manual.to_owned(), Mode::Manual, pressed).width(64),
                manual_says
            ),
        ]
        .spacing(1);
        let files = row![
            help.of(
                button("Save").on_press_maybe(save),
                "Writes the pipeline to its file, if it was changed since.",
            ),
            help.of(
                button("Load")
                    .style(button::secondary)
                    .on_press(Message::Load),
                "Opens the pipeline of another file in place of this one.",
            ),
            help.of(
                button("New")
                    .style(button::secondary)
                    .on_press(Message::New),
                "Starts an empty pipeline, in a file that the first Save makes.",
            ),
        ]
        .spacing(1);
        let asked = match help.0 {
            true => button::primary,
            false => button::secondary,
        };
        row![
            help.of(
                button("Expressions")
                    .style(button::secondary)
                    .on_press(Message::ToggleDrawer),
                "Shows or hides the drawer of expressions on the left. Drag its edge to resize it.",
            ),
            help.of(
                button("Help")
                    .style(asked)
                    .on_press(Message::Help),
                INTRO,
            ),
            space::horizontal(),
            modes,
            help.of(
                button("Reload")
                    .style(button::secondary)
                    .on_press(Message::Reload),
                "Runs everything again from the files on disk, whatever was kept before.",
            ),
            files,
        ]
        .spacing(10)
        .padding(10)
    }

    /// The nodes as cards on a surface, wired as they read each other.
    fn graph(&self) -> Surface<'_, String, Message> {
        let mut graph = surface()
            .on_move(Message::Move)
            .on_swap(|dragged, under, from| {
                self.swaps(&dragged, &under)
                    .then_some(Message::Swap(dragged, under, from))
            })
            .on_resize(Message::Resize)
            .on_wire(|from, to, slot| Message::Edit(to, Edit::Wire(slot, from)))
            .on_unwire(|to, slot| Message::Edit(to, Edit::Wire(slot, String::new())))
            .on_open(|id| Message::Edit(id, Edit::Toggle))
            .on_menu(Message::Menu)
            .on_zoom(Message::Zoom);
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

/// How thick the bars are that an error is scrolled by.
const BAR: f32 = 6.0;

/// The most that an error takes of the height of what shows it.
const ERROR: f32 = 120.0;

/// An error, which can be selected to copy it; one that is long or wide is scrolled through.
fn error<'a, Message: 'a>(message: &'a str) -> Element<'a, Message> {
    let written = text(message)
        .style(text::danger)
        .wrapping(text::Wrapping::None)
        .selectable(true);
    let bar = || {
        scrollable::Scrollbar::new()
            .width(BAR)
            .scroller_width(BAR)
            .margin(0)
    };
    let direction = scrollable::Direction::Both {
        vertical: bar(),
        horizontal: bar(),
    };
    // the bars lie over the edges of what is seen, so the text ends clear of them
    let clear = padding::right(BAR + 2.0).bottom(BAR + 2.0);
    scrollable(container(written).padding(clear))
        .direction(direction)
        .width(Fill)
        .height(Shrink.max(ERROR))
        .style(|theme: &Theme, status| {
            // and no text shows through them: it ends at a bar as at an edge
            let paper = theme
                .palette()
                .background
                .base
                .color;
            let mut style = scrollable::default(theme, status);
            style
                .vertical_rail
                .background = Some(paper.into());
            style
                .horizontal_rail
                .background = Some(paper.into());
            style.gap = Some(paper.into());
            style
        })
        .boxed()
}

/// A field for a name, which is given on Enter: what is `typed` until then, marked as not given.
fn name_field<'a, Message: Clone + 'a>(
    placeholder: &'static str,
    typed: Option<&str>,
    name: &str,
) -> text_input::TextInput<'a, Message> {
    let is_typed = is_unsaved(typed, name);
    let shown = typed
        .unwrap_or(name)
        .to_owned();
    text_input(placeholder, shown).style(move |theme: &Theme, status| {
        let mut style = text_input::default(theme, status);
        if is_typed {
            style.border.color = theme
                .palette()
                .warning
                .base
                .color;
            style.border.width = 2.0;
        }
        style
    })
}

/// Whether what is typed for a name is another than the name, which Enter is still to give.
fn is_unsaved(
    typed: Option<&str>,
    name: &str,
) -> bool {
    typed.is_some_and(|typed| typed != name)
}

#[cfg(test)]
#[path = "../tests/unit/editor.rs"]
mod tests;
