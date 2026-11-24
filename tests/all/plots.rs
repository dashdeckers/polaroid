//! Plots: the images they add to a frame, of what, and the files written of them.

use std::collections::HashMap;

use polaroid::polars::prelude::*;
use polaroid::{Pipeline, Registry, polars};
use serde_json::{Value, json};

/// What a line plot of `t` and `v` with these args beside makes of `frame`.
fn line(
    args: Value,
    frame: DataFrame,
) -> PolarsResult<DataFrame> {
    let mut frames = run(args, frame, json!([]))?;
    Ok(frames.remove(0))
}

/// The plot of [`line`] as the output of a Pipeline that has these nodes after it.
fn run(
    args: Value,
    frame: DataFrame,
    after: Value,
) -> PolarsResult<Vec<DataFrame>> {
    let mut line = json!({ "x": "t", "y": "v" });
    line.as_object_mut()
        .unwrap()
        .extend(
            args.as_object()
                .unwrap()
                .clone(),
        );
    let mut nodes = json!([
        { "id": "all", "block": "scan_parquet", "args": { "path": "" } },
        { "id": "drawn", "block": "line", "inputs": ["all"], "args": line },
    ]);
    nodes
        .as_array_mut()
        .unwrap()
        .extend(
            after
                .as_array()
                .unwrap()
                .clone(),
        );
    let pipeline = json!({ "polars": polars::VERSION, "nodes": nodes, "outputs": ["drawn"] });
    let pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();
    let known = HashMap::from([("all".to_owned(), frame)]);
    pipeline.run_from(&Registry::with_polars(), &known)
}

/// Rows of two groups, one of which has a null.
fn rows() -> DataFrame {
    df!(
        "name" => ["a", "a", "b", "b", "b"],
        "t" => [0, 1, 0, 1, 2],
        "v" => [Some(1.0), Some(3.0), Some(2.0), None, Some(4.0)],
    )
    .unwrap()
}

/// The images in the column `plot`.
fn images(frame: &DataFrame) -> Vec<Option<&[u8]>> {
    frame
        .column("plot")
        .unwrap()
        .binary()
        .unwrap()
        .iter()
        .collect()
}

#[test]
fn a_plot_of_all_rows_is_one_row_with_its_values_beside_the_image() {
    let drawn = line(json!({ "title": "values" }), rows()).unwrap();
    assert_eq!(drawn.get_column_names(), ["t", "v", "plot"]);
    assert_eq!(drawn.height(), 1);
    // as they were read, and not as the numbers that are drawn
    let lists = rows()
        .lazy()
        .select([col("t"), col("v")])
        .select([all()
            .as_expr()
            .implode(true)])
        .collect()
        .unwrap();
    assert!(
        drawn
            .drop("plot")
            .unwrap()
            .equals_missing(&lists)
    );
    assert!(images(&drawn)[0].is_some_and(|image| image.starts_with(b"\x89PNG")));
}

#[test]
fn a_plot_is_in_the_format_asked_for() {
    for (format, start) in [
        ("Svg", &b"<svg"[..]),
        ("Png", &b"\x89PNG"[..]),
        ("Pdf", &b"%PDF"[..]),
    ] {
        let drawn = line(json!({ "format": format }), rows()).unwrap();
        assert!(
            images(&drawn)[0].is_some_and(|image| image.starts_with(start)),
            "{format}"
        );
    }
    assert!(line(json!({ "format": "Gif" }), rows()).is_err());
}

#[test]
fn a_plot_by_columns_is_a_row_of_each_group_in_the_order_they_appear() {
    let drawn = line(json!({ "by": ["name"] }), rows()).unwrap();
    assert_eq!(drawn.get_column_names(), ["name", "t", "v", "plot"]);
    let names = drawn
        .column("name")
        .unwrap()
        .str()
        .unwrap();
    assert_eq!(Vec::from(names), [Some("a"), Some("b")]);
    // other values, so another image
    let images = images(&drawn);
    assert!(images[0].is_some() && images[1].is_some());
    assert_ne!(images[0], images[1]);
}

#[test]
fn columns_of_lists_are_drawn_row_by_row_and_keep_the_other_columns() {
    let lists = rows()
        .lazy()
        .group_by_stable([col("name")])
        .agg([col("t"), col("v")])
        .collect()
        .unwrap();
    let drawn = line(json!({}), lists.clone()).unwrap();
    assert_eq!(drawn.get_column_names(), ["name", "t", "v", "plot"]);

    // the same images as of the groups, and none where a list is null
    let grouped = line(json!({ "by": ["name"] }), rows()).unwrap();
    assert_eq!(images(&drawn), images(&grouped));
    let holed = lists
        .lazy()
        .with_column(
            when(col("name").eq(lit("a")))
                .then(lit(NULL))
                .otherwise(col("v"))
                .alias("v"),
        )
        .collect()
        .unwrap();
    let drawn = line(json!({}), holed.clone()).unwrap();
    assert_eq!(images(&drawn)[0], None);
    assert!(images(&drawn)[1].is_some());

    // every row is a plot already
    assert!(line(json!({ "by": ["name"] }), holed).is_err());
}

#[test]
fn a_plot_takes_its_own_fields_and_those_of_every_plot_and_no_other() {
    let args = json!({ "width": 320, "height": 240, "by": [], "format": "Png", "title": "small" });
    let small = line(args, rows()).unwrap();
    let large = line(json!({ "format": "Png" }), rows()).unwrap();
    assert!(
        images(&small)[0]
            .unwrap()
            .len()
            < images(&large)[0]
                .unwrap()
                .len()
    );

    assert!(line(json!({ "widht": 320 }), rows()).is_err());
}

#[test]
fn a_plot_of_a_column_that_is_not_there_fails_the_check() {
    let pipeline = json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "all", "block": "scan_parquet", "args": { "path": "" } },
            { "id": "drawn", "block": "line", "inputs": ["all"], "args": { "x": "t", "y": "v" } },
        ],
        "outputs": [],
    });
    let pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();
    let schemas = pipeline
        .schemas(&Registry::with_polars())
        .unwrap();
    assert!(schemas["drawn"].is_err());
}

#[test]
fn the_plots_of_a_frame_are_written_to_files_of_the_names_of_a_column() {
    let folder = std::env::temp_dir().join(format!("polaroid-plots-{}", std::process::id()));
    let file = |name: &str| folder.join(format!("{name}.svg"));

    let extension = serde_json::to_value(lit("svg").alias("type")).unwrap();
    let after = json!([
        { "id": "typed", "block": "with_columns", "inputs": ["drawn"],
          "args": { "exprs": [extension] } },
        { "id": "written", "block": "write_files", "inputs": ["typed"],
          "args": { "column": "plot", "folder": folder, "name": "name", "extension": "type" } },
    ]);
    // the writes are no output, and run all the same
    let frames = run(json!({ "by": ["name"] }), rows(), after).unwrap();
    for (name, image) in ["a", "b"]
        .iter()
        .zip(images(&frames[0]))
    {
        assert_eq!(
            std::fs::read(file(name))
                .ok()
                .as_deref(),
            image
        );
    }
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn a_plot_is_built_without_a_look_at_its_columns_which_a_check_takes() {
    let pipeline = json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "all", "block": "scan_parquet", "args": { "path": "" } },
            { "id": "drawn", "block": "line", "inputs": ["all"],
              "args": { "x": "t", "y": "v", "by": ["name"] } },
        ],
        "outputs": ["all"],
    });
    let pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();
    let registry = Registry::with_polars();
    let lists = rows()
        .lazy()
        .group_by_stable([col("name")])
        .agg([col("t"), col("v")])
        .collect()
        .unwrap();

    // of a frame that has none of its columns, as the editor builds it to see how it is wired
    let empty = HashMap::from([("all".to_owned(), DataFrame::empty())]);
    let frames = pipeline
        .frames_from(&registry, &empty)
        .unwrap();
    assert!(frames["drawn"].is_ok());

    // lists that are grouped are wrong, which shows when its columns are asked for
    let known = HashMap::from([("all".to_owned(), lists)]);
    let mut frames = pipeline
        .frames_from(&registry, &known)
        .unwrap();
    let drawn = frames
        .get_mut("drawn")
        .unwrap()
        .as_mut()
        .unwrap();
    let wrong = drawn
        .collect_schema()
        .unwrap_err()
        .to_string();
    assert!(wrong.contains("take the columns out of `by`"), "{wrong}");

    // and is no matter to a Run of what it reads
    let ran = pipeline
        .run_from(&registry, &known)
        .unwrap();
    assert_eq!(ran[0].height(), 2);
}

#[test]
fn the_figure_of_a_row_is_drawn_over_again_of_the_values_beside_its_image() {
    let registry = Registry::with_polars();
    let drawn = line(json!({ "by": ["name"] }), rows()).unwrap();
    let drawer = registry
        .drawer("line", json!({ "x": "t", "y": "v", "by": ["name"] }))
        .expect("a line is a plot")
        .unwrap();

    // the second group as numbers, without the row whose value is null
    let values = drawer
        .values(&drawn, 1)
        .unwrap()
        .expect("the row has an image");
    assert!(values.equals(&df!("t" => [0.0, 2.0], "v" => [2.0, 4.0]).unwrap()));
    assert!(
        drawer
            .figure(&values)
            .is_ok()
    );
    assert_eq!(drawer.size, (640.0, 480.0));

    // a block that is no plot draws nothing, and a plot none of args that are not its own
    assert!(
        registry
            .drawer("limit", json!({ "n": 1 }))
            .is_none()
    );
    assert!(
        registry
            .drawer("line", json!({ "x": "t" }))
            .is_some_and(|drawer| drawer.is_err())
    );
}
