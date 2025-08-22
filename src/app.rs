use std::collections::BTreeSet;
use std::process::Command;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use command_error::CommandExt;
use command_error::Utf8ProgramAndArgs;
use miette::miette;
use miette::Context;
use serde::de::DeserializeOwned;
use tracing::instrument;

use crate::cli::Args;
use crate::config::Config;
use crate::diff_trees::diff_trees;
use crate::format_bulleted_list;
use crate::fs::resolve_symlink_utf8;
use crate::nix::Nix;
use crate::pins::NixPins;

pub struct App {
    pub config: Config,
    nix_file: Utf8PathBuf,
    nix_profile: Option<Utf8PathBuf>,
    hostname: String,
    nix: Nix,
}

impl App {
    pub fn from_args(args: Args) -> miette::Result<Self> {
        let config = Config::from_args(args)?;
        let nix = config.nix()?;
        let nix_file = config.nix_file()?;
        // TODO: Should we create this profile if it doesn't exist?
        let nix_profile = config.nix_profile(&nix)?;
        let hostname = config.hostname()?;
        ::tracing::debug!(%nix_file, ?nix_profile, %hostname, "Resolved configuration");
        Ok(Self {
            config,
            nix_file,
            nix_profile,
            nix,
            hostname,
        })
    }

    pub fn command(&self) -> &crate::cli::Command {
        self.config.command()
    }

    #[instrument(level = "debug", skip(self))]
    fn nix_env_command(&self, profile: Option<Utf8PathBuf>) -> Command {
        let mut command = self.nix.nix_env_command();
        if let Some(profile) = profile {
            command.arg("--profile");
            command.arg(profile.as_str());
        }
        command
    }

    #[instrument(level = "debug", skip(self))]
    fn sudo_nix_env_command(&self, profile: Option<Utf8PathBuf>) -> Command {
        let mut command = self.nix.sudo_nix_env_command();
        // TODO: Duplication with `nix_env_command`.
        if let Some(profile) = profile {
            command.arg("--profile");
            command.arg(profile.as_str());
        }
        command
    }

    fn npingler_attr(&self, attr: &str) -> String {
        format!("npingler.{}.{}", self.hostname, attr)
    }

    #[instrument(level = "debug", skip(self))]
    fn build_npingler_attr(&self, attr: &str) -> miette::Result<Utf8PathBuf> {
        let attr = self.npingler_attr(attr);
        let out_paths = self.nix.build(&["--file", self.nix_file.as_str(), &attr])?;
        if out_paths.is_empty() {
            Err(miette!(
                "Building attr {attr} from {} produced no paths",
                self.nix_file
            ))
        } else if out_paths.len() > 1 {
            Err(miette!(
                "Building attr {attr} from {} produced too many paths:\n{}",
                self.nix_file,
                format_bulleted_list(&out_paths)
            ))
        } else {
            // This doesn't feel great.
            Ok(out_paths.into_iter().next().unwrap())
        }
    }

    #[instrument(level = "debug", skip(self))]
    fn eval_npingler_attr<T>(&self, attr: &str, apply: Option<&str>) -> miette::Result<T>
    where
        T: DeserializeOwned,
    {
        let attr = self.npingler_attr(attr);
        let mut args = vec!["--file", &self.nix_file.as_str()];

        if let Some(expr) = apply {
            args.push("--apply");
            args.push(expr);
        }

        args.push(&attr);

        self.nix.eval(&args)
    }

    #[instrument(level = "debug", skip(self))]
    pub fn update(&self) -> miette::Result<()> {
        let directory = self
            .nix_file
            .parent()
            .ok_or_else(|| miette!("Nix file has no parent directory: {}", self.nix_file))?;

        tracing::info!(%directory, "Upgrading `npins`");

        let mut command = Command::new("npins");
        command.current_dir(directory);
        command.arg("update");
        // TODO: Only run `npins` in verbose mode if `npingler` is in verbose mode?
        command.arg("--verbose");

        match self.config.run_mode() {
            crate::config::RunMode::Dry => {
                tracing::info!("Would run: {}", Utf8ProgramAndArgs::from(&command));
            }
            crate::config::RunMode::Wet => {
                command
                    .status_checked()
                    .wrap_err_with(|| format!("Failed to upgrade `npins` in {directory}"))?;
            }
        }

        Ok(())
    }

    fn get_profile_store_path(&self) -> miette::Result<Option<Utf8PathBuf>> {
        match self.nix_profile.as_deref() {
            Some(profile) => Ok(Some(resolve_symlink_utf8(profile.to_owned())?)),
            None => Ok(None),
        }
    }

    #[instrument(level = "debug", skip(self))]
    pub fn ensure_packages(&self) -> miette::Result<()> {
        tracing::info!("Building profile packages");
        let package_out_path = self.build_npingler_attr("packages")?;

        let old_profile = self.get_profile_store_path()?;
        tracing::debug!(?old_profile, "Resolved Nix profile");

        tracing::info!("Setting profile to {package_out_path}");
        let mut command = self.nix_env_command(self.nix_profile.clone());
        command.args(["--set", package_out_path.as_str()]);
        command.args(self.config.profile_extra_switch_args()?);

        match self.config.run_mode() {
            crate::config::RunMode::Dry => {
                tracing::info!("Would run: {}", Utf8ProgramAndArgs::from(&command));
            }
            crate::config::RunMode::Wet => {
                command
                    .status_checked()
                    .wrap_err("Failed to install new profile")?;
            }
        }

        {
            let removed_paths = match old_profile.as_deref() {
                Some(old_profile) => BTreeSet::from([old_profile]),
                None => BTreeSet::new(),
            };

            let added_paths = BTreeSet::from([&*package_out_path]);

            if removed_paths == added_paths {
                tracing::info!("No changes, profile already up to date");
            } else {
                let diff_result =
                    diff_trees(&removed_paths, &BTreeSet::from([&*package_out_path]))?;
                tracing::info!("Updated Nix profile:\n{}", diff_result);
            }
        }

        Ok(())
    }

    fn pin_flake_root(&self, name: &str, path: &Utf8Path) -> miette::Result<()> {
        tracing::info!("Pinning {name} -> {path}");
        let registry = self.config.root_registry_path()?;
        let mut command = self.nix.sudo_nix_command();
        command.args([
            "registry",
            "pin",
            "--registry",
            registry.as_str(),
            "--override-flake",
            name,
            path.as_str(),
            name,
        ]);

        match self.config.run_mode() {
            crate::config::RunMode::Dry => {
                tracing::info!("Would run: {}", Utf8ProgramAndArgs::from(&command));
            }
            crate::config::RunMode::Wet => {
                command.status_checked().wrap_err_with(|| {
                    format!("Failed to pin `nix registry` entry {name} to {path}")
                })?;
            }
        }
        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    pub fn ensure_channels(&self) -> miette::Result<()> {
        if !self.config.channels_pin_root() {
            tracing::debug!("Skipping pinning channels");
            return Ok(());
        }

        let profile = self.config.channels_root_profile()?;

        let mut command = self.sudo_nix_env_command(Some(profile));
        command.arg("--set");

        tracing::info!("Building channels");
        let channels = self.build_npingler_attr("pins.channels")?;
        command.arg(&channels);

        tracing::info!(%channels, "Pinning channels");

        match self.config.run_mode() {
            crate::config::RunMode::Dry => {
                tracing::info!("Would run: {}", Utf8ProgramAndArgs::from(&command));
            }
            crate::config::RunMode::Wet => {
                command
                    .status_checked()
                    .wrap_err("Failed to pin channels")?;
            }
        }

        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    pub fn ensure_registry(&self) -> miette::Result<()> {
        if !self.config.registry_pin_root() {
            tracing::debug!("Skipping pinning registry entries");
            return Ok(());
        }

        let pins: NixPins = self.eval_npingler_attr("pins.pins", None)?;

        tracing::info!("Pinning `root` Nix Flake registry entries");
        for (name, path) in &pins.entries {
            self.pin_flake_root(name, path)?;
        }

        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    pub fn switch(&self) -> miette::Result<()> {
        self.ensure_packages()?;
        self.ensure_registry()?;
        self.ensure_channels()?;
        Ok(())
    }
}
