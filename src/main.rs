use clap::Parser;

mod app;
mod cli;
mod config;
mod directories;
mod format_bulleted_list;
mod fs;
mod nix;
mod pins;
mod tracing;
mod which;

pub use format_bulleted_list::format_bulleted_list;

use crate::app::App;

fn main() -> miette::Result<()> {
    let args = cli::Args::parse();
    App::run(args)?;

    Ok(())
}
