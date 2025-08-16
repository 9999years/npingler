use clap::Parser;

mod app;
mod cli;
mod config;
mod diff_trees;
mod directories;
mod format_bulleted_list;
mod fs;
mod nix;
mod pins;
mod tracing;
mod which;

pub use format_bulleted_list::format_bulleted_list;

use crate::app::App;
use crate::config::Config;

fn main() -> miette::Result<()> {
    let opts = cli::Args::parse();
    let filter_reload = tracing::install_tracing(
        opts.log_filter()
            .as_deref()
            .unwrap_or(tracing::DEFAULT_FILTER),
    )?;
    let app = App::from_args(opts)?;
    tracing::update_log_filters(&filter_reload, &app.config.log_filter())?;

    // TODO: Avoid duplicate evals!

    match app.command() {
        cli::Command::Update { no_switch, .. } => {
            app.update()?;
            if !no_switch {
                app.switch()?;
            }
        }
        cli::Command::Switch { .. } => {
            app.switch()?;
        }

        cli::Command::Config(config_command) => match config_command {
            cli::ConfigCommand::Init { output } => Config::init(output.as_deref())?,
        },
    }

    Ok(())
}
