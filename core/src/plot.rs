//! Plots: blocks that give each row an image of the figure drawn of its values.

use polars::prelude::*;
use rustd3_figure::kurbo::Size;
use rustd3_figure::{Error, Figure, Target, lower, pdf, raster, svg};
use schemars::JsonSchema;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::{Block, ColumnName};

/// Device pixels to each pixel of a PNG, as a dense screen shows it.
const SCALE: f64 = 2.0;

/// A Plot draws a figure of the values of some columns, with its fields as its parameters.
///
/// As a Block it adds a column `plot` of images: of every row, if the columns hold lists, or of
/// every group of `by` otherwise, whose values it gathers into lists.
pub trait Plot: DeserializeOwned + JsonSchema + Send + Sync + 'static {
    /// The columns whose values it draws.
    fn columns(&self) -> Vec<&str>;

    /// What it takes as every Plot does.
    fn options(&self) -> &Options;

    /// The figure of one image: its columns in `values` as numbers, without nulls, in one chunk.
    fn draw<'a>(
        &self,
        values: &'a DataFrame,
    ) -> PolarsResult<Figure<'a>>;
}

fn width() -> f64 {
    640.0
}

fn height() -> f64 {
    480.0
}

/// What every Plot takes beside its own fields.
#[derive(Deserialize, JsonSchema)]
pub struct Options {
    /// Columns to group by, for a plot of each group. With none, all rows are one plot.
    /// Columns that hold lists are drawn row by row, and take none.
    #[serde(default)]
    pub by: Vec<ColumnName>,
    /// Width of the figure in pixels.
    #[serde(default = "width")]
    pub width: f64,
    /// Height of the figure in pixels.
    #[serde(default = "height")]
    pub height: f64,
    /// Format of the image. Png is two pixels to each of the figure, and small however many
    /// values are drawn. Svg and Pdf stay sharp at any size, but grow with the values.
    #[serde(default)]
    pub format: Format,
}

/// The format of the image of a figure.
#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
pub enum Format {
    #[default]
    Png,
    Svg,
    Pdf,
}

impl Format {
    /// The image of `figure` at `size`.
    fn image(
        self,
        figure: &Figure,
        size: Size,
    ) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Format::Svg => {
                let lowered = lower::lower(&figure.draw(size), svg::CAPS)?;
                svg::write(&lowered.list, &svg::Options::default())?.into_bytes()
            }
            Format::Png => {
                let target = Target::File { scale: SCALE };
                let (mut cache, held) = Default::default();
                let list = figure.draw_with(&mut cache, size, target, &held);
                raster::png::encode(&raster::render(&list, SCALE)?)?
            }
            Format::Pdf => pdf::write(&figure.draw(size), &pdf::Options::default())?,
        })
    }
}

impl<P: Plot> Block for P {
    fn apply(
        self,
        [frame]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        let plot = Arc::new(self);
        // what the columns hold tells how they are drawn, and is not known until the plan resolves
        let drawn = PlanCallback::new(move |(plans, schemas): (Vec<DslPlan>, Vec<SchemaRef>)| {
            let plans = plans
                .into_iter()
                .zip(&schemas)
                .next();
            let (plan, schema) =
                plans.ok_or_else(|| polars_err!(ComputeError: "a plot reads a frame"))?;
            Ok(drawn(&plot, LazyFrame::from(plan), schema)?.logical_plan)
        });
        Ok(frame.pipe_with_schema(drawn))
    }
}

/// `frame` with the images of `plot` beside the lists of values they are drawn of.
fn drawn<P: Plot>(
    plot: &Arc<P>,
    mut frame: LazyFrame,
    schema: &Schema,
) -> PolarsResult<LazyFrame> {
    let by = &plot.options().by;
    let columns = plot.columns();
    let listed: Vec<&str> = columns
        .iter()
        .copied()
        .filter(|name| {
            schema
                .get(name)
                .is_some_and(|dtype| dtype.is_list() || dtype.is_array())
        })
        .collect();
    if listed.len() == columns.len() {
        polars_ensure!(
            by.is_empty(),
            ComputeError:
                "{listed:?} hold lists, of which every row is a plot: take the columns out of `by`"
        );
    } else {
        polars_ensure!(
            listed.is_empty(),
            ComputeError: "{listed:?} hold lists, and the other columns of {columns:?} do not"
        );
        let by: Vec<Expr> = by
            .iter()
            .map(|by| col(by.0.as_str()))
            .collect();
        let values: Vec<Expr> = columns
            .iter()
            .map(|name| col(*name))
            .collect();
        frame = frame
            .group_by_stable(by)
            .agg(values);
    }

    // the numbers are for the figure alone: the columns stay as they are
    let numbers = DataType::List(Box::new(DataType::Float64));
    let mut values = columns
        .iter()
        .map(|name| col(*name).cast(numbers.clone()));
    let first = values
        .next()
        .ok_or_else(|| polars_err!(ComputeError: "a plot draws a column"))?;
    let others: Vec<Expr> = values.collect();
    let plot = plot.clone();
    let images = first.map_many(
        move |values| images(&*plot, values),
        &others,
        |_, _| Ok(Field::new("plot".into(), DataType::Binary)),
    );
    Ok(frame.with_column(images.alias("plot")))
}

/// The images of a Plot, one of each row of the lists of numbers that it draws.
fn images(
    plot: &impl Plot,
    values: &[Column],
) -> PolarsResult<Column> {
    let lists = values
        .iter()
        .map(Column::list)
        .collect::<PolarsResult<Vec<_>>>()?;
    let options = plot.options();
    let size = Size::new(options.width, options.height);

    let image = |row| {
        // a row where one of the lists is null has no image
        let Some(values) = lists
            .iter()
            .map(|list| Some(Column::from(list.get_as_series(row)?)))
            .collect()
        else {
            return Ok(None);
        };
        let mut values = DataFrame::new_infer_height(values)?.drop_nulls::<String>(None)?;
        values.rechunk_mut();
        let figure = plot.draw(&values)?;
        options
            .format
            .image(&figure, size)
            .map(Some)
            .map_err(|e| polars_err!(ComputeError: "{e}"))
    };
    let images = (0..values[0].len())
        .map(image)
        .collect::<PolarsResult<BinaryChunked>>()?;
    Ok(images.into_column())
}

/// The numbers of a column of the values that a Plot draws.
pub fn numbers<'a>(
    values: &'a DataFrame,
    name: &str,
) -> PolarsResult<&'a [f64]> {
    values
        .column(name)?
        .f64()?
        .cont_slice()
}
