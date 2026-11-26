//! Where the cards are: what a node's `ui` says, and a first place for a node that says nothing.

use std::collections::HashMap;

use iced::{Point, Size};
use polaroid_iced::theme::{extent, surface};
use serde_json::json;

use crate::{Node, Pipeline};

/// The width of a closed card.
pub const WIDTH: f32 = extent::M;
/// The width of an open card until it is resized; it is as tall as what it holds.
const OPEN: f32 = extent::XXL;

/// Where the first card is placed from the corner, and how far the next are beside and below it.
const EDGE: f32 = surface::EDGE;
const BESIDE: f32 = surface::BESIDE;
const BELOW: f32 = surface::BELOW;

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

/// The size of a node's open card as its `ui` says, which what the card holds may exceed.
pub fn size(node: &Node) -> Size {
    let axis = |name, default| {
        node.ui[name]
            .as_f64()
            .map_or(default, |axis| axis as f32)
    };
    Size::new(axis("w", OPEN), axis("h", 0.0))
}

/// Draws a node's open card at `size` from now on, or as large as what it holds if that is more.
pub fn resize(
    node: &mut Node,
    size: Size,
) {
    node.ui["w"] = json!(size.width.round());
    node.ui["h"] = json!(size.height.round());
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
                    EDGE + column as f32 * (WIDTH + BESIDE),
                    EDGE + *row as f32 * BELOW,
                ),
            );
            *row += 1;
        }
    }
}
