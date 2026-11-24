use std::collections::BTreeMap;

use polars::prelude::*;
use schemars::generate::SchemaSettings;
use schemars::{Schema, SchemaGenerator};
use serde::Serialize;
use serde_json::Value;

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
            .category("Source / Sink")
            .register::<io::ScanParquet, _>("scan_parquet")
            .register::<io::WriteParquet, _>("write_parquet")
            // as many rows out as in, with other columns
            .category("Transform")
            .register::<frames::WithColumns, _>("with_columns")
            .register::<frames::Select, _>("select")
            .register::<frames::Rename, _>("rename")
            .register::<frames::Sort, _>("sort")
            // fewer rows or columns out than in, and those as they were
            .category("Filter")
            .register::<frames::Filter, _>("filter")
            .register::<frames::Drop, _>("drop")
            .register::<frames::Limit, _>("limit")
            .register::<frames::Unique, _>("unique")
            .register::<frames::DropNulls, _>("drop_nulls")
            // other rows out than in
            .category("Reshape")
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

    /// Describes the registered Blocks and the Pipeline.
    pub fn catalog(&self) -> Catalog {
        let mut generator = SchemaSettings::draft2020_12().into_generator();

        let blocks = self
            .blocks
            .iter()
            .map(|(name, entry)| {
                let described = Described {
                    inputs: entry.inputs,
                    category: entry.category,
                    args: (entry.schema)(&mut generator).to_value(),
                };
                (*name, described)
            })
            .collect();

        let pipeline = generator
            .subschema_for::<Pipeline>()
            .to_value();
        // the categories that hold a Block, in the order they were named
        let categories = self
            .categories
            .iter()
            .copied()
            .filter(|category| {
                self.blocks
                    .values()
                    .any(|entry| entry.category == *category)
            })
            .collect();
        Catalog {
            polars: polars::VERSION,
            categories,
            blocks,
            pipeline,
            defs: Value::Object(
                generator
                    .definitions()
                    .clone(),
            ),
        }
    }
}

/// What a Registry holds and what a Pipeline is, as the `catalog` command prints it in JSON.
#[derive(Serialize)]
pub struct Catalog {
    pub polars: &'static str,
    /// The categories that hold a Block, in the order they were named.
    pub categories: Vec<&'static str>,
    pub blocks: BTreeMap<&'static str, Described>,
    /// The JSON Schema of a Pipeline.
    pub pipeline: Value,
    /// The definitions that the schemas refer to by `$ref`.
    #[serde(rename = "$defs")]
    pub defs: Value,
}

/// A Block as a [`Catalog`] describes it.
#[derive(Serialize)]
pub struct Described {
    /// The names of the frames it reads, in order.
    pub inputs: &'static [&'static str],
    pub category: &'static str,
    /// The JSON Schema of its args.
    pub args: Value,
}
