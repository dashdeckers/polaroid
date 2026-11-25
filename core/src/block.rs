use polars::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// A Block is a frame-level operation, with its fields as its parameters.
pub trait Block<const N: usize = 1>: DeserializeOwned + JsonSchema {
    const INPUTS: [&'static str; N] = ["frame"; N];

    /// Its icon: typst that draws a small glyph in `ink`, `grey`, `faint`, `rule` and `paper`.
    const ICON: &'static str = ICON;

    /// One or more frames in, one frame out.
    fn apply(
        self,
        inputs: [LazyFrame; N],
    ) -> PolarsResult<LazyFrame>;

    /// The column of the frame it gives whose images are to be shown, if it has such a one.
    fn images(&self) -> Option<&str> {
        None
    }
}

/// The icon of a Block that has none of its own: an outlined block on a wire, 20 by 14 pt.
pub(crate) const ICON: &str = r#"#box(width: 20pt, height: 14pt, {
  place(line(start: (0pt, 7pt), end: (5pt, 7pt), stroke: 0.9pt + ink))
  place(line(start: (15pt, 7pt), end: (20pt, 7pt), stroke: 0.9pt + ink))
  place(dx: 5pt, dy: 2pt, rect(width: 10pt, height: 10pt, radius: 1.5pt, stroke: 0.9pt + ink))
})"#;

/// `frame`, with `check` to look at its columns once the plan resolves, which is before a run.
pub(crate) fn checked(
    frame: LazyFrame,
    check: impl Fn(&Schema) -> PolarsResult<()> + Send + Sync + 'static,
) -> LazyFrame {
    let checked = PlanCallback::new(move |(plans, schemas): (Vec<DslPlan>, Vec<SchemaRef>)| {
        let (plan, schema) = plans
            .into_iter()
            .zip(&schemas)
            .next()
            .ok_or_else(|| polars_err!(ComputeError: "a check reads a frame"))?;
        check(schema)?;
        Ok(plan)
    });
    frame.pipe_with_schema(checked)
}

/// The name of a column of the frame that a Block reads.
#[derive(Deserialize, JsonSchema)]
pub struct ColumnName(pub String);
