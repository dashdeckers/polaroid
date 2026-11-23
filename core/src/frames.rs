//! The standard Polars repertoire.

use polars::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Block;

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
    pub how: JoinType,
}

impl Block<2> for Join {
    const INPUTS: [&'static str; 2] = ["left", "right"];

    fn apply(
        self,
        [left, right]: [LazyFrame; 2],
    ) -> PolarsResult<LazyFrame> {
        Ok(left.join(right, self.left_on, self.right_on, JoinArgs::new(self.how)))
    }
}

/// Stacks the rows of the `top` frame over those of the `bottom` frame.
///
/// Both must have the same columns, in the same order and of the same types.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Concat {}

impl Block<2> for Concat {
    const INPUTS: [&'static str; 2] = ["top", "bottom"];

    fn apply(
        self,
        [top, bottom]: [LazyFrame; 2],
    ) -> PolarsResult<LazyFrame> {
        concat([top, bottom], UnionArgs::default())
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
