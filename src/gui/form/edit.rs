//! The edits a form makes: a value for a JSON pointer.

use serde_json::Value;

/// A value to put at a JSON pointer, or `None` to remove the element there.
#[derive(Debug, Clone)]
pub struct Edit(pub(super) String, pub(super) Option<Value>);

impl Edit {
    /// Applies the Edit to `root`; a missing field or the element past an array's end is added.
    pub fn apply(
        self,
        root: &mut Value,
    ) {
        let Edit(pointer, value) = self;
        let Some((parent, key)) = pointer.rsplit_once('/') else {
            *root = value.unwrap_or_default();
            return;
        };

        match (root.pointer_mut(parent), value) {
            (Some(Value::Object(fields)), Some(value)) => {
                fields.insert(key.to_owned(), value);
            }
            (Some(Value::Array(items)), value) => {
                if let Ok(index) = key.parse::<usize>()
                    && index <= items.len()
                {
                    // none, one or both of an element out and a value in
                    let end = items
                        .len()
                        .min(index + 1);
                    items.splice(index..end, value);
                }
            }
            _ => {}
        }
    }
}
