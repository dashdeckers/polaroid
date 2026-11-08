use std::collections::BTreeMap;

use polars::prelude::*;
use schemars::generate::SchemaSettings;
use schemars::{Schema, SchemaGenerator};
use serde_json::{Value, json};

use crate::engine::Pipeline;
use crate::{Block, frames, io};

/// Type erased Block.
pub(crate) struct Entry {
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
                block.apply(inputs)
            },
        }
    }

    /// Builds the plan for one Node from its Block.
    pub fn build(
        &self,
        args: Value,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        (self.build)(args, inputs)
    }
}

/// The Blocks a Pipeline file may name.
#[derive(Default)]
pub struct Registry {
    blocks: BTreeMap<&'static str, Entry>,
}

impl Registry {
    /// The standard Polars repertoire.
    pub fn with_polars() -> Self {
        Self::default()
            // sources
            .register::<io::ScanParquet>("scan_parquet")
            // standard
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

    /// Adds a Block to the Registry.
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

    /// Returns a JSON description of the registered Blocks and the Pipeline.
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
