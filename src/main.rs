use polaroid::Registry;
use polaroid::polars::prelude::PolarsResult;

fn main() -> PolarsResult<()> {
    polaroid::cli(Registry::with_polars())
}
