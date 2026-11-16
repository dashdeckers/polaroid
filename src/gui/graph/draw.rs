//! Drawing the graph: wires under cards, and the tables on a layer of their own.

use iced::theme::palette::Palette;
use iced::widget::canvas::{self, Frame, Path, Stroke};
use iced::widget::text;
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, Vector};

use super::card::input;
use super::{Drag, Graph, PORT, STATUS, State, Status, WIDTH, table};

/// How far the halo of the selected card reaches beyond it.
const HALO: f32 = 5.0;
/// The size of the small table that marks a saved frame.
const ICON: Size = Size::new(15.0, 11.0);
/// The side of the "save output" box.
const TICK: f32 = 12.0;

/// The graph as `state` views it.
pub fn draw(
    graph: &Graph<'_>,
    state: &State,
    renderer: &Renderer,
    theme: &Theme,
    bounds: Rectangle,
) -> Vec<canvas::Geometry> {
    let palette = theme.palette();
    let Palette {
        background,
        primary,
        danger,
        success,
        ..
    } = palette;
    let muted = background
        .strongest
        .color;
    let nodes = &graph.pipeline.nodes;
    // halfway to the card behind it, for what a Check found and no Run
    let pale = |color: Color| {
        let card = background
            .base
            .color;
        Color {
            r: (color.r + card.r) / 2.0,
            g: (color.g + card.g) / 2.0,
            b: (color.b + card.b) / 2.0,
            a: color.a,
        }
    };

    let mut frame = Frame::new(renderer, bounds.size());
    frame.fill_rectangle(
        Point::ORIGIN,
        bounds.size(),
        background
            .weak
            .color,
    );
    frame.translate(state.offset);
    frame.scale(state.scale());

    for node in nodes {
        for (slot, id) in node
            .inputs
            .iter()
            .enumerate()
        {
            let source = nodes
                .iter()
                .position(|source| source.id == *id);
            if let Some(source) = source {
                wire(&mut frame, graph.output(source), input(node, slot), muted);
            }
        }
    }
    if let Some(Drag::Wire(from, end)) = state.drag {
        wire(&mut frame, graph.output(from), end, primary.base.color);
    }

    for (index, node) in nodes
        .iter()
        .enumerate()
    {
        let card = graph.card(index);
        let Point { x, y } = card.position();
        let outline = Path::rounded_rectangle(card.position(), card.size(), 5.0.into());
        let (border, width) = match graph.status[index] {
            Status::Unknown => (muted, 1.0),
            Status::Checked => (pale(success.strong.color), STATUS),
            Status::Rejected => (pale(danger.strong.color), STATUS),
            Status::Ran => (success.strong.color, STATUS),
            Status::Failed => (danger.strong.color, STATUS),
        };
        // inset by half its width, a border grows into the card
        let inset = (width - 1.0) / 2.0;
        let edge = Path::rounded_rectangle(
            card.position() + Vector::new(inset, inset),
            card.size() - Size::new(2.0 * inset, 2.0 * inset),
            (5.0 - inset).into(),
        );
        if graph.selected == Some(index) {
            // a halo, as the border is the status
            let halo = Path::rounded_rectangle(
                card.position() - Vector::new(HALO, HALO),
                card.size() + Size::new(2.0 * HALO, 2.0 * HALO),
                (5.0 + HALO).into(),
            );
            frame.fill(
                &halo,
                primary
                    .base
                    .color
                    .scale_alpha(0.4),
            );
        }
        frame.fill(
            &outline,
            background
                .base
                .color,
        );
        frame.stroke(
            &edge,
            Stroke::default()
                .with_width(width)
                .with_color(border),
        );

        let ink = background.base.text;
        let mut room = card.width - 24.0;
        if graph.saved[index] {
            // a small table, for the frame that is saved
            let icon = Rectangle::new(
                Point::new(x + card.width - 12.0 - ICON.width, y + 11.0),
                ICON,
            );
            room -= ICON.width + 8.0;
            let lines = Path::new(|path| {
                path.rectangle(icon.position(), icon.size());
                path.move_to(icon.position() + Vector::new(0.0, 4.0));
                path.line_to(icon.position() + Vector::new(icon.width, 4.0));
                path.move_to(icon.position() + Vector::new(5.0, 4.0));
                path.line_to(icon.position() + Vector::new(5.0, icon.height));
            });
            frame.stroke(&lines, Stroke::default().with_color(ink));
        }
        label(
            &mut frame,
            &node.id,
            Point::new(x + 12.0, y + 6.0),
            16.0,
            ink,
            room,
        );
        label(
            &mut frame,
            &node.block,
            Point::new(x + 12.0, y + 26.0),
            12.0,
            muted,
            WIDTH - 24.0,
        );

        for (slot, name) in graph
            .inputs(node)
            .iter()
            .enumerate()
        {
            let port = input(node, slot);
            let name = name
                .as_str()
                .unwrap_or_default();
            frame.fill(&Path::circle(port, PORT), muted);
            label(
                &mut frame,
                name,
                port + Vector::new(12.0, -8.0),
                12.0,
                muted,
                WIDTH - 24.0,
            );
        }

        frame.fill(&Path::circle(graph.output(index), PORT), muted);

        let row = graph.save(index);
        let tick = Rectangle::new(
            row.position() + Vector::new(12.0, 5.0),
            Size::new(TICK, TICK),
        );
        let is_saved = graph
            .pipeline
            .outputs
            .contains(&node.id);
        if is_saved {
            frame.fill_rectangle(tick.position(), tick.size(), primary.base.color);
            let mark = Path::new(|path| {
                path.move_to(tick.position() + Vector::new(2.5, 6.0));
                path.line_to(tick.position() + Vector::new(5.0, 8.5));
                path.line_to(tick.position() + Vector::new(9.5, 3.0));
            });
            frame.stroke(
                &mark,
                Stroke::default()
                    .with_width(1.5)
                    .with_color(primary.base.text),
            );
        } else {
            frame.stroke_rectangle(
                tick.position(),
                tick.size(),
                Stroke::default().with_color(muted),
            );
        }
        label(
            &mut frame,
            "save output",
            tick.position() + Vector::new(TICK + 6.0, -2.0),
            12.0,
            muted,
            WIDTH - 24.0,
        );
    }

    // on its own layer, as what a clip draws goes under the rest of a frame
    let mut tables = Frame::new(renderer, bounds.size());
    tables.translate(state.offset);
    tables.scale(state.scale());
    let unmoved = table::View::default();
    for (index, node) in nodes
        .iter()
        .enumerate()
    {
        let Some(body) = graph.body(index) else {
            continue;
        };
        let card = graph.card(index);
        let corner = Point::new(card.x + card.width, card.y + card.height);

        if let Some((_, table)) = graph.shown(index) {
            let view = state
                .views
                .get(&node.id)
                .map_or(&unmoved, |(view, _)| view);
            let clip = Rectangle::new(state.project(body.position()), body.size() * state.scale());
            // a clip starts from the canvas, not the view, and is not cut to it
            if let Some(clip) = clip.intersection(&Rectangle::with_size(bounds.size())) {
                tables.with_clip(clip, |tables| {
                    tables.translate(state.offset);
                    tables.scale(state.scale());
                    view.draw(tables, table, body, palette);
                });
            }
            tables.fill_text(canvas::Text {
                content: view.caption(table, body.size()),
                position: Point::new(corner.x - 16.0, card.y + 26.0),
                size: 12.0.into(),
                color: muted,
                align_x: text::Alignment::Right,
                ..canvas::Text::default()
            });
        }

        let grip = Path::new(|path| {
            for inset in [3.0, 7.0] {
                path.move_to(corner - Vector::new(STATUS + inset, STATUS + 1.0));
                path.line_to(corner - Vector::new(STATUS + 1.0, STATUS + inset));
            }
        });
        tables.stroke(&grip, Stroke::default().with_color(muted));
    }

    vec![
        frame.into_geometry(),
        tables.into_geometry(),
    ]
}

/// Draws a wire that leaves and enters level.
fn wire(
    frame: &mut Frame,
    from: Point,
    to: Point,
    color: Color,
) {
    let bend = Vector::new(((to.x - from.x).abs() / 2.0).max(40.0), 0.0);
    let path = Path::new(|path| {
        path.move_to(from);
        path.bezier_curve_to(from + bend, to - bend, to);
    });
    frame.stroke(
        &path,
        Stroke::default()
            .with_width(2.0)
            .with_color(color),
    );
}

/// Writes a line on a card, cut short at `width`.
fn label(
    frame: &mut Frame,
    content: &str,
    position: Point,
    size: f32,
    color: Color,
    width: f32,
) {
    frame.fill_text(canvas::Text {
        content: content.to_owned(),
        position,
        size: size.into(),
        color,
        max_width: width,
        wrapping: text::Wrapping::None,
        ellipsis: text::Ellipsis::End,
        ..canvas::Text::default()
    });
}
