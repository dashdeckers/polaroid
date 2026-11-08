//! The standard Polars repertoire.

use polars::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{Block, take};

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
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
        Ok(lf.select(self.exprs.clone()))
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
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
        Ok(lf.with_columns(self.exprs.clone()))
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
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
        Ok(self
            .predicates
            .iter()
            .cloned()
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
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
        let groups = match self.maintain_order {
            true => lf.group_by_stable(self.by.clone()),
            false => lf.group_by(self.by.clone()),
        };
        Ok(groups.agg(self.aggs.clone()))
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

impl Block for Join {
    const INPUTS: &'static [&'static str] = &["left", "right"];

    fn apply(
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [left, right] = take(inputs)?;
        Ok(left.join(
            right,
            self.left_on.clone(),
            self.right_on
                .clone(),
            JoinArgs::new(self.how.clone()),
        ))
    }
}

/// The `top` frame's rows, then the `bottom` frame's.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Concat {}

impl Block for Concat {
    const INPUTS: &'static [&'static str] = &["top", "bottom"];

    fn apply(
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [top, bottom] = take(inputs)?;
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
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
        let options = SortMultipleOptions::default().with_order_descending(self.descending);
        Ok(lf.sort_by_exprs(self.by.clone(), options))
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
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
        let subset = self
            .subset
            .as_deref()
            .map(names);
        Ok(lf.unique(subset, UniqueKeepStrategy::Any))
    }
}

/// Long form: each of `on` becomes a row, its name in `variable_name` and
/// its value in `value_name`, with `index` repeated beside.
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
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
        Ok(lf.unpivot(UnpivotArgsDSL {
            on: Some(names(&self.on)),
            index: names(&self.index),
            variable_name: self
                .variable_name
                .as_deref()
                .map(Into::into),
            value_name: self
                .value_name
                .as_deref()
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
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
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
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
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
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
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
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
        Ok(lf.rename(&self.existing, &self.new, true))
    }
}

/// Explicitly cache of a frame.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Cache {}

impl Block for Cache {
    fn apply(
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
        Ok(lf.cache())
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
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
        Ok(lf.limit(self.n))
    }
}
