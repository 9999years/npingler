use camino::Utf8PathBuf;

use crate::directories::ProjectPaths;

/// A friendly Nix profile manager.
#[derive(Debug, Clone, clap::Parser)]
#[command(version, author)]
#[allow(rustdoc::bare_urls)]
pub struct Args {
    /// Path to the configuration file to use.
    ///
    /// Defaults to `~/.config/npingler/config.toml`.
    #[arg(long)]
    pub config: Option<Utf8PathBuf>,

    /// Path or directory containing npingler Nix expressions.
    ///
    /// Defaults to the `--config` directory.
    #[arg(long)]
    pub file: Option<String>,

    #[command(flatten)]
    pub log: LogArgs,

    /// Don't actually change the configuration.
    #[arg(long, alias = "dry-run")]
    pub dry: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    /// Update the `npins` and switch to the updated profile.
    Update {
        /// Don't build or switch to the updated profile.
        #[arg(long)]
        no_switch: bool,

        #[command(flatten)]
        switch_args: SwitchArgs,
    },

    /// Build the current profile and switch to it (alias: install).
    #[command(alias = "install")]
    Switch {
        #[command(flatten)]
        switch_args: SwitchArgs,
    },

    // TODO: `pin-channels` and `pin-registry` commands would be nice, but the defaults (not
    // pinning channels or the registry) make the behavior very unintuitive.
    /// Commands to initialize the `npingler` configuration file.
    #[command(subcommand)]
    Config(ConfigCommand),
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum ConfigCommand {
    Init {
        /// Path to write the configuration to. Can be `-` for stdout. Defaults to
        /// `~/.config/npingler/config.toml`.
        output: Option<String>,
    },
}

#[derive(Debug, Default, Clone, clap::Args)]
pub struct SwitchArgs {
    /// The hostname to build the configuration for.
    ///
    /// This corresponds to the `npingler.${hostname}` output attribute in your flake.
    #[arg(long, alias = "host", env = "HOSTNAME")]
    pub hostname: Option<String>,

    #[command(flatten)]
    pub profile: ProfileArgs,

    #[command(flatten)]
    pub registry: RegistryArgs,

    #[command(flatten)]
    pub channel: ChannelArgs,
}

#[derive(Debug, Default, Clone, clap::Args)]
#[clap(next_help_heading = "Nix registry options")]
pub struct RegistryArgs {
    /// Pin Nix Flake registry entries for the `root` user.
    #[arg(long)]
    pub pin_registry_root: Option<bool>,

    /// The Nix Flake registry path for the `root` user, defaults to `/etc/nix/registry.json`.
    #[arg(long, env = "ROOT_NIX_REGISTRY")]
    pub root_registry_path: Option<Utf8PathBuf>,
}

#[derive(Debug, Default, Clone, clap::Args)]
#[clap(next_help_heading = "Nix channel options")]
pub struct ChannelArgs {
    /// Pin Nix channels for the `root` user.
    #[arg(long)]
    pub pin_channels_root: Option<bool>,

    /// The root user's Nix profile path. Defaults to `/nix/var/nix/profiles/per-user/root/channels`.
    #[arg(long, env = "ROOT_NIX_PROFILE")]
    pub root_profile: Option<Utf8PathBuf>,
}

#[derive(Debug, Default, Clone, clap::Args)]
#[clap(next_help_heading = "Nix profile options")]
pub struct ProfileArgs {
    /// Profile to use for `nix profile` operations.
    #[arg(long, env = "NIX_PROFILE")]
    pub profile: Option<Utf8PathBuf>,

    /// Shell-quoted extra arguments to pass to `nix-env --set ...` when switching to the new
    /// profile.
    #[arg(long)]
    pub extra_switch_args: Option<String>,
}

#[derive(Debug, Clone, clap::Args)]
#[clap(next_help_heading = "Logging options")]
pub struct LogArgs {
    /// Tracing log filter.
    ///
    /// See: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives
    #[arg(long, env = "NPINGLER_LOG")]
    pub log_filter: Option<String>,

    /// Alias for `--log-filter=trace`.
    #[arg(long)]
    pub debug: bool,

    /// Alias for `--log-filter=debug`.
    #[arg(short, long)]
    pub verbose: bool,
}

impl Args {
    pub fn config_paths(&self, project_paths: &ProjectPaths) -> miette::Result<Vec<Utf8PathBuf>> {
        if let Some(path) = &self.config {
            return Ok(vec![path.clone()]);
        }

        project_paths.config_paths()
    }

    pub fn log_filter(&self) -> Option<String> {
        let mut ret = String::new();

        if let Some(filter) = &self.log.log_filter {
            ret.push_str(filter);
        }

        if self.log.debug {
            ret.push_str(",trace");
        } else if self.log.verbose {
            ret.push_str(",debug");
        }

        if ret.is_empty() { None } else { Some(ret) }
    }
}
