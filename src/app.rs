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
use crate::cli::DiffArgs;
use crate::cli::ProfileSpec;
use crate::config::Config;
use crate::format_bulleted_list;
use crate::fs::resolve_symlink_utf8;
use crate::nix::Derivation;
use crate::nix::Nix;
use crate::nix::Registry;
use crate::pins::NixPins;

pub struct App {
    pub config: Config,
    nix_file: Utf8PathBuf,
    nix_profile: Utf8PathBuf,
    hostname: String,
    nix: Nix,
}

#[derive(Debug, Clone)]
struct ResolvedProfile {
    /// The spec that this profile was originally resolved from.
    spec: ProfileSpec,
    /// The store path of the profile. A resolved profile may or may not already have been built
    /// (i.e. this path may or may not already exist).
    path: Utf8PathBuf,
    /// If true, this profile may need to be built; if false, we can safely assume it has already
    /// been built.
    may_need_building: bool,
    derivation: Derivation,
}

#[derive(Debug, Clone)]
enum DiffResult {
    /// There's no previous profile to compare with - for example, when someone is running npingler
    /// for the very first time and ~/.nix-profile doesn't exist yet.
    NoPreviousProfile,
    /// Either the profile paths are the same, or the profile trees are the same.
    NoChanges,
    /// There are changes. The String contains a human-readable representation of them (which
    /// may ANSI escape codes) intended for printing to the console.
    Changes(String),
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
                    cli::Command::Diff { diff_args, .. } => {
                        app.diff(diff_args)?;
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
    fn eval<T>(&self, attr: &str, apply: Option<&str>) -> miette::Result<T>
    where
        T: DeserializeOwned,
    {
        let mut args = vec!["--file", &self.nix_file.as_str()];

        if let Some(expr) = apply {
            args.push("--apply");
            args.push(expr);
        }

        args.push(attr);

        self.nix.eval(&args)
    }

    #[instrument(level = "debug", skip(self))]
    fn eval_npingler_attr<T>(&self, attr: &str, apply: Option<&str>) -> miette::Result<T>
    where
        T: DeserializeOwned,
    {
        self.eval(&self.npingler_attr(attr), apply)
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

    fn get_profile_store_path(&self) -> miette::Result<Utf8PathBuf> {
        resolve_symlink_utf8(self.nix_profile.clone())
    }

    fn nix_profile_generation(&self, generation: i32) -> Utf8PathBuf {
        format!("{}-{}-link", self.nix_profile, generation).into()
    }

    fn get_profile_generation_store_path(&self, generation: i32) -> miette::Result<Utf8PathBuf> {
        resolve_symlink_utf8(self.nix_profile_generation(generation))
    }

    #[instrument(level = "debug", skip(self))]
    pub fn build_packages(&self) -> miette::Result<(Option<Utf8PathBuf>, Utf8PathBuf)> {
        tracing::info!("Building profile packages");

        let old_profile = self
            .resolve_profile(ProfileSpec::Current, Some("current profile"))
            .ok();

        let new_profile = self.resolve_profile(ProfileSpec::Config(None), Some("new profile"))?;

        self.diff_derivations(old_profile.as_ref(), &new_profile)?;

        self.ensure_profile(&new_profile, "new profile")?;

        let old_profile_path = old_profile.as_ref().map(|p| p.path.as_path());
        match self.diff_trees(old_profile_path, &new_profile.path) {
            Err(err) => {
                let old_profile_path = match old_profile_path {
                    Some(path) => path.as_str(),
                    None => "[none]",
                };
                tracing::debug!(
                    "Failed to diff profiles {old_profile_path} -> {}:\n{err}",
                    new_profile.path
                );
            }
            Ok(DiffResult::NoPreviousProfile) => (),
            Ok(DiffResult::NoChanges) => {
                tracing::info!("No changes, profile already up to date");
            }
            Ok(DiffResult::Changes(diff)) => {
                tracing::info!("Changes from current profile:\n{}", diff);
            }
        };

        Ok((old_profile.map(|o| o.path), new_profile.path))
    }

    pub fn diff(&self, diff_args: &DiffArgs) -> miette::Result<()> {
        // If --from was explicitly specified, require it to exist. Otherwise, use the current
        // profile, or None if there is no current profile.
        let old_profile = match diff_args.from {
            None => self.resolve_profile(ProfileSpec::Current, None).ok(),
            Some(ref from_profile) => Some(self.resolve_profile(from_profile.clone(), None)?),
        };

        let new_profile = self.resolve_profile(
            diff_args.to.clone().unwrap_or(ProfileSpec::Config(None)),
            None,
        )?;
        let new_profile_name = new_profile.spec.to_string();

        self.diff_derivations(old_profile.as_ref(), &new_profile)?;

        old_profile
            .as_ref()
            .map(|o| self.ensure_profile(o, &o.spec.to_string()))
            .transpose()?;
        self.ensure_profile(&new_profile, &new_profile_name)?;

        let old_profile_path = old_profile.as_ref().map(|p| p.path.as_path());

        match self.diff_trees(old_profile_path, &new_profile.path)? {
            DiffResult::NoPreviousProfile => (),
            DiffResult::NoChanges => {
                tracing::info!("No changes");
            }
            DiffResult::Changes(diff) => {
                tracing::info!("Changes:\n{}", diff);
            }
        };

        Ok(())
    }

    fn diff_derivations(
        &self,
        old_profile: Option<&ResolvedProfile>,
        new_profile: &ResolvedProfile,
    ) -> miette::Result<()> {
        if let Some(diff_derivations_command) = self.config.diff_derivations()?
            && let Some(command) = diff_derivations_command.first()
            && let Some(old_profile) = &old_profile
            && old_profile.derivation != new_profile.derivation
        {
            // Don't care... but use `status_checked` anyways to get logs :)
            let mut command = Command::new(command);

            if diff_derivations_command.len() > 1 {
                command.args(&diff_derivations_command[1..]);
            }

            let _ = command
                .args([
                    old_profile.derivation.path.as_str(),
                    new_profile.derivation.path.as_str(),
                ])
                .status_checked();
        }

        Ok(())
    }

    fn resolve_profile(
        &self,
        profile_spec: ProfileSpec,
        profile_name: Option<&str>,
    ) -> miette::Result<ResolvedProfile> {
        let (path, may_need_building) = match profile_spec {
            ProfileSpec::Current => (
                self.get_profile_store_path()
                    .inspect_err(|err| {
                        tracing::debug!("Failed to resolve {:?}: {err}", self.nix_profile)
                    })
                    .inspect(|profile| tracing::debug!(?profile, "Resolved Nix profile"))?,
                false,
            ),
            ProfileSpec::Generation(generation) => (
                self.get_profile_generation_store_path(generation)
                    .inspect_err(|err| {
                        tracing::debug!(
                            "Failed to resolve {:?}: {err}",
                            self.nix_profile_generation(generation)
                        )
                    })
                    .inspect(|profile| {
                        tracing::debug!(?profile, "Resolved Nix profile generation {generation}")
                    })?,
                false,
            ),
            ProfileSpec::Config(ref hostname) => {
                let hostname = match hostname {
                    Some(h) => h.clone(),
                    None => self.config.hostname()?,
                };
                (
                    self.eval(&format!("npingler.{}.packages", hostname), None)?,
                    true,
                )
            }
        };
        let derivation = self.nix.derivation_info(&path)?;
        let profile_name = match profile_name {
            Some(name) => name.to_string(),
            None => profile_spec.to_string(),
        };
        tracing::info!(
            out=%path,
            drv=%derivation.path,
            "Resolved {profile_name}"
        );
        tracing::debug!(?derivation, "Resolved profile derivation");
        Ok(ResolvedProfile {
            spec: profile_spec,
            path,
            may_need_building,
            derivation,
        })
    }

    // TODO: should this function return a value that tells the caller if the profile is actually
    // built (since with --dry, it might not be)?
    fn ensure_profile(&self, profile: &ResolvedProfile, profile_name: &str) -> miette::Result<()> {
        if !profile.may_need_building {
            return Ok(());
        };

        if profile.path.exists() {
            tracing::info!("{} is already built", first_to_upper(profile_name));
        } else {
            match self.config.run_mode() {
                crate::config::RunMode::Dry => {
                    tracing::info!(
                        "Would build: {} from {}",
                        profile.path,
                        profile.derivation.path
                    );
                }
                crate::config::RunMode::Wet => {
                    self.nix
                        .build(&[&format!("{}^out", profile.derivation.path.as_str())])
                        .wrap_err(format!("Failed to build {profile_name}"))?;
                    tracing::info!("Built {}: {}", profile_name, profile.path);
                }
            }
        };

        Ok(())
    }

    fn diff_trees(&self, old: Option<&Utf8Path>, new: &Utf8Path) -> miette::Result<DiffResult> {
        let old = match old {
            None => return Ok(DiffResult::NoPreviousProfile),
            Some(old) => old,
        };

        if old == new {
            return Ok(DiffResult::NoChanges);
        }

        let diff = diff_trees::Diff::new(old.as_std_path(), new.as_std_path()).into_diagnostic()?;
        // We have to display the diff to see if it's actually empty. See https://github.com/9999years/npingler/issues/68
        let displayed_diff = format!(
            "{}",
            diff.display(diff_trees::DisplayDiffOpts::new().color(true))
        );

        if displayed_diff.is_empty() {
            Ok(DiffResult::NoChanges)
        } else {
            Ok(DiffResult::Changes(displayed_diff))
        }
    }

    #[instrument(level = "debug", skip(self))]
    pub fn ensure_packages(&self) -> miette::Result<()> {
        let (old_profile, new_profile) = self.build_packages()?;

        match old_profile {
            Some(old) if old == new_profile => return Ok(()),
            Some(old) => tracing::info!(
                "Updating profile:\n{}\n{}",
                format!("- {old}").red(),
                format!("+ {new_profile}").green()
            ),
            None => tracing::info!("Updating profile:\n{}", format!("+ {new_profile}").green()),
        }

        if let Some(profile_dir) = self.nix_profile.parent()
            && fs_err::symlink_metadata(profile_dir).is_err()
        {
            fs_err::create_dir_all(profile_dir)
                .into_diagnostic()
                .wrap_err("Failed to create missing Nix profile directory")?;
        }

        let mut command = self
            .nix
            .nix_env_set_command(&self.nix_profile, &new_profile);

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
        tracing::debug!(?profile, "Resolved root profile");
        let channels = self.build_npingler_attr("pins.channels")?;
        tracing::debug!(?channels, "Built channels");
        let current_channels = fs_err::symlink_metadata(&profile).ok().and_then(|_| {
            crate::fs::resolve_symlink_utf8(profile.clone())
                .inspect_err(|err| {
                    tracing::debug!(?profile, "Failed to resolve root profile symlink: {err}")
                })
                .ok()
        });

        if current_channels.as_deref() == Some(channels.as_path()) {
            tracing::info!("Channels are already set to {channels}");
            return Ok(());
        } else {
            let current_channels = current_channels
                .map(|path| path.to_string())
                .unwrap_or_default();
            tracing::info!("Updating channels:\n- {current_channels}\n+ {channels}");
        }

        let mut command = self.nix.sudo_nix_env_set_command(&profile, &channels);

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
            Ok(registry) => registry,
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

fn first_to_upper(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => "".to_string(),
        Some(f) => format!("{}{}", f.to_uppercase().collect::<String>(), c.as_str()),
    }
}
