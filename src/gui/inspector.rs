//! The panel for the selected node: its id, inputs and args, and what is known of it.

use iced::widget::{button, checkbox, column, pick_list, row, rule, text, text_input};
use iced::{Center, Widget};

use super::{Editor, error, form};
use crate::{Node, schema};

/// A change to the selected node.
#[derive(Debug, Clone)]
pub enum Edit {
    Rename(String),
    /// It reads the node of this id as its input of this slot.
    Wire(usize, String),
    /// Whether a Run saves its output.
    Output(bool),
    /// What the form of its args asks for.
    Form(form::Message),
    Delete,
}

/// Applies `edit` to the selected node, if there is one.
pub fn edit(
    editor: &mut Editor,
    edit: Edit,
) {
    let Some(index) = editor.selected else {
        return;
    };
    match edit {
        Edit::Rename(id) => {
            // an empty or taken id would have its readers taken for another node's
            let is_taken = editor
                .pipeline
                .nodes
                .iter()
                .any(|node| node.id == id);
            if id.is_empty() || is_taken {
                return;
            }
            editor.forget(index);
            let nodes = &mut editor
                .pipeline
                .nodes;
            let old = std::mem::replace(&mut nodes[index].id, id.clone());
            let readers = nodes
                .iter_mut()
                .flat_map(|node| &mut node.inputs)
                .chain(
                    &mut editor
                        .pipeline
                        .outputs,
                );
            for reader in readers {
                if *reader == old {
                    reader.clone_from(&id);
                }
            }
        }
        Edit::Wire(slot, source) => {
            editor.forget(index);
            let inputs = &mut editor
                .pipeline
                .nodes[index]
                .inputs;
            let len = inputs
                .len()
                .max(slot + 1);
            inputs.resize(len, String::new());
            inputs[slot] = source;
        }
        Edit::Output(is_output) => {
            let outputs = &mut editor
                .pipeline
                .outputs;
            let id = &editor
                .pipeline
                .nodes[index]
                .id;
            outputs.retain(|output| output != id);
            match is_output {
                true => outputs.push(id.clone()),
                // a frame that is no longer saved is let go
                false => {
                    if let Some(known) = editor
                        .known
                        .get_mut(id)
                    {
                        known.frame = None;
                    }
                }
            }
        }
        Edit::Form(message) => {
            let args = &mut editor
                .pipeline
                .nodes[index]
                .args;
            let is_changed = editor
                .form
                .update(message, args);
            // the form may only show its value another way
            if !is_changed {
                return;
            }
            editor.forget(index);
        }
        Edit::Delete => {
            editor.forget(index);
            let node = editor
                .pipeline
                .nodes
                .remove(index);
            editor
                .pipeline
                .outputs
                .retain(|output| *output != node.id);
            editor.select(None);
        }
    }
    editor.is_dirty = true;
    editor.check_wiring();
}

/// The selected node: its id, inputs and args, then its columns or error.
pub fn view<'a>(
    editor: &'a Editor,
    node: &'a Node,
) -> impl Widget<Edit> {
    let defs = &editor.catalog["$defs"];
    let block = &editor.catalog["blocks"][&node.block];
    let schema = schema::resolve(defs, &block["args"]);

    let others: Vec<&str> = editor
        .pipeline
        .nodes
        .iter()
        .map(|other| other.id.as_str())
        .filter(|id| *id != node.id)
        .collect();
    let inputs = block["inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(slot, name)| {
            let source = node
                .inputs
                .get(slot)
                .map(String::as_str)
                .filter(|source| !source.is_empty());
            row![
                name.as_str()
                    .unwrap_or_default(),
                pick_list(source, others.clone(), ToString::to_string)
                    .on_select(move |source| Edit::Wire(slot, source.to_owned()))
                    .placeholder("unwired"),
            ]
            .spacing(10)
            .align_y(Center)
        });

    let is_output = editor
        .pipeline
        .outputs
        .contains(&node.id);
    let found = match editor.found(&node.id) {
        Some(Ok(columns)) => {
            let lines: Vec<String> = columns
                .iter()
                .map(|(name, dtype)| format!("{name}: {dtype}"))
                .collect();
            text(lines.join("\n")).selectable(true)
        }
        Some(Err(e)) => error(e),
        None => text("Not checked."),
    };
    let failed = editor
        .known
        .get(&node.id)
        .and_then(|known| {
            known
                .failed
                .as_deref()
        })
        .map(error);

    column![
        text_input("id", &node.id).on_input(Edit::Rename),
        text(&node.block),
        schema["description"]
            .as_str()
            .map(|description| text(description.replace('\n', " ")).style(text::secondary)),
        column(inputs).spacing(10),
        checkbox(is_output)
            .label("save output")
            .on_toggle(Edit::Output),
        rule::horizontal(1),
        form::view(
            &editor.form,
            defs,
            schema,
            &node.args,
            columns(editor, node)
        )
        .map(Edit::Form),
        rule::horizontal(1),
        found,
        failed,
        button("Delete")
            .style(button::danger)
            .on_press(Edit::Delete),
    ]
    .spacing(10)
    .padding(10)
}

/// The columns `node` may name: those the latest Check found of its inputs.
fn columns(
    editor: &Editor,
    node: &Node,
) -> Vec<String> {
    let found = node
        .inputs
        .iter()
        .filter_map(|id| {
            editor
                .found(id)?
                .ok()
        })
        .flatten();

    // once each, as two inputs may share a column
    let mut columns = Vec::new();
    for (name, _) in found {
        if !columns.contains(name) {
            columns.push(name.clone());
        }
    }
    columns
}
