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
    let args = |subfolder: &str| {
        json!({
            "column": "bytes", "folder": folder, "subfolders": [{ "Column": subfolder }],
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
fn write_files_nests_its_subfolders_of_text_and_of_columns_and_takes_no_invalid_name() {
    let folder = folder("nested");
    let write = |typed: &str, named: &str, name: &str| {
        let frame = df!(
            "bytes" => [b"one".to_vec()],
            "name" => [name],
            "type" => ["txt"],
            "in" => [named],
        )
        .unwrap();
        let args = json!({
            "column": "bytes", "folder": folder,
            "subfolders": [{ "Text": typed }, { "Column": "in" }],
            "name": "name", "extension": "type",
        });
        apply("write_files", args, frame)
    };

    write("x", "y", "a").unwrap();
    assert!(
        folder
            .join("x")
            .join("y")
            .join("a.txt")
            .exists()
    );
    // what is no valid file name on some platform is none here
    for (typed, named, name) in [
        ("..", "y", "a"),
        ("x", "x/y", "a"),
        ("x", "C:", "a"),
        ("", "y", "a"),
        ("x", "y.", "a"),
        ("x", "y", ""),
        ("x", "y", "a?"),
        ("x", "y", "nul"),
        ("x", "y", "CON"),
        ("x", "y", "a."),
    ] {
        assert!(write(typed, named, name).is_err(), "{typed} {named} {name}");
    }
    let error = write("x", "y", "CON")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("row 0: 'CON' is an invalid file name"),
        "{error}"
    );
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
    let error = write([Some("c"), Some("C")])
        .unwrap_err()
        .to_string();
    assert!(error.contains("rows 0 and 1 write"), "{error}");
    assert!(write([Some("c"), Some("c")]).is_err());
    assert!(write([Some("c"), None]).is_err());
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn write_files_writes_none_of_its_files_where_one_is_there_unless_it_overwrites() {
    let folder = folder("there");
    let write = |overwrite: bool| {
        let frame = df!(
            "bytes" => [b"one".to_vec(), b"two".to_vec()],
            "name" => ["a", "b"],
            "type" => ["txt", "txt"],
        )
        .unwrap();
        let args = json!({
            "column": "bytes", "folder": folder, "name": "name", "extension": "type",
            "overwrite_existing": overwrite,
        });
        apply("write_files", args, frame)
    };
    let read = |name: &str| std::fs::read(folder.join(name)).ok();
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.txt"), "there").unwrap();

    // the error names the file that is there, and no other is written
    let error = write(false)
        .unwrap_err()
        .to_string();
    assert!(error.contains("a.txt"), "{error}");
    assert_eq!(read("a.txt").as_deref(), Some(&b"there"[..]));
    assert_eq!(read("b.txt"), None);

    write(true).unwrap();
    assert_eq!(read("a.txt").as_deref(), Some(&b"one"[..]));
    assert_eq!(read("b.txt").as_deref(), Some(&b"two"[..]));
    std::fs::remove_dir_all(&folder).unwrap();
}

/// A zip of one empty file of this name, stored as it is.
fn zip(name: &str) -> Vec<u8> {
    let named = |mut header: Vec<u8>, zeros: usize| {
        header.extend([0; 20]);
        header.extend((name.len() as u16).to_le_bytes());
        header.extend(vec![0; zeros]);
        header.extend(name.as_bytes());
        header
    };
    let local = named(b"PK\x03\x04\x14\0".to_vec(), 2);
    let central = named(b"PK\x01\x02\x14\0\x14\0".to_vec(), 16);
    let mut zip = [local.clone(), central.clone()].concat();
    zip.extend(b"PK\x05\x06\0\0\0\0\x01\0\x01\0");
    zip.extend((central.len() as u32).to_le_bytes());
    zip.extend((local.len() as u32).to_le_bytes());
    zip.extend([0, 0]);
    zip
}

#[test]
fn write_files_without_an_extension_takes_the_one_that_the_bytes_tell() {
    let folder = folder("kinds");
    let files: [(&str, &str, Vec<u8>); 11] = [
        ("a", "png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec()),
        ("b", "jpg", b"\xff\xd8\xff\xe0".to_vec()),
        ("c", "gif", b"GIF89a".to_vec()),
        ("d", "webp", b"RIFF\x1a\0\0\0WEBPVP8 ".to_vec()),
        (
            "e",
            "svg",
            br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.to_vec(),
        ),
        ("f", "svg", b"<?xml version=\"1.0\"?>\n<svg/>".to_vec()),
        ("g", "pdf", b"%PDF-1.7".to_vec()),
        ("h", "parquet", b"PAR1".to_vec()),
        ("i", "gz", b"\x1f\x8b\x08".to_vec()),
        ("j", "zip", zip("a.txt")),
        ("k", "xlsx", zip("xl/workbook.xml")),
    ];
    // of every type in one column
    let frame = df!(
        "bytes" => files.clone().map(|(_, _, bytes)| bytes),
        "name" => files.clone().map(|(name, _, _)| name),
    )
    .unwrap();
    let args = json!({ "column": "bytes", "folder": folder, "name": "name" });
    apply("write_files", args, frame).unwrap();
    for (name, extension, bytes) in files {
        let file = folder.join(format!("{name}.{extension}"));
        assert_eq!(std::fs::read(&file).ok(), Some(bytes), "{}", file.display());
    }
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn write_files_names_the_row_of_a_file_whose_type_its_bytes_do_not_tell() {
    let folder = folder("untold");
    for bytes in [
        b"some text".to_vec(),
        vec![0, 1, 2, 3],
        Vec::new(),
    ] {
        let frame = df!(
            "bytes" => [b"GIF89a".to_vec(), bytes],
            "name" => ["a", "b"],
        )
        .unwrap();
        let args = json!({ "column": "bytes", "folder": folder, "name": "name" });
        let error = apply("write_files", args, frame)
            .unwrap_err()
            .to_string();
        assert!(error.contains("row 1"), "{error}");
        assert!(error.contains("extension"), "{error}");
    }
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn write_files_takes_the_extension_that_is_given_over_the_one_that_the_bytes_tell() {
    let folder = folder("given");
    let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    let frame = df!(
        "bytes" => [png.clone(), png],
        "name" => ["a", "b"],
        "type" => [Some("bin"), None],
    )
    .unwrap();
    let args = json!({ "column": "bytes", "folder": folder, "name": "name", "extension": "type" });
    apply("write_files", args, frame).unwrap();
    // and in a row where it is null, the bytes tell it
    for (file, is_there) in [
        ("a.bin", true),
        ("a.png", false),
        ("b.png", true),
    ] {
        assert_eq!(
            folder
                .join(file)
                .exists(),
            is_there,
            "{file}"
        );
    }

    // which the editor shows as an argument that may be left out
    let catalog = Registry::with_polars().catalog();
    let args = polaroid::schema::resolve(&catalog.defs, &catalog.blocks["write_files"].args);
    assert!(polaroid::schema::optional(&args["properties"]["extension"]).is_some());
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

#[test]
fn slice_and_tail_get_the_rows_from_an_offset_and_from_the_end() {
    let frame = df!("a" => [1, 2, 3, 4]).unwrap();
    let rows = |block: &str, args: Value| {
        let frame = apply(block, args, frame.clone()).unwrap();
        let rows = frame
            .column("a")
            .unwrap()
            .i32()
            .unwrap();
        rows.into_no_null_iter()
            .collect::<Vec<_>>()
    };
    assert_eq!(rows("slice", json!({ "offset": 1, "length": 2 })), [2, 3]);
    // a negative offset counts from the end, and fewer rows than asked for are all there are
    assert_eq!(rows("slice", json!({ "offset": -1, "length": 2 })), [4]);
    assert_eq!(rows("tail", json!({ "n": 3 })), [2, 3, 4]);
    assert_eq!(rows("tail", json!({ "n": 9 })), [1, 2, 3, 4]);
}

#[test]
fn top_k_and_bottom_k_get_the_rows_at_either_end_in_order_and_no_nulls() {
    let frame = df!("a" => [Some(2), None, Some(3), Some(1)]).unwrap();
    let rows = |block: &str| {
        let by = serde_json::to_value(col("a")).unwrap();
        let frame = apply(block, json!({ "k": 2, "by": [by] }), frame.clone()).unwrap();
        let rows = frame
            .column("a")
            .unwrap()
            .i32()
            .unwrap();
        rows.iter()
            .collect::<Vec<_>>()
    };
    assert_eq!(rows("top_k"), [Some(3), Some(2)]);
    assert_eq!(rows("bottom_k"), [Some(1), Some(2)]);
}

#[test]
fn with_row_index_numbers_the_rows_in_a_first_column() {
    let frame = df!("a" => ["x", "y"]).unwrap();
    let indexed = apply("with_row_index", json!({}), frame.clone()).unwrap();
    assert_eq!(indexed.get_column_names(), ["index", "a"]);

    let args = json!({ "name": "row", "offset": 5 });
    let indexed = apply("with_row_index", args, frame.clone()).unwrap();
    let rows = indexed
        .column("row")
        .unwrap()
        .idx()
        .unwrap();
    assert_eq!(
        rows.into_no_null_iter()
            .collect::<Vec<_>>(),
        [5, 6]
    );
    // a name that is taken is none for the column
    assert!(apply("with_row_index", json!({ "name": "a" }), frame).is_err());
}

#[test]
fn unnest_puts_the_fields_of_a_struct_in_its_place() {
    let frame = df!("a" => [1], "b" => [2], "c" => [3])
        .unwrap()
        .lazy()
        .select([
            col("a"),
            as_struct(vec![col("b"), col("c")]).alias("both"),
        ])
        .collect()
        .unwrap();
    let args = json!({ "columns": ["both"] });
    let flat = apply("unnest", args, frame.clone()).unwrap();
    assert_eq!(flat.get_column_names(), ["a", "b", "c"]);

    let args = json!({ "columns": ["both"], "separator": "." });
    let flat = apply("unnest", args, frame.clone()).unwrap();
    assert_eq!(flat.get_column_names(), ["a", "both.b", "both.c"]);
    assert!(apply("unnest", json!({ "columns": [] }), frame).is_err());
}

/// What a node of `block` with `args` makes of these two frames.
fn both(
    block: &str,
    args: Value,
    first: DataFrame,
    second: DataFrame,
) -> PolarsResult<DataFrame> {
    let pipeline = json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "first", "block": "scan_parquet", "args": { "path": "" } },
            { "id": "second", "block": "scan_parquet", "args": { "path": "" } },
            { "id": "out", "block": block, "inputs": ["first", "second"], "args": args },
        ],
        "outputs": ["out"],
    });
    let pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();
    let known = HashMap::from([
        ("first".to_owned(), first),
        ("second".to_owned(), second),
    ]);
    let mut frames = pipeline.run_from(&Registry::with_polars(), &known)?;
    Ok(frames.remove(0))
}

#[test]
fn concat_takes_the_columns_of_both_frames_if_it_is_diagonal() {
    let top = df!("a" => [1], "b" => ["x"]).unwrap();
    let bottom = df!("a" => [2], "c" => [true]).unwrap();
    assert!(both("concat", json!({}), top.clone(), bottom.clone()).is_err());

    let all = both("concat", json!({ "diagonal": true }), top, bottom).unwrap();
    assert_eq!(all.get_column_names(), ["a", "b", "c"]);
    assert_eq!(all.height(), 2);
    assert_eq!(
        all.column("c")
            .unwrap()
            .null_count(),
        1
    );
}

#[test]
fn concat_horizontal_puts_the_columns_side_by_side_and_fills_the_shorter_frame() {
    let left = df!("a" => [1, 2]).unwrap();
    let right = df!("b" => ["x"]).unwrap();
    let all = both("concat_horizontal", json!({}), left.clone(), right).unwrap();
    assert_eq!(all.get_column_names(), ["a", "b"]);
    assert_eq!(
        all.column("b")
            .unwrap()
            .null_count(),
        1
    );
    // a name is that of one column
    assert!(both("concat_horizontal", json!({}), left.clone(), left).is_err());
}

#[test]
fn a_frame_is_written_to_a_csv_file_and_read_from_it_again() {
    let folder = folder("csv");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("frame.csv");
    let frame = df!("a" => [1i64, 2], "b" => ["x", "y"]).unwrap();
    let args = json!({ "path": path, "separator": ";" });
    apply("write_csv", args.clone(), frame.clone()).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "a;b\n1;x\n2;y\n");

    let pipeline = json!({
        "polars": polars::VERSION,
        "nodes": [{ "id": "in", "block": "scan_csv", "args": args }],
        "outputs": ["in"],
    });
    let pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();
    let read = pipeline
        .run(&Registry::with_polars())
        .unwrap()
        .remove(0);
    assert!(read.equals(&frame));

    // a separator is one byte
    let args = json!({ "path": path, "separator": "é" });
    assert!(apply("write_csv", args, frame).is_err());
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn pivot_makes_a_column_of_each_value_that_is_named() {
    let frame = df!(
        "day" => [1, 1, 2, 2, 2],
        "kind" => [10, 20, 10, 20, 30],
        "n" => [1, 2, 3, 4, 5],
    )
    .unwrap();
    let args = |extra: Value| {
        let mut args = json!({ "on": "kind", "on_columns": ["20", "10"], "index": ["day"] });
        args.as_object_mut()
            .unwrap()
            .extend(
                extra
                    .as_object()
                    .unwrap()
                    .clone(),
            );
        args
    };
    let wide = apply("pivot", args(json!({ "values": ["n"] })), frame.clone()).unwrap();
    // in the order they are named, and without the values that are not
    assert_eq!(wide.get_column_names(), ["day", "20", "10"]);
    let expected = df!("day" => [1, 2], "20" => [2, 4], "10" => [1, 3]).unwrap();
    assert!(wide.equals(&expected));

    // the columns are known before the frame is computed
    let pipeline = json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "in", "block": "scan_parquet", "args": { "path": "" } },
            { "id": "out", "block": "pivot", "inputs": ["in"],
              "args": args(json!({ "values": ["n"], "aggregate": "Len" })) },
        ],
        "outputs": [],
    });
    let pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();
    let known = HashMap::from([("in".to_owned(), frame.clear())]);
    let schemas = pipeline
        .schemas_from(&Registry::with_polars(), &known)
        .unwrap();
    let schema = schemas["out"]
        .as_ref()
        .unwrap();
    assert_eq!(schema.len(), 3);

    assert!(apply("pivot", args(json!({ "values": [] })), frame).is_err());
}

#[test]
fn join_asof_matches_each_row_to_the_one_before_it_within_a_tolerance() {
    let left = df!("t" => [1, 5, 10], "a" => ["x", "y", "z"]).unwrap();
    let right = df!("t" => [1, 4, 6], "b" => [1, 4, 6]).unwrap();
    let on = serde_json::to_value(col("t")).unwrap();
    let matched = |extra: Value| {
        let mut args = json!({ "left_on": on, "right_on": on });
        args.as_object_mut()
            .unwrap()
            .extend(
                extra
                    .as_object()
                    .unwrap()
                    .clone(),
            );
        let joined = both("join_asof", args, left.clone(), right.clone())?;
        let matched = joined
            .column("b")?
            .i32()?
            .iter()
            .collect::<Vec<_>>();
        PolarsResult::Ok(matched)
    };
    assert_eq!(matched(json!({})).unwrap(), [Some(1), Some(4), Some(6)]);
    assert_eq!(
        matched(json!({ "strategy": "Forward" })).unwrap(),
        [Some(1), Some(6), None]
    );
    assert_eq!(
        matched(json!({ "tolerance": 1.0 })).unwrap(),
        [Some(1), Some(4), None]
    );
    let apart = json!({ "allow_exact_matches": false });
    assert_eq!(matched(apart).unwrap(), [None, Some(4), Some(6)]);
    // a tolerance of time is none for a key of numbers, nor are two tolerances one
    assert!(matched(json!({ "tolerance_time": "5m" })).is_err());
    assert!(matched(json!({ "tolerance": 1.0, "tolerance_time": "5m" })).is_err());
}

#[test]
fn join_takes_no_join_that_is_a_block_of_its_own() {
    let frame = df!("a" => [1]).unwrap();
    let on = serde_json::to_value(col("a")).unwrap();
    let args = |how: Value| json!({ "left_on": [on], "right_on": [on], "how": how });
    assert!(both("join", args(json!("Semi")), frame.clone(), frame.clone()).is_ok());
    let asof = json!({ "AsOf": {} });
    assert!(both("join", args(asof), frame.clone(), frame).is_err());
}

#[test]
fn rows_are_grouped_into_windows_of_an_index_and_rolled_over() {
    let frame = df!("t" => [0i64, 1, 2, 3, 4], "n" => [1, 1, 1, 1, 1]).unwrap();
    let sum = serde_json::to_value(col("n").sum()).unwrap();
    let sums = |block: &str, args: Value| {
        let frame = apply(block, args, frame.clone())?;
        let sums = frame
            .column("n")?
            .i32()?
            .into_no_null_iter()
            .collect::<Vec<_>>();
        PolarsResult::Ok(sums)
    };

    // windows of two rows, and then of three that start every two
    let args = json!({ "index_column": "t", "every": "2i", "aggs": [sum] });
    assert_eq!(sums("group_by_dynamic", args).unwrap(), [2, 2, 1]);
    let args = json!({ "index_column": "t", "every": "2i", "period": "3i", "aggs": [sum] });
    assert_eq!(sums("group_by_dynamic", args).unwrap(), [3, 3, 1]);

    // the window of a row ends at it
    let args = json!({ "index_column": "t", "period": "2i", "aggs": [sum] });
    assert_eq!(sums("rolling", args).unwrap(), [1, 2, 2, 2, 2]);

    // what is no length of time is found before a run
    let args = json!({ "index_column": "t", "period": "soon", "aggs": [sum] });
    assert!(sums("rolling", args).is_err());
}

#[test]
fn a_window_block_is_planned_without_a_look_at_the_columns_it_reads() {
    let sum = serde_json::to_value(col("n").sum()).unwrap();
    let pipeline = json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "in", "block": "scan_parquet", "args": { "path": "" } },
            { "id": "grouped", "block": "group_by_dynamic", "inputs": ["in"],
              "args": { "index_column": "t", "every": "1h", "aggs": [sum] } },
            { "id": "rolled", "block": "rolling", "inputs": ["in"],
              "args": { "index_column": "t", "period": "1h", "aggs": [sum] } },
        ],
        "outputs": [],
    });
    let pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();
    // an editor wires the nodes over a frame of no columns in the place of each file
    let known = HashMap::from([("in".to_owned(), DataFrame::empty())]);
    let frames = pipeline
        .frames_from(&Registry::with_polars(), &known)
        .unwrap();
    assert!(frames["grouped"].is_ok());
    assert!(frames["rolled"].is_ok());
}
