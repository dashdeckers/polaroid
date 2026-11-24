//! The standard Polars repertoire.

use polars::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::block::checked;
use crate::{Block, ColumnName};

const EXPLODE: ExplodeOptions = ExplodeOptions {
    empty_as_null: true,
    keep_nulls: true,
};

fn names(columns: &[String]) -> Selector {
    cols(
        columns
            .iter()
            .map(String::as_str),
    )
}

/// Selects columns from the frame: only the columns of `exprs` are left, in their order.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Select {
    /// Columns to select. Each expression gives a column named after the first column it
    /// reads, or as alias names it.
    pub exprs: Vec<Expr>,
}

impl Block for Select {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.select(self.exprs))
    }
}

/// Adds columns to the frame. An added column replaces an existing column of the same name.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WithColumns {
    /// Columns to add. Each expression gives a column named after the first column it
    /// reads, or as alias names it.
    pub exprs: Vec<Expr>,
}

impl Block for WithColumns {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.with_columns(self.exprs))
    }
}

/// Filters rows: keeps those where every one of `predicates` is true, in their order.
///
/// A row where one of them is false or null is discarded.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    /// Expressions that evaluate to a boolean for each row. A row is kept if all of them are true.
    pub predicates: Vec<Expr>,
}

impl Block for Filter {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(self
            .predicates
            .into_iter()
            .fold(lf, LazyFrame::filter))
    }
}

/// Groups the rows by `by` and computes `aggs` for each group: one row per group.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GroupBy {
    /// Columns to group by. Rows with the same values in all of them form a group.
    /// With none, all rows are one group.
    pub by: Vec<Expr>,
    /// Aggregations to compute for each group, such as a sum or a mean, each giving a column.
    /// An expression that does not aggregate gives the values of the group as a list.
    pub aggs: Vec<Expr>,
    /// Keep the groups in the order they first appear in the frame, which is slower.
    /// Otherwise their order is not guaranteed.
    #[serde(default)]
    pub maintain_order: bool,
}

impl Block for GroupBy {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        let groups = match self.maintain_order {
            true => lf.group_by_stable(self.by),
            false => lf.group_by(self.by),
        };
        Ok(groups.agg(self.aggs))
    }
}

/// Joins the `left` frame to the `right` frame on the rows where `left_on` equals `right_on`.
///
/// A column of `right` whose name `left` has too is suffixed `_right`. The order of the rows is
/// not guaranteed.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Join {
    /// Join columns of the left frame. Leave these and right_on empty for a Cross join.
    pub left_on: Vec<Expr>,
    /// Join columns of the right frame, each matched to the one at its position in left_on.
    pub right_on: Vec<Expr>,
    /// Join strategy. Inner: the rows that match in both frames. Left: all rows of the left
    /// frame, with nulls where the right has no match. Right: all rows of the right frame, with
    /// nulls where the left has no match. Full: all rows of both, each keeping its join columns.
    /// Semi: the left rows that have a match, without the columns of the right frame. Anti: the
    /// left rows that have no match. Cross: every left row beside every right row.
    pub how: How,
}

/// How the rows of two frames are joined on columns that are equal.
#[derive(Deserialize, JsonSchema)]
pub enum How {
    Inner,
    Left,
    Right,
    Full,
    Semi,
    Anti,
    Cross,
}

impl Block<2> for Join {
    const INPUTS: [&'static str; 2] = ["left", "right"];

    fn apply(
        self,
        [left, right]: [LazyFrame; 2],
    ) -> PolarsResult<LazyFrame> {
        let how = match self.how {
            How::Inner => JoinType::Inner,
            How::Left => JoinType::Left,
            How::Right => JoinType::Right,
            How::Full => JoinType::Full,
            How::Semi => JoinType::Semi,
            How::Anti => JoinType::Anti,
            How::Cross => JoinType::Cross,
        };
        Ok(left.join(right, self.left_on, self.right_on, JoinArgs::new(how)))
    }
}

/// Stacks the rows of the `top` frame over those of the `bottom` frame.
///
/// Both must have the same columns, in the same order and of the same types, unless `diagonal`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Concat {
    /// Take the columns of both frames, by name: one that a frame lacks is null in its rows.
    #[serde(default)]
    pub diagonal: bool,
}

impl Block<2> for Concat {
    const INPUTS: [&'static str; 2] = ["top", "bottom"];

    fn apply(
        self,
        [top, bottom]: [LazyFrame; 2],
    ) -> PolarsResult<LazyFrame> {
        let args = UnionArgs {
            diagonal: self.diagonal,
            ..Default::default()
        };
        concat([top, bottom], args)
    }
}

/// Sorts the frame by `by`. Nulls come first.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sort {
    /// Columns to sort by: by the first, then by the next among the rows equal in it.
    pub by: Vec<Expr>,
    /// Sort in descending order. Nulls still come first.
    #[serde(default)]
    pub descending: bool,
}

impl Block for Sort {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        let options = SortMultipleOptions::default().with_order_descending(self.descending);
        Ok(lf.sort_by_exprs(self.by, options))
    }
}

/// Drops duplicate rows: one row is left of those that are equal in `subset`.
///
/// Which of them is kept, and the order of the rows, is not guaranteed.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Unique {
    /// Columns to consider when identifying duplicate rows. With none, all columns are used.
    #[serde(default)]
    pub subset: Option<Vec<String>>,
}

impl Block for Unique {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        let subset = self
            .subset
            .as_deref()
            // no columns named is every column
            .filter(|subset| !subset.is_empty())
            .map(names);
        Ok(lf.unique(subset, UniqueKeepStrategy::Any))
    }
}

/// Unpivots the frame from wide to long format.
///
/// The columns of `on` are stacked into rows, their names in one column and their values in
/// another, beside the `index` columns.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Unpivot {
    /// Columns to stack into rows. With none, all columns that are not in index are used.
    pub on: Vec<String>,
    /// Columns to keep as identifiers, repeated beside each stacked value.
    pub index: Vec<String>,
    /// Name of the column that holds the names of the stacked columns. Defaults to "variable".
    #[serde(default)]
    pub variable_name: Option<String>,
    /// Name of the column that holds their values. Defaults to "value".
    #[serde(default)]
    pub value_name: Option<String>,
}

impl Block for Unpivot {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.unpivot(UnpivotArgsDSL {
            // no columns named is every other column
            on: (!self.on.is_empty()).then(|| names(&self.on)),
            index: names(&self.index),
            variable_name: self
                .variable_name
                .map(Into::into),
            value_name: self
                .value_name
                .map(Into::into),
        }))
    }
}

/// Explodes the frame to long format: one row per element of the lists in `columns`.
///
/// The other columns are repeated. An empty list or a null gives one row with a null.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Explode {
    /// Columns to explode, at least one, of the List or Array data type. Several explode
    /// together, so their lists must be equally long in each row.
    pub columns: Vec<String>,
}

impl Block for Explode {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.explode(names(&self.columns), EXPLODE))
    }
}

/// Removes `columns` from the frame.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Drop {
    /// Names of the columns to remove. A name that the frame does not have is an error.
    pub columns: Vec<String>,
}

impl Block for Drop {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.drop(names(&self.columns)))
    }
}

/// Drops the rows that have a null in one or more of `columns`.
///
/// The order of the remaining rows is kept.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DropNulls {
    /// Columns in which nulls are looked for. With none, no row is dropped.
    pub columns: Vec<String>,
}

impl Block for DropNulls {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.drop_nulls(Some(names(&self.columns))))
    }
}

/// Renames columns: each of `existing` to the name at its position in `new`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rename {
    /// Names of the columns to rename. A name that the frame does not have is an error.
    pub existing: Vec<String>,
    /// Their new names, as many and in the same order.
    pub new: Vec<String>,
}

impl Block for Rename {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        polars_ensure!(
            self.existing.len() == self.new.len(),
            ComputeError: "{} columns to rename, {} new names", self.existing.len(), self.new.len()
        );
        Ok(lf.rename(self.existing, self.new, true))
    }
}

/// Gets the first `n` rows.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Limit {
    /// Number of rows to return. A frame of fewer rows is returned whole.
    pub n: IdxSize,
}

impl Block for Limit {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.limit(self.n))
    }
}

/// Gets `n` rows picked at random, in the order they have in the frame.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    /// Number of rows to return. A frame of fewer rows is returned whole.
    pub n: IdxSize,
    /// Seed of the random pick: the same seed picks the same rows, and another seed others.
    #[serde(default)]
    pub seed: u64,
}

impl Block for Sample {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        // the rows that a shuffle of the row numbers gives one of the first `n` numbers
        let shuffled = int_range(lit(0), len(), 1, IDX_DTYPE).shuffle(Some(self.seed));
        Ok(lf.filter(shuffled.lt(lit(self.n))))
    }
}

/// Shows the images of a column, as an editor does on the card of the node.
///
/// The nodes that read this one get the frame it reads.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewImages {
    /// Column of the images to show, of binary values such as the images of a plot.
    pub column: ColumnName,
}

impl Block for ViewImages {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        let column = self.column.0;
        Ok(checked(lf, move |schema| {
            let dtype = schema.try_get(&column)?;
            polars_ensure!(
                dtype.is_binary(),
                SchemaMismatch: "column '{column}' holds {dtype}, which is no image"
            );
            Ok(())
        }))
    }

    fn images(&self) -> Option<&str> {
        Some(&self.column.0)
    }
}

/// Puts the columns of the `right` frame beside those of the `left` frame, row by row.
///
/// The shorter frame is filled with nulls. No two columns may have the same name.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConcatHorizontal {}

impl Block<2> for ConcatHorizontal {
    const INPUTS: [&'static str; 2] = ["left", "right"];

    fn apply(
        self,
        [left, right]: [LazyFrame; 2],
    ) -> PolarsResult<LazyFrame> {
        concat_lf_horizontal([left, right], HConcatOptions::default())
    }
}

/// Gets `length` rows from the row at `offset` on.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Slice {
    /// Row to start at: 0 is the first row, and a negative number counts from the end, so that
    /// -1 is the last row.
    pub offset: i64,
    /// Number of rows to return. If fewer are left, those are returned.
    pub length: IdxSize,
}

impl Block for Slice {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.slice(self.offset, self.length))
    }
}

/// Gets the last `n` rows.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Tail {
    /// Number of rows to return. A frame of fewer rows is returned whole.
    pub n: IdxSize,
}

impl Block for Tail {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.tail(self.n))
    }
}

/// Gets the `k` rows that are largest by `by`, largest first. Nulls are never among them.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TopK {
    /// Number of rows to return. A frame of fewer rows is returned whole, sorted.
    pub k: IdxSize,
    /// Columns to compare the rows by: by the first, then by the next among the rows equal in it.
    pub by: Vec<Expr>,
}

impl Block for TopK {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.top_k(self.k, self.by, SortMultipleOptions::default()))
    }
}

/// Gets the `k` rows that are smallest by `by`, smallest first. Nulls are never among them.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BottomK {
    /// Number of rows to return. A frame of fewer rows is returned whole, sorted.
    pub k: IdxSize,
    /// Columns to compare the rows by: by the first, then by the next among the rows equal in it.
    pub by: Vec<Expr>,
}

impl Block for BottomK {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.bottom_k(self.k, self.by, SortMultipleOptions::default()))
    }
}

fn index() -> String {
    "index".to_owned()
}

/// Adds a column that numbers the rows, as the first column of the frame.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WithRowIndex {
    /// Name of the column to add. A column of that name must not be there yet.
    #[serde(default = "index")]
    pub name: String,
    /// Number of the first row.
    #[serde(default)]
    pub offset: IdxSize,
}

impl Block for WithRowIndex {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.with_row_index(self.name, Some(self.offset)))
    }
}

/// Puts the fields of the structs in `columns` in their place, each as a column of its own.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Unnest {
    /// Columns to unnest, at least one, of the Struct data type.
    pub columns: Vec<String>,
    /// Text between the name of a column and the name of its field, which together name the
    /// new column. Without it, the new column is named as the field alone.
    #[serde(default)]
    pub separator: Option<String>,
}

impl Block for Unnest {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        polars_ensure!(
            !self.columns.is_empty(),
            ComputeError: "unnest takes at least one column"
        );
        let separator = self
            .separator
            .map(Into::into);
        Ok(lf.unnest(names(&self.columns), separator))
    }
}

fn backward() -> AsofStrategy {
    AsofStrategy::Backward
}

fn yes() -> bool {
    true
}

/// Joins each row of the `left` frame to the row of the `right` frame whose key is nearest.
///
/// Both frames must be sorted by their key. All rows of `left` are kept, with nulls where
/// `right` has no match. A column of `right` whose name `left` has too is suffixed `_right`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JoinAsof {
    /// Key of the left frame, such as a time: of numbers, dates or times.
    pub left_on: Expr,
    /// Key of the right frame, of the data type of left_on.
    pub right_on: Expr,
    /// Columns of the left frame that must equal those of right_by before the keys are matched.
    #[serde(default)]
    pub left_by: Vec<String>,
    /// Columns of the right frame, each matched to the one at its position in left_by.
    #[serde(default)]
    pub right_by: Vec<String>,
    /// Which row matches. Backward: the last whose key is at most that of the left row.
    /// Forward: the first whose key is at least it. Nearest: the one whose key is closest.
    #[serde(default = "backward")]
    pub strategy: AsofStrategy,
    /// How far apart two keys of numbers may be to match. Without it, however far.
    #[serde(default)]
    pub tolerance: Option<f64>,
    /// How far apart two keys of dates or times may be to match, such as 5m, 2h15m or 1d6h.
    #[serde(default)]
    pub tolerance_time: Option<String>,
    /// Whether a row whose key equals that of the left row matches.
    #[serde(default = "yes")]
    pub allow_exact_matches: bool,
}

impl Block<2> for JoinAsof {
    const INPUTS: [&'static str; 2] = ["left", "right"];

    fn apply(
        self,
        [left, right]: [LazyFrame; 2],
    ) -> PolarsResult<LazyFrame> {
        polars_ensure!(
            self.tolerance.is_none() || self.tolerance_time.is_none(),
            ComputeError: "join_asof takes a tolerance or a tolerance_time, not both"
        );
        let by = |columns: Vec<String>| {
            let columns: Vec<PlSmallStr> = columns
                .into_iter()
                .map(Into::into)
                .collect();
            (!columns.is_empty()).then_some(columns)
        };
        let options = AsOfOptions {
            strategy: self.strategy,
            tolerance: self
                .tolerance
                .map(Scalar::from),
            tolerance_str: self
                .tolerance_time
                .map(Into::into),
            left_by: by(self.left_by),
            right_by: by(self.right_by),
            allow_eq: self.allow_exact_matches,
            check_sortedness: true,
        };
        let how = JoinType::AsOf(Box::new(options));
        Ok(left.join(right, [self.left_on], [self.right_on], JoinArgs::new(how)))
    }
}

/// What a text such as 1h30m says of a length of time, or of a number of rows as in 10i.
fn duration(text: &str) -> PolarsResult<Duration> {
    Duration::try_parse(text)
}

fn left() -> ClosedWindow {
    ClosedWindow::Left
}

fn right() -> ClosedWindow {
    ClosedWindow::Right
}

fn labelled() -> Label {
    Label::Left
}

fn window_bound() -> StartBy {
    StartBy::WindowBound
}

/// Groups the rows into windows of time and computes `aggs` for each: one row per window.
///
/// The frame must be sorted by `index_column`, within each group of `by` if there are any. A
/// length of time is written as 1h, 30m, 1d12h, 1w, 1mo or 1y, and a number of rows as 10i.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GroupByDynamic {
    /// Column of dates, times or whole numbers by which the rows fall into windows.
    pub index_column: crate::ColumnName,
    /// How far the start of a window is from the start of the one before.
    pub every: String,
    /// How long a window is. Without it, as long as every, so that no two windows overlap.
    #[serde(default)]
    pub period: Option<String>,
    /// By how much the start of every window is moved. Without it, by nothing.
    #[serde(default)]
    pub offset: Option<String>,
    /// Columns to group by first, so that each group has windows of its own.
    #[serde(default)]
    pub by: Vec<Expr>,
    /// Aggregations to compute for each window, such as a sum or a mean, each giving a column.
    pub aggs: Vec<Expr>,
    /// Which ends of a window belong to it.
    #[serde(default = "left")]
    pub closed: ClosedWindow,
    /// What a window has in index_column: its start, its end, or the first value in it.
    #[serde(default = "labelled")]
    pub label: Label,
    /// Where the first window starts. WindowBound: at a whole multiple of every. DataPoint: at
    /// the first value. A weekday: on that day before the first value, for windows of weeks.
    #[serde(default = "window_bound")]
    pub start_by: StartBy,
    /// Add the start and the end of each window as columns _lower_boundary and _upper_boundary.
    #[serde(default)]
    pub include_boundaries: bool,
}

impl Block for GroupByDynamic {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        let every = duration(&self.every)?;
        let options = DynamicGroupOptions {
            every,
            period: match &self.period {
                Some(period) => duration(period)?,
                None => every,
            },
            offset: duration(
                self.offset
                    .as_deref()
                    .unwrap_or("0ns"),
            )?,
            label: self.label,
            include_boundaries: self.include_boundaries,
            closed_window: self.closed,
            start_by: self.start_by,
            ..Default::default()
        };
        // a plain column, as Polars looks at the frame for anything else while the plan is built
        let index = col(self.index_column.0);
        Ok(lf
            .group_by_dynamic(index, self.by, options)
            .agg(self.aggs))
    }
}

/// Computes `aggs` over the window of time that ends at each row: as many rows out as in.
///
/// The frame must be sorted by `index_column`, within each group of `by` if there are any. A
/// length of time is written as 1h, 30m, 1d12h, 1w, 1mo or 1y, and a number of rows as 10i.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rolling {
    /// Column of dates, times or whole numbers by which the rows fall into windows.
    pub index_column: crate::ColumnName,
    /// How long a window is.
    pub period: String,
    /// Where a window starts, from the value of its row. Without it, a period before, so that
    /// the window ends at its row.
    #[serde(default)]
    pub offset: Option<String>,
    /// Columns to group by first, so that a window holds rows of its group alone.
    #[serde(default)]
    pub by: Vec<Expr>,
    /// Aggregations to compute for each window, such as a sum or a mean, each giving a column.
    pub aggs: Vec<Expr>,
    /// Which ends of a window belong to it.
    #[serde(default = "right")]
    pub closed: ClosedWindow,
}

impl Block for Rolling {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        let period = duration(&self.period)?;
        let options = RollingGroupOptions {
            period,
            offset: match &self.offset {
                Some(offset) => duration(offset)?,
                None => -period,
            },
            closed_window: self.closed,
            ..Default::default()
        };
        let index = col(self.index_column.0);
        Ok(lf
            .rolling(index, self.by, options)
            .agg(self.aggs))
    }
}

/// How the values that fall into one cell of a pivot are made one.
#[derive(Deserialize, JsonSchema)]
pub enum Aggregate {
    First,
    Last,
    Sum,
    Mean,
    Median,
    Min,
    Max,
    /// How many there are.
    Len,
}

fn first() -> Aggregate {
    Aggregate::First
}

/// Pivots the frame from long to wide format: a column for each value of `on`, filled with
/// `values`, and one row for each combination of `index`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pivot {
    /// Column whose values become the names of the new columns.
    pub on: crate::ColumnName,
    /// Values of on to make columns of, in this order, as text. The rows of other values are
    /// left out. They are given, as the columns of a frame are known before it is computed.
    pub on_columns: Vec<String>,
    /// Columns that tell the rows of the result apart.
    pub index: Vec<String>,
    /// Columns whose values fill the new columns, at least one. With several, a new column is
    /// named as the values column and the value of on, with an underscore between.
    pub values: Vec<String>,
    /// What to make of several values that fall into one cell.
    #[serde(default = "first")]
    pub aggregate: Aggregate,
}

impl Block for Pivot {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        polars_ensure!(
            !self.on_columns.is_empty() && !self.values.is_empty(),
            ComputeError: "pivot takes at least one of on_columns and one of values"
        );
        let on = self.on.0;
        // as text, the values are named whatever on holds
        let named = Column::new(on.as_str().into(), self.on_columns);
        let aggregate = match self.aggregate {
            Aggregate::First => element().first(),
            Aggregate::Last => element().last(),
            Aggregate::Sum => element().sum(),
            Aggregate::Mean => element().mean(),
            Aggregate::Median => element().median(),
            Aggregate::Min => element().min(),
            Aggregate::Max => element().max(),
            Aggregate::Len => element().len(),
        };
        Ok(lf
            .with_column(col(on.as_str()).cast(DataType::String))
            .pivot(
                names(std::slice::from_ref(&on)),
                Arc::new(DataFrame::new_infer_height(vec![named])?),
                names(&self.index),
                names(&self.values),
                aggregate,
                true,
                "_".into(),
                polars::frame::PivotColumnNaming::Auto,
            ))
    }
}
