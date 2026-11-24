//! The standard plots.

use polars::prelude::*;
use rustd3_core::{scale, series};
use rustd3_figure::{Figure, axis, mark};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::ColumnName;
use crate::plot::{Options, Plot, numbers};

/// Draws a line through the points of `x` and `y`, in the order of the rows.
///
/// Adds a column `plot` with the image: one of all rows, one of each group of `by`, or one of
/// each row if `x` and `y` hold lists. The values that an image is drawn of stay beside it.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Line {
    /// Column of the horizontal positions, of numbers. A point where it or y is null is left out.
    pub x: ColumnName,
    /// Column of the vertical positions, of numbers.
    pub y: ColumnName,
    /// Title over the plot.
    #[serde(default)]
    pub title: Option<String>,
    #[serde(flatten)]
    pub options: Options,
}

/// What the one line of a figure is called, to give it the first colour of the theme.
const KEY: &str = "0";

impl Plot for Line {
    fn columns(&self) -> Vec<&str> {
        vec![&self.x.0, &self.y.0]
    }

    fn options(&self) -> &Options {
        &self.options
    }

    fn draw<'a>(
        &self,
        values: &'a DataFrame,
    ) -> PolarsResult<Figure<'a>> {
        // the columns as they lie in the frame, which is not copied
        let points = series::slices(numbers(values, &self.x.0)?, numbers(values, &self.y.0)?);
        // as the line that rustd3 presets, and zoomed and panned where it is shown live
        let mut figure = Figure::new();
        let (x, y) = (figure.fit(scale::linear()), figure.fit(scale::linear()));
        let palette = figure
            .get_theme()
            .get_palette()
            .to_vec();
        let color = figure.scale(scale::ordinal([KEY], palette));
        let panel = figure.panel(0, 0);
        panel
            .mark(mark::line(points, x, y).stroke(color, KEY))
            .axis(
                axis::bottom(x)
                    .label(self.x.0.as_str())
                    .minor(true),
            )
            .axis(
                axis::left(y)
                    .label(self.y.0.as_str())
                    .minor(true),
            )
            .zoom_x(x)
            .zoom_y(y);
        if let Some(title) = &self.title {
            panel.title(title.as_str());
        }
        Ok(figure)
    }
}
