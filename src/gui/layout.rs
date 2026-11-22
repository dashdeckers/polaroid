//! Where the cards are: what a node's `ui` says, and a first place for a node that says nothing.

use std::collections::HashMap;

use iced::{Point, Size, Vector};
use serde_json::json;

use super::table;
use crate::{Node, Pipeline};

/// The width of a closed card.
pub const WIDTH: f32 = 220.0;
/// The width of an open card and the height of the table in it, until it is resized.
const OPEN: Size = Size::new(520.0, 240.0);

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
    node.ui["x"] = json!(position.x.round());
    node.ui["y"] = json!(position.y.round());
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

/// The width of a node's open card and the height of the table in it, as its `ui` says.
pub fn size(node: &Node) -> Size {
    let axis = |name, default| {
        node.ui[name]
            .as_f64()
            .map_or(default, |axis| axis as f32)
    };
    Size::new(axis("w", OPEN.width), axis("h", OPEN.height))
}

/// Draws a node's open card this much larger from now on, down to a row of its table.
pub fn grow(
    node: &mut Node,
    by: Vector,
) {
    let Size { width, height } = size(node);
    let least = table::HEADER + table::ROW;
    node.ui["w"] = json!((width + by.x).max(WIDTH));
    node.ui["h"] = json!((height + by.y).max(least));
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
        if !node.ui.is_object() {
            // whatever it was instead could hold no place
            node.ui = json!({});
            let column = columns[&node.id];
            let row = rows
                .entry(column)
                .or_insert(0);
            place(
                node,
                Point::new(
                    20.0 + column as f32 * (WIDTH + 70.0),
                    20.0 + *row as f32 * 160.0,
                ),
            );
            *row += 1;
        }
    }
}
