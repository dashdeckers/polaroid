//! The standard plots.

use polars::prelude::*;
use rustd3_core::series;
use rustd3_figure::Figure;
use rustd3_figure::helpers::chart;
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
        let chart = chart::line(points)
            .x_label(self.x.0.as_str())
            .y_label(self.y.0.as_str());
        Ok(match &self.title {
            Some(title) => chart
                .title(title.as_str())
                .figure(),
            None => chart.figure(),
        })
    }
}
