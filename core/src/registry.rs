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
    category: &'static str,
    schema: fn(&mut SchemaGenerator) -> Schema,
    build: fn(Value, Vec<LazyFrame>) -> PolarsResult<LazyFrame>,
}

impl Entry {
    fn of<B: Block<N>, const N: usize>(category: &'static str) -> Self {
        Entry {
            inputs: &B::INPUTS,
            category,
            schema: |generator| generator.subschema_for::<B>(),
            build: |args, inputs| {
                let block: B = serde_json::from_value(args)
                    .map_err(|e| polars_err!(ComputeError: "args: {e}"))?;
                block.apply(take(inputs)?)
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

/// The input frames as the array that a Block takes them in.
fn take<const N: usize>(inputs: Vec<LazyFrame>) -> PolarsResult<[LazyFrame; N]> {
    let got = inputs.len();
    inputs
        .try_into()
        .map_err(|_| polars_err!(ComputeError: "expected {N} input frames, got {got}"))
}

/// The Blocks a Pipeline file may name, each in a category to find it under.
pub struct Registry {
    blocks: BTreeMap<&'static str, Entry>,
    /// The categories in the order they were first named, and that of the next Block.
    categories: Vec<&'static str>,
    category: &'static str,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            blocks: BTreeMap::new(),
            categories: vec!["Other"],
            category: "Other",
        }
    }
}

impl Registry {
    /// The standard Polars repertoire.
    pub fn with_polars() -> Self {
        Self::default()
            .category("Read and write")
            .register::<io::ScanParquet, _>("scan_parquet")
            .register::<io::WriteParquet, _>("write_parquet")
            // as many rows out as in
            .category("Keep rows")
            .register::<frames::WithColumns, _>("with_columns")
            .register::<frames::Rename, _>("rename")
            .register::<frames::Drop, _>("drop")
            .register::<frames::Sort, _>("sort")
            // other rows out than in, or other columns altogether
            .category("Reshape")
            .register::<frames::Select, _>("select")
            .register::<frames::Filter, _>("filter")
            .register::<frames::Limit, _>("limit")
            .register::<frames::Unique, _>("unique")
            .register::<frames::DropNulls, _>("drop_nulls")
            .register::<frames::GroupBy, _>("group_by")
            .register::<frames::Join, _>("join")
            .register::<frames::Concat, _>("concat")
            .register::<frames::Unpivot, _>("unpivot")
            .register::<frames::Explode, _>("explode")
    }

    /// Names the category of the Blocks added from here on; without one it is "Other".
    pub fn category(
        mut self,
        category: &'static str,
    ) -> Self {
        if !self
            .categories
            .contains(&category)
        {
            self.categories
                .push(category);
        }
        self.category = category;
        self
    }

    /// Adds a Block to the Registry, in the category named last.
    pub fn register<B: Block<N>, const N: usize>(
        mut self,
        name: &'static str,
    ) -> Self {
        // the default names every input of a Block alike
        let mut inputs = B::INPUTS.to_vec();
        inputs.sort_unstable();
        inputs.dedup();
        assert!(inputs.len() == N, "block '{name}' names an input twice");

        let taken = self
            .blocks
            .insert(name, Entry::of::<B, N>(self.category))
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
                    json!({"inputs": entry.inputs, "category": entry.category, "args": args}),
                )
            })
            .collect();

        let pipeline = generator.subschema_for::<Pipeline>();
        // the categories that hold a Block, in the order they were named
        let categories: Vec<&str> = self
            .categories
            .iter()
            .copied()
            .filter(|category| {
                self.blocks
                    .values()
                    .any(|entry| entry.category == *category)
            })
            .collect();
        json!({
            "polars": polars::VERSION,
            "categories": categories,
            "blocks": blocks,
            "pipeline": pipeline,
            "$defs": generator.definitions(),
        })
    }
}
