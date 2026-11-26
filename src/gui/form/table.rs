//! A table: an object of any keys, each key beside its value.

use std::sync::LazyLock;

use iced::widget::{column, row};
use iced::{Element, Widget};
use serde_json::{Map, Value};

use super::look::{Face, GAP, Tone, field, small, width};
use super::{Form, Message, escaped};
use crate::schema::default;

impl<'a> Form<'a, '_> {
    /// The entries of a table, each to rename and take away, over a button to add one.
    pub(super) fn table(
        self,
        items: &'a Value,
        value: &'a Value,
        at: &str,
    ) -> Element<'a, Message> {
        static EMPTY: LazyLock<Map<String, Value>> = LazyLock::new(Map::new);
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
                        small("×", Message::Drop(inside.clone())),
                        "Takes this one out of the table."
                    ),
                    named(entries, key, at, wide),
                    self.placed(items, entry, inside, true)
                ]
                .spacing(GAP)
                .boxed()
            });
        // one more, to name: none while one waits for its name
        let add = (!entries.contains_key("")).then(|| {
            let add = Message::Put(format!("{at}/"), default(self.defs, items));
            self.help
                .of(small("+", add), "Adds one to the table, to name.")
        });
        column(lines.chain(add))
            .spacing(GAP)
            .boxed()
    }
}

/// The width of a column of fields that holds the widest of `keys`.
fn widest<'a>(keys: impl IntoIterator<Item = &'a String>) -> f32 {
    keys.into_iter()
        .map(|key| width(key, Face::Plain))
        .fold(0.0, f32::max)
}

/// The field of the key of an entry of `entries`, which renames the entry as it is typed in.
fn named<'a>(
    entries: &'a Map<String, Value>,
    key: &'a String,
    at: &str,
    wide: f32,
) -> Element<'a, Message> {
    let table = at.to_owned();
    field("key", key.as_str(), Face::Plain, Tone::Given)
        .width(wide)
        .on_input(move |typed| Message::Put(table.clone(), renamed(entries, key, &typed)))
        .boxed()
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
