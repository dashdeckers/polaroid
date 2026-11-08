//! Every block by the name a pipeline file calls it, and the catalog a GUI
//! builds its forms from.

use std::collections::BTreeMap;

use polars::prelude::*;
use schemars::generate::SchemaSettings;
use schemars::{Schema, SchemaGenerator};
use serde_json::{Value, json};

use crate::engine::Pipeline;
use crate::{Block, frames, io};

/// What the engine needs of a block, its type erased.
pub(crate) struct Entry {
    /// The frames it reads, by role, in order.
    pub inputs: &'static [&'static str],
    schema: fn(&mut SchemaGenerator) -> Schema,
    build: fn(Value, Vec<LazyFrame>) -> PolarsResult<LazyFrame>,
}

impl Entry {
    fn of<B: Block>() -> Self {
        Entry {
            inputs: B::INPUTS,
            schema: |generator| generator.subschema_for::<B>(),
            build: |args, inputs| {
                let block: B = serde_json::from_value(args)
                    .map_err(|e| polars_err!(ComputeError: "args: {e}"))?;
                block.apply_all(inputs)
            },
        }
    }

    /// The block filled in from `args`, applied to `inputs`.
    pub fn build(
        &self,
        args: Value,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        (self.build)(args, inputs)
    }
}

/// The blocks a pipeline file may name.
#[derive(Default)]
pub struct Registry {
    blocks: BTreeMap<&'static str, Entry>,
}

impl Registry {
    /// The sources and the standard Polars repertoire.
    pub fn with_polars() -> Self {
        Self::default()
            // sources
            .register::<io::ScanParquetDir>("scan_parquet_dir")
            .register::<io::ReadSheet>("read_sheet")
            .register::<io::ScanCsv>("scan_csv")
            // the Polars repertoire
            .register::<frames::Select>("select")
            .register::<frames::WithColumns>("with_columns")
            .register::<frames::Filter>("filter")
            .register::<frames::GroupBy>("group_by")
            .register::<frames::Join>("join")
            .register::<frames::Concat>("concat")
            .register::<frames::Sort>("sort")
            .register::<frames::Unique>("unique")
            .register::<frames::Unpivot>("unpivot")
            .register::<frames::Explode>("explode")
            .register::<frames::Drop>("drop")
            .register::<frames::DropNulls>("drop_nulls")
            .register::<frames::Rename>("rename")
            .register::<frames::Limit>("limit")
            .register::<frames::Cache>("cache")
    }

    /// `B`, as pipeline files call it: `name`. Panics if the name is taken.
    pub fn register<B: Block>(
        mut self,
        name: &'static str,
    ) -> Self {
        let taken = self
            .blocks
            .insert(name, Entry::of::<B>())
            .is_some();
        assert!(!taken, "two blocks are called '{name}'");
        self
    }

    pub(crate) fn get(
        &self,
        name: &str,
    ) -> Option<&Entry> {
        self.blocks
            .get(name)
    }

    /// Everything an editor needs to offer every block: each one's inputs and
    /// the JSON Schema of its args, documentation included, beside the schema
    /// of a pipeline file. Polars' expressions are among the definitions.
    pub fn catalog(&self) -> Value {
        let mut generator = SchemaSettings::draft2020_12().into_generator();
        let blocks: serde_json::Map<String, Value> = self
            .blocks
            .iter()
            .map(|(name, entry)| {
                let args = (entry.schema)(&mut generator);
                (
                    name.to_string(),
                    json!({"inputs": entry.inputs, "args": args}),
                )
            })
            .collect();
        let pipeline = generator.subschema_for::<Pipeline>();

        json!({
            "polars": polars::VERSION,
            "blocks": blocks,
            "pipeline": pipeline,
            "$defs": generator.definitions(),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_catalog_describes_every_block_and_the_expressions() {
        let registry = Registry::with_polars();
        let catalog = registry.catalog();
        assert_eq!(
            catalog["blocks"]
                .as_object()
                .unwrap()
                .len(),
            registry
                .blocks
                .len()
        );
        assert_eq!(
            catalog["blocks"]["join"]["inputs"],
            json!(["left", "right"])
        );
        assert!(catalog["$defs"]["Expr"].is_object());
    }

    #[test]
    #[should_panic(expected = "two blocks are called 'filter'")]
    fn a_name_is_one_block() {
        let _ = Registry::with_polars().register::<frames::Filter>("filter");
    }
}
