use std::process::Command;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use clap::CommandFactory;
use command_error::CommandExt;
use command_error::Utf8ProgramAndArgs;
use miette::Context;
use miette::IntoDiagnostic;
use miette::miette;
use owo_colors::OwoColorize;
use serde::de::DeserializeOwned;
use tracing::instrument;

use crate::cli;
use crate::cli::Args;
use crate::config::Config;
use crate::format_bulleted_list;
use crate::fs::resolve_symlink_utf8;
use crate::nix::Nix;
use crate::nix::Registry;
use crate::pins::NixPins;

pub struct App {
    pub config: Config,
    nix_file: Utf8PathBuf,
    nix_profile: Option<Utf8PathBuf>,
    hostname: String,
    nix: Nix,
}

impl App {
    pub fn run(args: Args) -> miette::Result<()> {
        let filter_reload = crate::tracing::install_tracing(
            args.log_filter()
                .as_deref()
                .unwrap_or(crate::tracing::DEFAULT_FILTER),
        )?;

        // `App::from_args` requires `nix` is on the `$PATH`, so we handle some util commands like
        // generating shell completions before we look for that.
        //
        // TODO: This is pretty ugly? It would be nice to not match on the command twice like this.

        match &args.command {
            cli::Command::Config(config_command) => {
                match config_command {
                    cli::ConfigCommand::Init { output } => Config::init(output.as_deref())?,
                }
                return Ok(());
            }
            cli::Command::Util(util_command) => match util_command {
                cli::UtilCommand::GenerateCompletions { output, shell } => {
                    let mut clap_command = cli::Args::command();
                    let bin_name = "npingler";
                    match output {
                        None => {
                            clap_complete::generate(
                                *shell,
                                &mut clap_command,
                                bin_name,
                                &mut std::io::stdout(),
                            );
                        }
                        Some(path) => {
                            let mut file = fs_err::File::create(path).into_diagnostic()?;
                            clap_complete::generate(*shell, &mut clap_command, bin_name, &mut file);
                        }
                    }
                    return Ok(());
                }

                #[cfg(feature = "clap_mangen")]
                cli::UtilCommand::GenerateManPages { output } => {
                    let command = cli::Args::command();
                    clap_mangen::generate_to(command, output)
                        .into_diagnostic()
                        .wrap_err("Failed to generate man pages")?;
                    return Ok(());
                }
            },
            _ => {
                let app = App::from_args(args)?;
                crate::tracing::update_log_filters(&filter_reload, &app.config.log_filter())?;

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
                    cli::Command::Build { .. } => {
                        app.build_packages()?;
                    }
                    cli::Command::Config(config_command) => match config_command {
                        cli::ConfigCommand::Init { .. } => unreachable!(),
                    },
                    cli::Command::Util(util_command) => match util_command {
                        cli::UtilCommand::GenerateCompletions { .. } => unreachable!(),
                        #[cfg(feature = "clap_mangen")]
                        cli::UtilCommand::GenerateManPages { .. } => unreachable!(),
                    },
                }
            }
        }

        Ok(())
    }

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
            let out_path = out_paths.into_iter().next().unwrap();
            tracing::debug!(%attr, %out_path, "Built attr");
            Ok(out_path)
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
    pub fn build_packages(&self) -> miette::Result<Utf8PathBuf> {
        tracing::info!("Building profile packages");

        let old_profile = self.get_profile_store_path()?;
        let old_profile_drv = old_profile.as_deref().and_then(|old_profile| {
            match self.nix.derivation_info(old_profile) {
                Ok(drv) => Some(drv),
                Err(err) => {
                    tracing::warn!(
                        "Failed to get derivation info for old profile {old_profile}:\n{err}"
                    );
                    None
                }
            }
        });
        tracing::debug!(?old_profile, "Resolved Nix profile");

        let new_profile: Utf8PathBuf = self.eval_npingler_attr("packages", None)?;
        let new_profile_drv = self.nix.derivation_info(&new_profile)?;

        if let Some(diff_derivations_command) = self.config.diff_derivations()?
            && let Some(command) = diff_derivations_command.first()
            && let Some(old_profile_drv) = &old_profile_drv
            && old_profile_drv != &new_profile_drv
        {
            // Don't care... but use `status_checked` anyways to get logs :)
            let mut command = Command::new(command);

            if diff_derivations_command.len() > 1 {
                command.args(&diff_derivations_command[1..]);
            }

            let _ = command
                .args([old_profile_drv.path.as_str(), new_profile_drv.path.as_str()])
                .status_checked();
        }

        if !new_profile.exists() {
            match self.config.run_mode() {
                crate::config::RunMode::Dry => {
                    tracing::info!("Would build: {new_profile} from {}", new_profile_drv.path);
                }
                crate::config::RunMode::Wet => {
                    self.nix
                        .build(&[&format!("{}^out", new_profile_drv.path.as_str())])
                        .wrap_err("Failed to build new profile")?;
                }
            }
        }

        if old_profile.as_ref() == Some(&new_profile) {
            tracing::info!("No changes, profile already up to date");
        } else {
            self.diff_trees(old_profile.as_deref(), new_profile.as_path())?;
        }

        Ok(new_profile)
    }

    fn diff_trees(&self, old: Option<&Utf8Path>, new: &Utf8Path) -> miette::Result<()> {
        let old = match old {
            None => {
                tracing::info!("Updated Nix profile to {new}");
                return Ok(());
            }
            Some(old) => old,
        };

        let diff = diff_trees::Diff::new(old.as_std_path(), new.as_std_path()).into_diagnostic()?;

        if diff.is_empty() {
            // Sometimes, the profile path changes, but no installed paths actually
            // change their contents. Instead of showing a blank diff, we list the old and new
            // profiles.
            tracing::info!(
                "Updated Nix profile:\n{}\n{}",
                format!("- {old}").red(),
                format!("+ {new}").green(),
            );
        } else {
            tracing::info!(
                "Updated Nix profile:\n{}",
                diff.display(diff_trees::DisplayDiffOpts::new().color(true))
            );
        }

        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    pub fn ensure_packages(&self) -> miette::Result<()> {
        let new_profile = self.build_packages()?;

        tracing::info!("Setting profile to {new_profile}");

        let mut command = self.nix_env_command(self.nix_profile.clone());
        command.args(["--set", new_profile.as_str()]);
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

        Ok(())
    }

    fn pin_flake_root(
        &self,
        name: &str,
        path: &Utf8Path,
        registry: &Option<Registry>,
    ) -> miette::Result<()> {
        let current_path = match registry {
            Some(registry) => registry.id_to_path(name),
            None => None,
        };

        match current_path {
            Some(current_path) => {
                if current_path == path {
                    tracing::info!("Registry entry {name} is already set to {path}");
                    return Ok(());
                } else {
                    tracing::info!("Updating registry entry {name}:\n- {current_path}\n+ {path}");
                }
            }
            None => {
                tracing::info!("Pinning registry entry {name} to {path}");
            }
        }

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

        tracing::info!("Pinning channels");

        let profile = self.config.channels_root_profile()?;
        let channels = self.build_npingler_attr("pins.channels")?;
        let current_channels = crate::fs::resolve_symlink_utf8(profile.clone())?;

        if current_channels == channels {
            tracing::info!("Channels are already set to {channels}");
            return Ok(());
        } else {
            tracing::info!("Updating channels:\n- {current_channels}\n+ {channels}");
        }

        let mut command = self.sudo_nix_env_command(Some(profile));
        command.arg("--set");
        command.arg(&channels);

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

        let registry = match self.nix.parse_registry(Nix::system_registry_path()) {
            Ok(registry) => Some(registry),
            Err(error) => {
                tracing::warn!("{error:?}");
                None
            }
        };

        tracing::info!("Pinning `root` Nix Flake registry entries");
        for (name, path) in &pins.entries {
            self.pin_flake_root(name, path, &registry)?;
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
