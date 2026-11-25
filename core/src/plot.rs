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
    /// Its icon, as that of a Block.
    const ICON: &'static str = crate::block::ICON;

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

/// A Plot told from the args of a node, to draw a row of the frame it gave over again.
pub struct Drawer {
    columns: Vec<String>,
    /// How wide and tall its figures are, in pixels.
    pub size: (f64, f64),
    plot: Box<dyn Draw>,
}

/// What a [`Drawer`] keeps of a Plot.
trait Draw: Send + Sync {
    fn figure<'a>(
        &self,
        values: &'a DataFrame,
    ) -> PolarsResult<Figure<'a>>;
}

impl<P: Plot> Draw for P {
    fn figure<'a>(
        &self,
        values: &'a DataFrame,
    ) -> PolarsResult<Figure<'a>> {
        self.draw(values)
    }
}

impl std::fmt::Debug for Drawer {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("Drawer")
            .field("columns", &self.columns)
            .finish_non_exhaustive()
    }
}

impl Drawer {
    pub(crate) fn of(plot: impl Plot) -> Self {
        let options = plot.options();
        Drawer {
            columns: plot
                .columns()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            size: (options.width, options.height),
            plot: Box::new(plot),
        }
    }

    /// What the image of a row of `frame` was drawn of; none if the row has no image.
    pub fn values(
        &self,
        frame: &DataFrame,
        row: usize,
    ) -> PolarsResult<Option<DataFrame>> {
        // the one row alone is cast, of the many a frame may have
        let lists = self
            .columns
            .iter()
            .map(|name| {
                frame
                    .column(name)?
                    .slice(row as i64, 1)
                    .cast(&numbers_listed())
            })
            .collect::<PolarsResult<Vec<_>>>()?;
        values(&lists, 0)
    }

    /// The figure of what [`Self::values`] gave.
    pub fn figure<'a>(
        &self,
        values: &'a DataFrame,
    ) -> PolarsResult<Figure<'a>> {
        self.plot
            .figure(values)
    }
}

/// What a figure is drawn of: lists of numbers.
fn numbers_listed() -> DataType {
    DataType::List(Box::new(DataType::Float64))
}

/// The values of a row of lists of numbers, as a Plot draws them; none if one of them is null.
fn values(
    lists: &[Column],
    row: usize,
) -> PolarsResult<Option<DataFrame>> {
    let mut values = Vec::new();
    for list in lists {
        let Some(list) = list
            .list()?
            .get_as_series(row)
        else {
            return Ok(None);
        };
        values.push(Column::from(list));
    }
    let mut values = DataFrame::new_infer_height(values)?.drop_nulls::<String>(None)?;
    values.rechunk_mut();
    Ok(Some(values))
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
    const ICON: &'static str = P::ICON;

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

    fn images(&self) -> Option<&str> {
        Some("plot")
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
    let mut values = columns
        .iter()
        .map(|name| col(*name).cast(numbers_listed()));
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
    lists: &[Column],
) -> PolarsResult<Column> {
    let options = plot.options();
    let size = Size::new(options.width, options.height);

    let image = |row| {
        // a row where one of the lists is null has no image
        let Some(values) = values(lists, row)? else {
            return Ok(None);
        };
        let figure = plot.draw(&values)?;
        options
            .format
            .image(&figure, size)
            .map(Some)
            .map_err(|e| polars_err!(ComputeError: "{e}"))
    };
    let images = (0..lists[0].len())
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
