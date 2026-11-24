//! The blocks, each on a frame of its own.

use std::collections::HashMap;

use polaroid::polars::prelude::*;
use polaroid::{Pipeline, Registry, polars};
use serde_json::{Value, json};

/// What a node of `block` with `args` makes of `frame`.
fn apply(
    block: &str,
    args: Value,
    frame: DataFrame,
) -> PolarsResult<DataFrame> {
    let pipeline = json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "in", "block": "scan_parquet", "args": { "path": "" } },
            { "id": "out", "block": block, "inputs": ["in"], "args": args },
        ],
        "outputs": ["out"],
    });
    let pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();
    let known = HashMap::from([("in".to_owned(), frame)]);
    let mut frames = pipeline.run_from(&Registry::with_polars(), &known)?;
    Ok(frames.remove(0))
}

#[test]
fn rename_takes_as_many_new_names_as_columns() {
    let frame = df!("a" => [1], "b" => [2]).unwrap();
    let args = json!({ "existing": ["a", "b"], "new": ["x"] });
    assert!(apply("rename", args, frame).is_err());
}

#[test]
fn unique_of_no_columns_is_of_every_column() {
    let frame = df!("a" => [1, 1, 2]).unwrap();
    let unique = apply("unique", json!({ "subset": [] }), frame).unwrap();
    assert_eq!(unique.height(), 2);
}

#[test]
fn unpivot_of_no_columns_is_of_every_other_column() {
    let frame = df!("id" => [1], "a" => [2], "b" => [3]).unwrap();
    let args = json!({ "on": [], "index": ["id"] });
    let long = apply("unpivot", args, frame).unwrap();
    assert_eq!(long.height(), 2);
}

#[test]
fn every_block_and_every_one_of_its_args_is_described() {
    let catalog = Registry::with_polars().catalog();
    for (name, block) in &catalog.blocks {
        let args = polaroid::schema::resolve(&catalog.defs, &block.args);
        assert!(args["description"].is_string(), "{name}");
        let fields = args["properties"]
            .as_object()
            .into_iter()
            .flatten();
        for (field, schema) in fields {
            assert!(schema["description"].is_string(), "{name}.{field}");
        }
    }
}

#[test]
fn sort_puts_nulls_first_either_way() {
    let by = serde_json::to_value(col("a")).unwrap();
    for descending in [false, true] {
        let frame = df!("a" => [Some(1), None, Some(3)]).unwrap();
        let args = json!({ "by": [by], "descending": descending });
        let sorted = apply("sort", args, frame).unwrap();
        let first = sorted
            .column("a")
            .unwrap()
            .get(0)
            .unwrap();
        assert_eq!(first, AnyValue::Null);
    }
}

#[test]
fn drop_nulls_of_no_columns_drops_no_row() {
    let frame = df!("a" => [Some(1), None]).unwrap();
    let kept = apply("drop_nulls", json!({ "columns": [] }), frame).unwrap();
    assert_eq!(kept.height(), 2);
}

#[test]
fn sample_picks_other_rows_by_another_seed_and_keeps_their_order() {
    let rows: Vec<u32> = (0..1000).collect();
    let frame = df!("a" => &rows, "b" => &rows).unwrap();
    let picked = |n: u32, seed: Option<u64>| -> Vec<u32> {
        let args = match seed {
            Some(seed) => json!({ "n": n, "seed": seed }),
            None => json!({ "n": n }),
        };
        let sampled = apply("sample", args, frame.clone()).unwrap();
        // whole rows are picked
        assert!(
            sampled
                .column("a")
                .unwrap()
                .equals(
                    sampled
                        .column("b")
                        .unwrap()
                )
        );
        let a = sampled
            .column("a")
            .unwrap()
            .u32()
            .unwrap();
        a.into_no_null_iter()
            .collect()
    };

    let first = picked(5, Some(1));
    assert_eq!(first.len(), 5);
    assert!(first.is_sorted());
    // the same rows by the same seed, others by another, and no seed is seed 0
    assert_eq!(picked(5, Some(1)), first);
    assert_ne!(picked(5, Some(2)), first);
    assert_eq!(picked(5, None), picked(5, Some(0)));
    // not the first rows, as limit gives
    assert_ne!(first, [0, 1, 2, 3, 4]);
    // and all of a frame of fewer rows
    assert_eq!(picked(2000, Some(1)), rows);
    assert!(picked(0, Some(1)).is_empty());
}

#[test]
fn a_column_that_is_not_there_is_an_error_to_drop_and_to_rename() {
    let frame = df!("a" => [1]).unwrap();
    assert!(apply("drop", json!({ "columns": ["b"] }), frame.clone()).is_err());
    let args = json!({ "existing": ["b"], "new": ["c"] });
    assert!(apply("rename", args, frame).is_err());
}

#[test]
fn explode_gives_a_null_row_for_a_null_list() {
    let lists = df!("id" => [1, 2])
        .unwrap()
        .lazy()
        .with_column(
            when(col("id").eq(lit(1)))
                .then(concat_list([lit(1), lit(2)]).unwrap())
                .otherwise(lit(NULL))
                .alias("list"),
        )
        .collect()
        .unwrap();
    let long = apply("explode", json!({ "columns": ["list"] }), lists).unwrap();
    assert_eq!(long.height(), 3);
    assert!(apply("explode", json!({ "columns": [] }), long).is_err());
}

/// A folder of its own for the files of the test of `name`, which is to remove it.
fn folder(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("polaroid-{name}-{}", std::process::id()))
}

#[test]
fn write_files_writes_each_value_to_the_file_of_its_row_and_gives_its_frame() {
    let folder = folder("files");
    let frame = df!(
        "bytes" => [Some(b"one".to_vec()), None, Some(b"three".to_vec())],
        "name" => ["a", "b", "c"],
        "type" => ["txt", "txt", ".bin"],
        "in" => [Some("x"), Some("y"), None],
    )
    .unwrap();
    let args = |directories: &str| {
        json!({
            "column": "bytes", "folder": folder, "directories": directories,
            "name": "name", "extension": "type",
        })
    };

    let written = apply("write_files", args("in"), frame.clone()).unwrap();
    // the nodes that read it get the frame it writes
    assert!(written.equals_missing(&frame));
    // the folders that were missing are made
    assert_eq!(std::fs::read(folder.join("x/a.txt")).unwrap(), b"one");
    // a row without a folder writes to the one of them all, and a dot before the extension is one
    assert_eq!(std::fs::read(folder.join("c.bin")).unwrap(), b"three");
    // a row without a value writes no file
    assert!(
        !folder
            .join("y")
            .exists()
    );

    // a column that is not there is found before anything is written
    assert!(apply("write_files", args("none"), frame.clone()).is_err());
    // and so is one that does not hold what is read of it
    assert!(apply("write_files", args("bytes"), frame).is_err());
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn write_files_nests_the_folders_of_a_list_and_takes_none_that_leaves_its_folder() {
    let folder = folder("nested");
    let write = |folders: &[&str], name: &str| {
        let folders = Series::new("".into(), folders);
        let frame = df!(
            "bytes" => [b"one".to_vec()],
            "name" => [name],
            "type" => ["txt"],
            "in" => [folders],
        )
        .unwrap();
        let args = json!({
            "column": "bytes", "folder": folder, "directories": "in",
            "name": "name", "extension": "type",
        });
        apply("write_files", args, frame)
    };

    write(&["x", "y"], "a").unwrap();
    assert!(
        folder
            .join("x")
            .join("y")
            .join("a.txt")
            .exists()
    );
    // what is no single part of a path on some platform is none here
    for (folders, name) in [
        (&[".."][..], "a"),
        (&["x/y"], "a"),
        (&["C:"], "a"),
        (&[""], "a"),
        (&["x"], "a?"),
        (&["x"], "nul"),
        (&["x"], "a."),
    ] {
        assert!(write(folders, name).is_err(), "{folders:?} {name}");
    }
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn write_files_writes_no_file_twice_and_none_without_a_name() {
    let folder = folder("twice");
    let write = |names: [Option<&str>; 2]| {
        let frame = df!(
            "bytes" => [b"one".to_vec(), b"two".to_vec()],
            "name" => names,
            "type" => ["txt", "txt"],
        )
        .unwrap();
        let args =
            json!({ "column": "bytes", "folder": folder, "name": "name", "extension": "type" });
        apply("write_files", args, frame)
    };

    assert!(write([Some("a"), Some("b")]).is_ok());
    // a file system may hold these to be one file
    assert!(write([Some("a"), Some("A")]).is_err());
    assert!(write([Some("a"), None]).is_err());
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn write_files_is_planned_without_a_look_at_the_columns_it_reads() {
    let pipeline = json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "in", "block": "scan_parquet", "args": { "path": "" } },
            { "id": "out", "block": "write_files", "inputs": ["in"],
              "args": { "column": "bytes", "folder": "", "name": "name", "extension": "type" } },
        ],
        "outputs": [],
    });
    let pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();
    // an editor wires the nodes over a frame of no columns in the place of each file
    let known = HashMap::from([("in".to_owned(), DataFrame::empty())]);
    let registry = Registry::with_polars();
    let frames = pipeline
        .frames_from(&registry, &known)
        .unwrap();
    assert!(frames["out"].is_ok());
    // and a Check then finds the columns that are not there
    let schemas = pipeline
        .schemas_from(&registry, &known)
        .unwrap();
    assert!(schemas["out"].is_err());
}

#[test]
fn view_images_gives_its_frame_and_tells_the_column_of_the_images_to_show() {
    let frame = df!("image" => [b"one".to_vec()], "name" => ["a"]).unwrap();
    let args = json!({ "column": "image" });
    let viewed = apply("view_images", args.clone(), frame.clone()).unwrap();
    assert!(viewed.equals(&frame));

    let registry = Registry::with_polars();
    let images = registry.images("view_images", args);
    assert_eq!(images.as_deref(), Some("image"));
    // a block that shows no images tells of none
    assert_eq!(registry.images("limit", json!({ "n": 1 })), None);

    // a column that holds no images, or is not there, is found before a run
    assert!(apply("view_images", json!({ "column": "name" }), frame.clone()).is_err());
    assert!(apply("view_images", json!({ "column": "none" }), frame).is_err());
}
