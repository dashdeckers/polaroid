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

/// Just these expressions' columns.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Select {
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

/// Every column, and these expressions' beside or in place of them.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WithColumns {
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

/// The rows meeting every one of `predicates`, applied one filter at a time.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Filter {
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

/// One row per group of `by`, its `aggs` beside.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GroupBy {
    pub by: Vec<Expr>,
    pub aggs: Vec<Expr>,
    /// Keep the groups in the order they first appear.
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

/// The `left` frame joined to the `right` where `left_on` matches `right_on`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Join {
    pub left_on: Vec<Expr>,
    pub right_on: Vec<Expr>,
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

/// The `top` frame's rows, then the `bottom` frame's.
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

/// Ascending by `by`, the first deciding, or descending.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sort {
    pub by: Vec<Expr>,
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

/// One row per distinct combination of `subset`, or of every column.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Unique {
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
            .map(names);
        Ok(lf.unique(subset, UniqueKeepStrategy::Any))
    }
}

/// Each of `on` becomes a row: its name in `variable_name`, its value in `value_name`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Unpivot {
    pub on: Vec<String>,
    pub index: Vec<String>,
    #[serde(default)]
    pub variable_name: Option<String>,
    #[serde(default)]
    pub value_name: Option<String>,
}

impl Block for Unpivot {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.unpivot(UnpivotArgsDSL {
            on: Some(names(&self.on)),
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

/// One row per element of the list `columns`, which explode together.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Explode {
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

/// Everything but `columns`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Drop {
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

/// The rows where none of `columns` is null.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DropNulls {
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

/// Each of `existing` renamed to its counterpart in `new`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rename {
    pub existing: Vec<String>,
    pub new: Vec<String>,
}

impl Block for Rename {
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        Ok(lf.rename(self.existing, self.new, true))
    }
}

/// The first `n` rows.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Limit {
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
