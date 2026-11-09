use std::process::ExitCode;

use polaroid::Registry;

fn main() -> ExitCode {
    match polaroid::cli(Registry::with_polars()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
