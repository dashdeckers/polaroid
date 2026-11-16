//! The geometry of the cards: where each part of one is, and what is at a point.

use iced::{Point, Rectangle, Size, Vector};
use serde_json::Value;

use super::layout::{is_open, position, size};
use super::{GRIP, Graph, HEADER, INPUT, PORT, SAVE, STATUS, WIDTH};
use crate::Node;
use crate::polars::prelude::DataFrame;

impl Graph<'_> {
    pub(super) fn inputs(
        &self,
        node: &Node,
    ) -> &[Value] {
        self.blocks[&node.block]["inputs"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// The height of a card's header, inputs and "save output" box, above what it shows when open.
    pub(super) fn top(
        &self,
        node: &Node,
    ) -> f32 {
        HEADER
            + INPUT
                * self
                    .inputs(node)
                    .len() as f32
            + SAVE
    }

    /// The row of a card that holds its "save output" box.
    pub(super) fn save(
        &self,
        index: usize,
    ) -> Rectangle {
        let node = &self.pipeline.nodes[index];
        let top = position(node) + Vector::new(0.0, self.top(node) - SAVE);
        Rectangle::new(top, Size::new(WIDTH, SAVE))
    }

    /// The node whose "save output" box is at `point`.
    pub(super) fn save_at(
        &self,
        point: Point,
    ) -> Option<usize> {
        self.card_at(point)
            .filter(|index| {
                self.save(*index)
                    .contains(point)
            })
    }

    pub(super) fn card(
        &self,
        index: usize,
    ) -> Rectangle {
        let node = &self.pipeline.nodes[index];
        let size = match is_open(node) {
            true => size(node),
            false => Size::new(WIDTH, self.top(node)),
        };
        Rectangle::new(position(node), size)
    }

    /// The id and block of a card, which a double click lays open or closes.
    pub(super) fn header(
        &self,
        index: usize,
    ) -> Rectangle {
        let card = self.card(index);
        Rectangle::new(card.position(), Size::new(card.width, HEADER))
    }

    /// The body of an open card, below its "save output" box.
    pub(super) fn body(
        &self,
        index: usize,
    ) -> Option<Rectangle> {
        let node = &self.pipeline.nodes[index];
        let card = self.card(index);
        let top = self.top(node);
        is_open(node).then(|| {
            Rectangle::new(
                card.position() + Vector::new(STATUS, top),
                Size::new(card.width - 2.0 * STATUS, card.height - top - STATUS),
            )
        })
    }

    /// The body of an open card and the table in it, if it has one.
    pub(super) fn shown(
        &self,
        index: usize,
    ) -> Option<(Rectangle, &DataFrame)> {
        Some((self.body(index)?, self.tables[index].as_deref()?))
    }

    /// The node whose table is at `point`, with what [`Self::shown`] says.
    pub(super) fn shown_at(
        &self,
        point: Point,
    ) -> Option<(usize, Rectangle, &DataFrame)> {
        let index = self.card_at(point)?;
        let (body, table) = self.shown(index)?;
        body.contains(point)
            .then_some((index, body, table))
    }

    /// The corner grip of an open card, which resizes it.
    pub(super) fn grip(
        &self,
        index: usize,
    ) -> Option<Rectangle> {
        let card = self.card(index);
        let corner = Point::new(card.x + card.width - GRIP, card.y + card.height - GRIP);
        is_open(&self.pipeline.nodes[index]).then(|| Rectangle::new(corner, Size::new(GRIP, GRIP)))
    }

    /// The node whose corner grip is at `point`.
    pub(super) fn grip_at(
        &self,
        point: Point,
    ) -> Option<usize> {
        self.card_at(point)
            .filter(|index| {
                self.grip(*index)
                    .is_some_and(|grip| grip.contains(point))
            })
    }

    /// The node whose card is at `point`, the topmost first.
    pub(super) fn card_at(
        &self,
        point: Point,
    ) -> Option<usize> {
        (0..self
            .pipeline
            .nodes
            .len())
            .rev()
            .find(|index| {
                self.card(*index)
                    .contains(point)
            })
    }

    /// The node whose output port is at `point`.
    pub(super) fn output_at(
        &self,
        point: Point,
    ) -> Option<usize> {
        (0..self
            .pipeline
            .nodes
            .len())
            .rev()
            .find(|index| {
                self.output(*index)
                    .distance(point)
                    <= 2.0 * PORT
            })
    }

    /// Where a node's wire leaves its card.
    pub(super) fn output(
        &self,
        index: usize,
    ) -> Point {
        let card = self.card(index);
        Point::new(card.x + card.width, card.y + HEADER / 2.0)
    }

    /// The node and slot of the input port at `point`.
    pub(super) fn input_at(
        &self,
        point: Point,
    ) -> Option<(usize, usize)> {
        self.pipeline
            .nodes
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, node)| {
                let slot = (0..self
                    .inputs(node)
                    .len())
                    .find(|slot| input(node, *slot).distance(point) <= 2.0 * PORT)?;
                Some((index, slot))
            })
    }
}

/// Where the wire of a node's input `slot` enters its card.
pub fn input(
    node: &Node,
    slot: usize,
) -> Point {
    position(node) + Vector::new(0.0, HEADER + INPUT * (slot as f32 + 0.5))
}
