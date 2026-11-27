//! The drawer of what a pipeline names: its expressions and values, each edited under its name.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};
use std::rc::Rc;

use iced::widget::{
    column,
    container,
    keyed_column,
    mouse_area,
    row,
    scrollable,
    space,
    stack,
    text,
};
use iced::{Center, Element, Fill, Function, Right, Widget, mouse, padding};
use polaroid_iced::theme::{self, Tone, control, font, icon, sign, size, spacing, stroke};
use serde_json::{Map, Value};

use super::form::{self, Expressions, Form};
use super::look::{self, Help, add, cross, toggle};
use super::marks::{self, Marks};
use super::ui;
use crate::{Kind, Pipeline, Registry, methods};

/// What the drawer shows of what a pipeline names, by the names.
#[derive(Default)]
pub struct Drawer {
    sections: BTreeMap<String, Section>,
    /// The entry that is dragged, and the place in its list that the cursor was at last.
    drag: Option<(String, Option<Place>)>,
}

/// What the drawer shows of an expression or a value that the pipeline does not hold.
#[derive(Default)]
struct Section {
    is_open: bool,
    form: form::State,
    /// It as it is edited, if the pipeline does not take that yet.
    draft: Option<form::Draft>,
}

/// A list of the drawer: the expressions without a hole, those with one, or the values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum List {
    Expressions,
    Holed,
    Values,
}

impl List {
    const ALL: [Self; 3] = [
        Self::Expressions,
        Self::Holed,
        Self::Values,
    ];

    /// Its title, the kind of what it lists, and what it says it is for.
    fn says(self) -> (&'static str, Kind, &'static str) {
        match self {
            Self::Expressions => (
                "Expressions",
                Kind::Expression,
                "Adds an empty expression, under a name that is free.",
            ),
            Self::Holed => (
                "Holed expressions",
                Kind::Expression,
                "The expressions that have a hole, which takes what a call of them gives it. An \
                 expression is listed here once a hole is written in it.",
            ),
            Self::Values => (
                "Values",
                Kind::Value,
                "Adds an empty value, under a name that is free: a number, a text, a flag, a \
                 list or a table, for expressions and cards to name.",
            ),
        }
    }

    /// What `pipeline` names that it lists, in the order of the file.
    fn names(
        self,
        pipeline: &Pipeline,
    ) -> impl Iterator<Item = (&String, &Value)> {
        let named = match self {
            Self::Values => &pipeline.values,
            _ => &pipeline.expressions,
        };
        named
            .iter()
            .filter(move |(name, _)| {
                self == Self::Values || pipeline.is_holed(name) == (self == Self::Holed)
            })
    }

    /// What the `ui` of a pipeline calls it.
    fn key(self) -> &'static str {
        match self {
            Self::Expressions => "expressions",
            Self::Holed => "holed",
            Self::Values => "values",
        }
    }

    /// Whether the `ui` of a pipeline has it shown by name, and not in the order of the file.
    fn is_sorted(
        self,
        ui: &Value,
    ) -> bool {
        ui[SORTED]
            .as_array()
            .is_some_and(|sorted| sorted.contains(&self.key().into()))
    }
}

/// Where the `ui` of a pipeline keeps the lists that are shown by name.
const SORTED: &str = "sorted";

/// What is done to the expression or the value of a name.
#[derive(Debug, Clone)]
pub enum Message {
    /// An empty expression or value is added, under a name that is free.
    New(Kind),
    /// Its editor is laid open, or closed if it is open.
    Toggle(String),
    /// It is called this from now on, if the pipeline takes the name.
    Rename(String, String),
    Form(String, form::Message),
    /// It holds this value at this pointer, as a card that names it moved a number of it.
    Put(String, String, Value),
    Delete(String),
    /// This list is shown by name, or in the order of the file again if it is.
    Sort(List),
    /// The entry of this name is dragged by its grip.
    Grab(String),
    /// The cursor of the drag is at this place of the list.
    Over(Place),
    /// The entry that is dragged is let go at the place the cursor was at last.
    Drop,
    /// The drag comes to nothing.
    Cancel,
    /// What is asked of the tags, which the editor sees to.
    Marks(marks::Message),
}

/// Where in its list a dragged entry is let go: before the entry of this name, or after it.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    Before(String),
    After(String),
}

/// What an update did to the pipeline.
pub enum Change {
    Nothing,
    /// The pipeline is another, but no node computes anything else.
    Pipeline,
    /// What is named this is another or gone, for every node that uses it.
    Named(String),
    /// This is asked of the tags, which the drawer does not keep.
    Marks(marks::Message),
}

impl Drawer {
    /// Does what `message` asks, to the drawer or to what `pipeline` names.
    pub fn update(
        &mut self,
        message: Message,
        pipeline: &mut Pipeline,
        registry: &Registry,
    ) -> Change {
        match message {
            Message::Marks(message) => Change::Marks(message),
            Message::Grab(name) => {
                self.drag = Some((name, None));
                Change::Nothing
            }
            Message::Over(place) => {
                if let Some((_, over)) = &mut self.drag {
                    *over = Some(place);
                }
                Change::Nothing
            }
            Message::Cancel => {
                self.drag = None;
                Change::Nothing
            }
            Message::Drop => {
                let Some((name, Some(place))) = self.drag.take() else {
                    return Change::Nothing;
                };
                let (beside, after) = match &place {
                    Place::Before(beside) => (beside, 0),
                    Place::After(beside) => (beside, 1),
                };
                let named = holding(pipeline, &name);
                let index = |named: &Map<String, Value>, name: &str| {
                    named
                        .keys()
                        .position(|key| key == name)
                };
                let (Some(was), true) = (index(named, &name), *beside != name) else {
                    return Change::Nothing;
                };
                let Some(moved) = named.shift_remove(&name) else {
                    return Change::Nothing;
                };
                let at = index(named, beside).map_or(was, |at| at + after);
                named.shift_insert(at, name, moved);
                match was == at {
                    true => Change::Nothing,
                    false => Change::Pipeline,
                }
            }
            Message::Sort(list) => {
                let key = Value::from(list.key());
                let mut sorted = pipeline.ui[SORTED]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                match list.is_sorted(&pipeline.ui) {
                    true => sorted.retain(|sorted| *sorted != key),
                    false => sorted.push(key),
                }
                ui::object(&mut pipeline.ui, &[]).insert(SORTED.to_owned(), sorted.into());
                Change::Pipeline
            }
            Message::New(kind) => {
                let (named, stem) = match kind {
                    Kind::Value => (&mut pipeline.values, "value"),
                    _ => (&mut pipeline.expressions, "expression"),
                };
                let name = (1..)
                    .map(|n| format!("{stem}_{n}"))
                    .find(|name| !named.contains_key(name))
                    .expect("some number is free");
                named.insert(name.clone(), Value::Null);
                self.section(name)
                    .is_open = true;
                Change::Pipeline
            }
            Message::Toggle(name) => {
                let section = self.section(name);
                section.is_open = !section.is_open;
                Change::Nothing
            }
            Message::Rename(old, new) => {
                if !pipeline.rename_named(&old, &new) {
                    return Change::Nothing;
                }
                // its tags and what is shown of it go along to its new name
                marks::renamed(pipeline, &old, Some(&new));
                if let Some(section) = self
                    .sections
                    .remove(&old)
                {
                    self.sections
                        .insert(new, section);
                }
                Change::Pipeline
            }
            Message::Form(name, message) => {
                let Some(stored) = holding(pipeline, &name)
                    .get(&name)
                    .cloned()
                else {
                    return Change::Nothing;
                };
                let section = self.section(name.clone());
                let check = check(pipeline, registry, &name);
                let draft = section
                    .draft
                    .as_ref();
                let change = section
                    .form
                    .update(message, &stored, draft, &*check);
                drop(check);
                match change {
                    form::Change::Nothing => Change::Nothing,
                    // until the pipeline takes it, the nodes that use it have it as it was
                    form::Change::Draft(draft) => {
                        section.draft = Some(draft);
                        Change::Nothing
                    }
                    form::Change::Value(value) => {
                        section.draft = None;
                        if value == stored {
                            return Change::Nothing;
                        }
                        holding(pipeline, &name).insert(name.clone(), value);
                        Change::Named(name)
                    }
                    form::Change::Named(name, at, value) => {
                        self.update(Message::Put(name, at, value), pipeline, registry)
                    }
                }
            }
            Message::Put(name, at, value) => {
                let Some(named) = holding(pipeline, &name).get_mut(&name) else {
                    return Change::Nothing;
                };
                form::put(named, &at, Some(value));
                Change::Named(name)
            }
            Message::Delete(name) => {
                self.sections
                    .remove(&name);
                marks::renamed(pipeline, &name, None);
                match holding(pipeline, &name).shift_remove(&name) {
                    Some(_) => Change::Named(name),
                    None => Change::Nothing,
                }
            }
        }
    }

    /// What is shown of the expression or the value of a name.
    fn section(
        &mut self,
        name: String,
    ) -> &mut Section {
        self.sections
            .entry(name)
            .or_default()
    }

    /// The expressions and the values each under its name; `columns` are those they may name.
    pub fn view<'a>(
        &'a self,
        defs: &'a Value,
        pipeline: &'a Pipeline,
        registry: &'a Registry,
        marks: &'a Marks,
        columns: &[String],
        help: Help,
    ) -> impl Widget<Message> + use<'a> {
        let closed = form::State::default();
        let is_taken = |name: &str| {
            pipeline
                .kind_of(name)
                .is_some()
        };
        let section = |kind: Kind, name: &'a String, stored: &'a Value| {
            let section = self
                .sections
                .get(name);
            let is_open = section.is_some_and(|section| section.is_open);
            // its tags are dots at the end of its field, which is as wide with them as without
            let dots = marks::dots(&pipeline.ui, &marks::of(pipeline, name)).map(|dots| {
                container(dots)
                    .align_x(Right)
                    .center_y(Fill)
                    .width(Fill)
                    .padding(padding::right(theme::FIELD.right))
            });
            let named = stack![
                look::name("name", name, is_taken, Message::Rename.with(name.clone())),
                dots
            ];
            let head = row![
                help.of(
                    toggle(is_open, Message::Toggle(name.clone())),
                    "Lays it open to edit it, or closes it to what it is written as.",
                ),
                help.of(
                    named,
                    "The name that cards and expressions use it by. Type another and press Enter \
                     to rename it, wherever it is used; the border is orange until then, and red \
                     while the name is empty or another's. Leaving the field shows the name \
                     again. Right-click it to pick its tags, each a dot at the end of it.",
                ),
                help.of(
                    cross(Message::Delete(name.clone())),
                    "Deletes it; what used it is left without one.",
                ),
            ]
            .spacing(spacing::XS)
            .align_y(Center);
            // a right press on it opens the picker of its tags under it
            let picker = marks
                .picker(pipeline, name)
                .map(|picker| picker.map(Message::Marks));
            let head = look::popped(
                head,
                picker,
                Message::Marks(marks::Message::Pick(name.clone())),
                Message::Marks(marks::Message::Close),
            );
            // what a call of an expression that has holes gives, or why it is no sound one
            let takes = (kind == Kind::Expression)
                .then(|| pipeline.holes(name))
                .and_then(|holes| match holes {
                    Ok(holes) if holes.is_empty() => None,
                    Ok(holes) => {
                        let holes: Vec<String> = holes
                            .iter()
                            .map(|(hole, taken)| format!("{hole}: {taken}"))
                            .collect();
                        Some(look::aside(format!("takes {}", holes.join(", "))))
                    }
                    Err(e) => Some(look::note(form::line(&e), Tone::Refused)),
                });
            // open it is edited, and closed it is written out
            let draft = section.and_then(|section| {
                section
                    .draft
                    .as_ref()
            });
            let (value, held) = form::Draft::shown(draft, stored);
            let (schema, expressions, written) = match kind {
                Kind::Expression => (&defs["Expr"], Expressions::Chained, methods::text(value)),
                _ => (&form::ANY, Expressions::Named, value.to_string()),
            };
            let body: Element<'a, Message> = match is_open {
                true => Form {
                    state: section.map_or(&closed, |section| &section.form),
                    defs,
                    pipeline,
                    offers: &form::Offers::default(),
                    columns,
                    expressions,
                    check: &check(pipeline, registry, name),
                    value,
                    held,
                    label_width: 0.0,
                    help,
                }
                .view(schema)
                .map(Message::Form.with(name.clone()))
                .boxed(),
                false => look::under_name(look::aside(written).font(font::MONO)),
            };
            column![
                head,
                takes.map(look::under_name),
                body
            ]
            .spacing(spacing::XS)
            .boxed()
        };
        let listed = |list: List| {
            let (title, kind, says) = list.says();
            let title = text(title).size(size::HEADING);
            // nothing is added to those that have a hole: one is written in an expression
            let (title, add) = match list {
                List::Holed => (help.of(title, says), look::slot()),
                _ => (title.boxed(), help.of(add(Message::New(kind)), says)),
            };
            let is_sorted = list.is_sorted(&pipeline.ui);
            let order = match is_sorted {
                true => "a–z",
                false => "file",
            };
            let head = row![
                title,
                space::horizontal(),
                help.of(
                    look::small(order, Message::Sort(list)),
                    "The order of this list: as in the file, or by name. Press it for the other.",
                ),
                add
            ]
            .spacing(spacing::XS)
            .height(control::S)
            .align_y(Center);
            let mut names: Vec<_> = list
                .names(pipeline)
                .collect();
            if is_sorted {
                names.sort_by_key(|(name, _)| name.to_lowercase());
            }
            // in the order of the file, an entry is dragged by its grip to another place in it
            let has_grips = !is_sorted;
            // while one of its entries is dragged, each place in it says that the cursor is there
            let drag = self
                .drag
                .as_ref()
                .filter(|(dragged, _)| {
                    names
                        .iter()
                        .any(|(name, _)| *name == dragged)
                });
            let place = |place: Place, content: Element<'a, Message>| {
                let is_over = drag.is_some_and(|(_, over)| over.as_ref() == Some(&place));
                let line = container(
                    space()
                        .width(Fill)
                        .height(stroke::MEDIUM),
                )
                .style(theme::container::drop(is_over));
                let area = mouse_area(column![line, content].spacing(spacing::XS));
                match drag {
                    Some(_) => area.on_enter(Message::Over(place)),
                    None => area,
                }
                .boxed()
            };
            // after the last of them is a place too: the room between this list and the next
            let room = space()
                .height(spacing::M)
                .boxed();
            let end = match names.last() {
                Some((last, _)) => place(Place::After((*last).clone()), room),
                None => room,
            };
            let sections = names
                .into_iter()
                .map(|(name, stored)| {
                    let grip = match has_grips {
                        true => {
                            let grip = text(sign::GRIP)
                                .size(size::HEADING)
                                .style(theme::text::muted);
                            let grip = container(grip)
                                .center_x(GRIP)
                                .center_y(theme::HEAD);
                            help.of(
                                mouse_area(grip)
                                    .on_press(Message::Grab(name.clone()))
                                    .interaction(mouse::Interaction::Grab),
                                "Drag it up or down to another place in its list, which is its \
                                 place in the file too.",
                            )
                        }
                        // its room stays, for the entries not to move as the order is toggled
                        false => space()
                            .width(GRIP)
                            .boxed(),
                    };
                    let is_dragged = drag.is_some_and(|(dragged, _)| dragged == name);
                    let entry = container(section(kind, name, stored))
                        .style(theme::container::held(is_dragged));
                    let entry = row![grip, entry].spacing(spacing::XS);
                    (key(name), place(Place::Before(name.clone()), entry.boxed()))
                });
            column![
                head,
                keyed_column(sections).spacing(spacing::XS),
                end
            ]
            .spacing(spacing::XS)
            .width(Fill)
            .boxed()
        };
        let named = column(List::ALL.map(listed));
        // the scrollbar beside what is named, not over it
        let bar = look::bar().spacing(spacing::XS);
        let named = scrollable(named)
            .direction(scrollable::Direction::Vertical(bar))
            .style(theme::scrollable::thin);
        let sheet = mouse_area(
            container(named)
                .padding(spacing::M)
                .style(theme::container::sheet)
                .width(Fill)
                .height(Fill),
        );
        // a drag ends where the button is let go, and outside the drawer it comes to nothing
        match self.drag {
            Some(_) => sheet
                .on_release(Message::Drop)
                .on_exit(Message::Cancel),
            None => sheet,
        }
    }
}

/// The room of the grip that an entry is dragged by.
const GRIP: f32 = icon::S;

/// What tells the entry of a name from the others of its list, wherever it is listed there.
fn key(name: &str) -> u64 {
    BuildHasherDefault::<DefaultHasher>::default().hash_one(name)
}

/// The values of `pipeline` if `name` is one of them, and else its expressions.
fn holding<'a>(
    pipeline: &'a mut Pipeline,
    name: &str,
) -> &'a mut Map<String, Value> {
    match pipeline.kind_of(name) {
        Some(Kind::Value) => &mut pipeline.values,
        _ => &mut pipeline.expressions,
    }
}

/// Whether `pipeline` takes `name` as it is edited: sound alone, and taken by each of its users.
fn check<'a>(
    pipeline: &'a Pipeline,
    registry: &'a Registry,
    name: &'a str,
) -> form::Check<'a> {
    // one copy to try every edit in, made when the first is tried
    let tried = RefCell::new(None);
    Rc::new(move |edited| {
        let mut tried = tried.borrow_mut();
        let tried = tried.get_or_insert_with(|| pipeline.clone());
        holding(tried, name).insert(name.to_owned(), edited.clone());
        if tried.kind_of(name) == Some(Kind::Expression) {
            tried
                .holes(name)
                .map_err(|e| form::line(&e))?;
        }
        let users = tried.users(name);
        tried
            .nodes
            .iter()
            .filter(|node| users.contains(&node.id.as_str()))
            .try_for_each(|node| {
                registry
                    .resolved(&node.block, &node.args, tried)
                    .map(drop)
                    .map_err(|e| format!("{}: {}", node.id, form::line(&e.why)).into())
            })
    })
}
