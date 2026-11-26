//! A table: an object of any keys, each key beside its value; and rows of one shape as a grid.

use std::iter::once;
use std::sync::LazyLock;

use iced::widget::text_input::TextInput;
use iced::widget::{column, row, space};
use iced::{Element, Font, Widget};
use polaroid_iced::theme::{Tone, font, spacing};
use serde_json::{Map, Value};

use super::{ANY, Form, Message, escaped, is_any};
use crate::gui::look::{add, clear, field, lined, width};
use crate::reference;
use crate::schema::{default, resolve};

impl<'a> Form<'a, '_> {
    /// The entries of a table, each to rename and take away, over a button to add one.
    pub(super) fn table(
        self,
        items: &'a Value,
        value: &'a Value,
        at: &str,
    ) -> Element<'a, Message> {
        static EMPTY: LazyLock<Map<String, Value>> = LazyLock::new(Map::new);
        if let Some(grid) = self.grid(items, value, at) {
            return grid;
        }
        let entries = value
            .as_object()
            .unwrap_or(&EMPTY);
        let wide = widest(entries.keys());
        let lines = entries
            .iter()
            .map(|(key, entry)| {
                let inside = format!("{at}/{}", escaped(key));
                row![
                    self.help.of(
                        clear(Message::Drop(inside.clone())),
                        "Takes this one out of the table."
                    ),
                    self.entry_key(entries, key, at, wide),
                    self.placed(items, entry, inside, true)
                ]
                .spacing(spacing::XS)
                .boxed()
            });
        // one more, to name: none while one waits for its name
        let add = (!entries.contains_key("")).then(|| {
            let more = Message::Put(format!("{at}/"), default(self.defs, items));
            self.help
                .of(add(more), "Adds one to the table, to name.")
        });
        column(lines.chain(add))
            .spacing(spacing::XS)
            .boxed()
    }

    /// The field of `key` in the table at `at`: held as typed at `id` while that `is_taken`.
    fn key(
        self,
        key: &'a str,
        id: String,
        at: &str,
        font: Font,
        is_taken: impl Fn(&str) -> bool + 'a,
        renamed: impl Fn(&str) -> Value + 'a,
    ) -> TextInput<'a, Message> {
        let held = self
            .state
            .keying
            .as_ref()
            .filter(|(held, _)| *held == id)
            .map(|(_, typed)| typed.clone());
        let tone = match held {
            Some(_) => Tone::Refused,
            None => Tone::Given,
        };
        let table = at.to_owned();
        field("key", held.unwrap_or_else(|| key.to_owned()), font, tone).on_input(move |typed| {
            match typed != key && is_taken(&typed) {
                true => Message::Key(id.clone(), typed),
                false => Message::Put(table.clone(), renamed(&typed)),
            }
        })
    }

    /// The field of the key of an entry of `entries`, which renames the entry as it is typed in.
    fn entry_key(
        self,
        entries: &'a Map<String, Value>,
        key: &'a String,
        at: &str,
        wide: f32,
    ) -> Element<'a, Message> {
        let id = format!("{at}/{}", escaped(key));
        let is_taken = |typed: &str| entries.contains_key(typed);
        let renamed = |typed: &str| renamed(entries, key, typed);
        self.key(key, id, at, font::SANS, is_taken, renamed)
            .width(wide)
            .boxed()
    }

    /// Two or more rows of a list or table of any JSON as a grid, if all have the same keys.
    pub(super) fn grid(
        self,
        items: &Value,
        value: &'a Value,
        at: &str,
    ) -> Option<Element<'a, Message>> {
        // each row by the step of the pointer to it, and the keys that the first has
        let rows: Vec<(String, &'a Value)> = match value {
            Value::Array(rows) => rows
                .iter()
                .enumerate()
                .map(|(i, row)| (i.to_string(), row))
                .collect(),
            Value::Object(rows) => rows
                .iter()
                .map(|(key, row)| (escaped(key), row))
                .collect(),
            _ => return None,
        };
        let keys = rows
            .first()?
            .1
            .as_object()?
            .keys();
        // a name is no table, though JSON writes it as one; a cell that holds more is no cell
        let is_name = |value: &Value| reference(value).is_some();
        let is_cell = |cell: &Value| !cell.is_array() && !cell.is_object() || is_name(cell);
        let is_row = |row: &Value| {
            row.as_object()
                .is_some_and(|cells| {
                    cells
                        .keys()
                        .eq(keys.clone())
                        && cells
                            .values()
                            .all(is_cell)
                })
                && !is_name(row)
        };
        let is_grid = is_any(resolve(self.defs, items))
            && keys.len() > 0
            && rows.len() > 1
            && rows
                .iter()
                .all(|(_, row)| is_row(row));
        if !is_grid {
            return None;
        }

        let put = |rows: Value| Message::Put(at.to_owned(), rows);
        // what takes a row out, then the keys of the rows of a table
        let drops = rows
            .iter()
            .map(|(step, _)| {
                self.help.of(
                    clear(Message::Drop(format!("{at}/{step}"))),
                    "Takes this row out.",
                )
            });
        let entries = value.as_object();
        let names = entries.map(|entries| {
            let wide = widest(entries.keys());
            let names = entries
                .keys()
                .map(|key| self.entry_key(entries, key, at, wide));
            headed(space().boxed(), names)
        });
        // then a column of each key, under what renames it and takes it out in every row
        let cells = keys
            .clone()
            .map(|key| {
                let id = format!("{at}/{}/{}", rows[0].0, escaped(key));
                let taken = keys.clone();
                let is_taken = move |typed: &str| {
                    taken
                        .clone()
                        .any(|key| key == typed)
                };
                let renamed = |typed: &str| each(value, |cells| renamed(cells, key, typed));
                let rename = self
                    .key(key, id, at, font::BOLD, is_taken, renamed)
                    .width(width(key, font::BOLD));
                let without = each(value, |cells| {
                    let mut cells = cells.clone();
                    cells.shift_remove(key);
                    Value::Object(cells)
                });
                let head = row![
                    rename,
                    self.help
                        .of(clear(put(without)), "Takes this column out of every row.")
                ];
                let column = rows
                    .iter()
                    .map(|(step, row)| {
                        let inside = format!("{at}/{step}/{}", escaped(key));
                        self.placed(&ANY, &row[key], inside, true)
                    });
                headed(head.boxed(), column)
            });
        // and what adds a column and a row, to name: none while one waits for its name
        let add_column = (!keys
            .clone()
            .any(String::is_empty))
        .then(|| {
            let wider = each(value, |cells| {
                let mut cells = cells.clone();
                cells.insert(String::new(), Value::Null);
                Value::Object(cells)
            });
            let more = self
                .help
                .of(add(put(wider)), "Adds a column to every row, to name.");
            headed(more, [])
        });
        let end = match entries {
            Some(entries) if entries.contains_key("") => None,
            Some(_) => Some(format!("{at}/")),
            None => Some(format!("{at}/{}", rows.len())),
        };
        let add_row = end.map(|end| {
            let blank = keys
                .clone()
                .map(|key| (key.clone(), Value::Null))
                .collect();
            self.help
                .of(add(Message::Put(end, blank)), "Adds a row.")
        });
        let columns = once(headed(space().boxed(), drops))
            .chain(names)
            .chain(cells)
            .chain(add_column);
        Some(
            column![
                row(columns).spacing(spacing::XS),
                add_row
            ]
            .spacing(spacing::XS)
            .boxed(),
        )
    }
}

/// A column of a grid: `head` over `cells`, each in a line of one height.
fn headed<'a>(
    head: Element<'a, Message>,
    cells: impl IntoIterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    column(
        once(head)
            .chain(cells)
            .map(lined),
    )
    .spacing(spacing::XS)
    .boxed()
}

/// The width of a column of fields that holds the widest of `keys`.
fn widest<'a>(keys: impl IntoIterator<Item = &'a String>) -> f32 {
    keys.into_iter()
        .map(|key| width(key, font::SANS))
        .fold(0.0, f32::max)
}

/// `entries` with the key `old` called `new`, in its place; as they are if another is called so.
pub(super) fn renamed(
    entries: &Map<String, Value>,
    old: &str,
    new: &str,
) -> Value {
    if entries.contains_key(new) {
        return Value::Object(entries.clone());
    }
    entries
        .iter()
        .map(|(key, value)| match key == old {
            true => (new.to_owned(), value.clone()),
            false => (key.clone(), value.clone()),
        })
        .collect()
}

/// `rows`, a list or a table of tables, with each of them as `change` leaves it.
pub(super) fn each(
    rows: &Value,
    change: impl Fn(&Map<String, Value>) -> Value,
) -> Value {
    let changed = |row: &Value| {
        row.as_object()
            .map_or(Value::Null, &change)
    };
    match rows {
        Value::Array(rows) => rows
            .iter()
            .map(changed)
            .collect(),
        Value::Object(rows) => rows
            .iter()
            .map(|(key, row)| (key.clone(), changed(row)))
            .collect(),
        _ => Value::Null,
    }
}
