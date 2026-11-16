//! Where the cards are: what a node's `ui` says, and a first place for a node that says nothing.

use std::collections::HashMap;

use iced::{Point, Size, Vector};
use serde_json::json;

use super::{HEADER, INPUT, SAVE, TABLE, WIDTH};
use crate::{Node, Pipeline};

/// Where a node's card is drawn, as its `ui` says.
pub fn position(node: &Node) -> Point {
    let axis = |name| {
        node.ui[name]
            .as_f64()
            .unwrap_or_default() as f32
    };
    Point::new(axis("x"), axis("y"))
}

/// Draws a node's card at `position` from now on.
pub fn place(
    node: &mut Node,
    position: Point,
) {
    if !node.ui.is_object() {
        node.ui = json!({});
    }
    node.ui["x"] = position
        .x
        .round()
        .into();
    node.ui["y"] = position
        .y
        .round()
        .into();
}

/// Whether a node's card is laid open, as its `ui` says.
pub fn is_open(node: &Node) -> bool {
    node.ui["expanded"]
        .as_bool()
        .unwrap_or_default()
}

/// Draws a node's card open if it was closed, and closed if it was open.
pub fn toggle(node: &mut Node) {
    node.ui["expanded"] = (!is_open(node)).into();
}

/// The size of a node's open card, as its `ui` says.
pub fn size(node: &Node) -> Size {
    let axis = |name, default| {
        node.ui[name]
            .as_f64()
            .map_or(default, |axis| axis as f32)
    };
    Size::new(axis("w", TABLE.width), axis("h", TABLE.height))
}

/// Draws a node's open card at `size` from now on.
pub fn resize(
    node: &mut Node,
    size: Size,
) {
    node.ui["w"] = size
        .width
        .round()
        .into();
    node.ui["h"] = size
        .height
        .round()
        .into();
}

/// A free-looking place for a new card: right of that of `node`.
pub fn beside(node: &Node) -> Point {
    position(node) + Vector::new(WIDTH + 70.0, 0.0)
}

/// Places every node that has no place yet: a column per step from the sources.
pub fn layout(pipeline: &mut Pipeline) {
    // a node goes one column right of its last input; as many passes as nodes settle that
    let mut columns = HashMap::new();
    for _ in &pipeline.nodes {
        for node in &pipeline.nodes {
            let column = node
                .inputs
                .iter()
                .filter_map(|input| columns.get(input))
                .max()
                .map_or(0, |column| column + 1);
            columns.insert(node.id.clone(), column);
        }
    }

    let mut rows = HashMap::new();
    for node in &mut pipeline.nodes {
        if node.ui.is_null() {
            let column = columns[&node.id];
            let row = rows
                .entry(column)
                .or_insert(0);
            place(
                node,
                Point::new(
                    20.0 + column as f32 * (WIDTH + 70.0),
                    20.0 + *row as f32 * (HEADER + 2.0 * INPUT + SAVE + 30.0),
                ),
            );
            *row += 1;
        }
    }
}
