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
    fn of<B: Block<N>, const N: usize>() -> Self {
        Entry {
            inputs: &B::INPUTS,
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
            .register::<io::ScanParquet, _>("scan_parquet")
            // standard
            .register::<frames::Select, _>("select")
            .register::<frames::WithColumns, _>("with_columns")
            .register::<frames::Filter, _>("filter")
            .register::<frames::GroupBy, _>("group_by")
            .register::<frames::Join, _>("join")
            .register::<frames::Concat, _>("concat")
            .register::<frames::Sort, _>("sort")
            .register::<frames::Unique, _>("unique")
            .register::<frames::Unpivot, _>("unpivot")
            .register::<frames::Explode, _>("explode")
            .register::<frames::Drop, _>("drop")
            .register::<frames::DropNulls, _>("drop_nulls")
            .register::<frames::Rename, _>("rename")
            .register::<frames::Limit, _>("limit")
            // sinks
            .register::<io::WriteParquet, _>("write_parquet")
    }

    /// Adds a Block to the Registry.
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
            .insert(name, Entry::of::<B, N>())
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
