use camino::Utf8Path;
use camino::Utf8PathBuf;
use miette::Context;
use miette::IntoDiagnostic;
use miette::miette;

use crate::cli::Args;
use crate::cli::SwitchArgs;
use crate::directories::ProjectPaths;
use crate::format_bulleted_list;
use crate::nix::Nix;

pub const DEFAULT_CONFIG: &str = include_str!("../config.toml");

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum LogFilter {
    One(String),
    Many(Vec<String>),
}

#[derive(serde::Deserialize, Default)]
pub struct Log {
    #[serde(alias = "filter")]
    filters: Option<LogFilter>,
}

#[derive(serde::Deserialize, Default)]
pub struct Registry {
    pin_root: Option<bool>,
    root_path: Option<String>,
}

#[derive(serde::Deserialize, Default)]
pub struct Channels {
    pin_root: Option<bool>,
    root_profile: Option<String>,
}

#[derive(serde::Deserialize, Default)]
pub struct Profile {
    file: Option<String>,
    extra_switch_args: Option<Vec<String>>,
    diff_derivations: Option<bool>,
}

/// Configuration loaded from a file.
#[derive(serde::Deserialize, Default)]
pub struct ConfigFile {
    #[serde(default)]
    log: Log,
    file: Option<String>,
    #[serde(default)]
    profile: Profile,
    #[serde(default)]
    registry: Registry,
    #[serde(default)]
    channels: Channels,
}

impl ConfigFile {
    pub fn from_path(path: &Utf8Path) -> miette::Result<Self> {
        tracing::debug!("Loading config from {path}");
        let contents = std::fs::read_to_string(path)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {path}"))?;

        toml::from_str(&contents).into_diagnostic()
    }
}

pub enum RunMode {
    Dry,
    /// Well do YOU have a better name for it?
    Wet,
}

pub struct Config {
    path: Option<Utf8PathBuf>,
    project_paths: ProjectPaths,
    file: ConfigFile,
    args: Args,
    switch_args: SwitchArgs,
}

impl Config {
    pub fn from_args(args: Args) -> miette::Result<Self> {
        let project_paths = ProjectPaths::new()?;
        let paths = args.config_paths(&project_paths)?;

        let switch_args = match args.command {
            crate::cli::Command::Update {
                ref switch_args, ..
            } => switch_args.clone(),
            crate::cli::Command::Switch { ref switch_args } => switch_args.clone(),
            crate::cli::Command::Config(_) => SwitchArgs::default(),
        };

        tracing::trace!(?paths, "Looking for configuration file");
        for path in paths {
            if path
                .try_exists()
                .into_diagnostic()
                .wrap_err_with(|| format!("Failed to check if configuration path exists: {path}"))?
            {
                tracing::debug!(%path, "Loading configuration");
                let file = ConfigFile::from_path(&path)?;
                return Ok(Self {
                    path: Some(path),
                    project_paths,
                    file,
                    args,
                    switch_args,
                });
            }
        }

        tracing::debug!("No configuration file found");
        Ok(Self {
            path: None,
            project_paths,
            file: Default::default(),
            args,
            switch_args,
        })
    }

    pub fn command(&self) -> &crate::cli::Command {
        &self.args.command
    }

    pub fn log_filter(&self) -> String {
        let mut ret = String::new();
        match &self.file.log.filters {
            Some(LogFilter::One(filter)) => {
                ret.push_str(filter);
            }
            Some(LogFilter::Many(filters)) => {
                ret.push_str(&filters.join(","));
            }
            None => {}
        }

        if let Some(filter) = &self.args.log_filter() {
            ret.push(',');
            ret.push_str(filter);
        }

        if ret.is_empty() {
            ret.push_str(crate::tracing::DEFAULT_FILTER);
        }

        ret
    }

    pub fn nix_file(&self) -> miette::Result<Utf8PathBuf> {
        if let Some(file) = &self.args.file {
            match Self::resolve_nix_file(&self.project_paths.expand_tilde(file)?)? {
                Some(path) => {
                    return Ok(path);
                }
                None => {
                    return Err(miette!(
                        "`--file` does not exist or (if it's a directory) contain a `default.nix`: {file}"
                    ));
                }
            }
        }

        if let Some(file) = &self.file.file {
            match Self::resolve_nix_file(&self.project_paths.expand_tilde(file)?)? {
                Some(path) => {
                    return Ok(path);
                }
                None => {
                    return Err(miette!(
                        "`file` setting in config does not exist or (if it's a directory) contain a `default.nix`: {file}"
                    ));
                }
            }
        }

        let mut paths = self.project_paths.nix_paths()?;

        if let Some(path) = &self.path {
            paths.push(
                path.parent()
                    .ok_or_else(|| miette!("Configuration file has no parent directory: {path}"))?
                    .to_path_buf(),
            );
        }

        for path in &paths {
            if let Some(path) = Self::resolve_nix_file(path)? {
                return Ok(path);
            }
        }

        Err(miette!(
            "Unable to find npingler `default.nix`. I looked in these paths:\n{}",
            format_bulleted_list(&paths)
        ))
    }

    fn resolve_nix_file(path: &Utf8Path) -> miette::Result<Option<Utf8PathBuf>> {
        match crate::fs::exists_metadata(path).into_diagnostic()? {
            Some(metadata) => {
                if metadata.is_dir() {
                    let default_nix = path.join("default.nix");
                    match crate::fs::exists_metadata(&default_nix).into_diagnostic()? {
                        Some(_) => Ok(Some(path.to_owned())),
                        None => Ok(None),
                    }
                } else {
                    Ok(Some(path.to_owned()))
                }
            }
            None => Ok(None),
        }
    }

    pub fn hostname(&self) -> miette::Result<String> {
        match &self.switch_args.hostname {
            Some(hostname) => Ok(hostname.clone()),
            None => gethostname::gethostname()
                .into_string()
                .map_err(|s| miette!("Hostname is not UTF-8: {s:?}")),
        }
    }

    pub fn nix_profile(&self, nix: &Nix) -> miette::Result<Option<Utf8PathBuf>> {
        if let Some(profile) = self.switch_args.profile.profile.as_deref() {
            return Ok(Some(profile.to_path_buf()));
        }

        if let Some(profile) = self.file.profile.file.as_deref() {
            return Ok(Some(self.project_paths.expand_tilde(profile)?));
        }

        self.project_paths.nix_profile(nix)
    }

    pub fn nix(&self) -> miette::Result<Nix> {
        Nix::new()
    }

    pub fn channels_pin_root(&self) -> bool {
        if let Some(pin) = self.switch_args.channel.pin_channels_root {
            return pin;
        }

        if let Some(pin) = self.file.channels.pin_root {
            return pin;
        }

        false
    }

    pub fn channels_root_profile(&self) -> miette::Result<Utf8PathBuf> {
        if let Some(profile) = &self.switch_args.channel.root_profile {
            return Ok(profile.clone());
        }

        if let Some(profile) = &self.file.channels.root_profile {
            return self.project_paths.expand_tilde(profile);
        }

        Ok(Utf8Path::new("/nix/var/nix/profiles/per-user/root/channels").to_owned())
    }

    pub fn root_registry_path(&self) -> miette::Result<Utf8PathBuf> {
        if let Some(profile) = &self.switch_args.registry.root_registry_path {
            return Ok(profile.clone());
        }

        if let Some(profile) = &self.file.registry.root_path {
            return self.project_paths.expand_tilde(profile);
        }

        Ok(Utf8Path::new("/etc/nix/registry.json").to_owned())
    }

    pub fn registry_pin_root(&self) -> bool {
        if let Some(pin_root) = self.switch_args.registry.pin_registry_root {
            return pin_root;
        }

        if let Some(pin_root) = self.file.registry.pin_root {
            return pin_root;
        }

        false
    }

    pub fn profile_extra_switch_args(&self) -> miette::Result<Vec<String>> {
        if let Some(args) = self.switch_args.profile.extra_switch_args.clone() {
            return shell_words::split(&args)
                .into_diagnostic()
                .wrap_err_with(|| format!("Failed to shell unquote: {args}"));
        }

        if let Some(args) = self.file.profile.extra_switch_args.clone() {
            return Ok(args);
        }

        Ok(Default::default())
    }

    /// Write the default config file.
    pub fn init(output: Option<&str>) -> miette::Result<()> {
        let path: Utf8PathBuf = match output {
            Some(path) => {
                if path == "-" {
                    print!("{}", DEFAULT_CONFIG);
                    return Ok(());
                } else {
                    path.to_owned().try_into()?
                }
            }
            None => ProjectPaths::new()?.default_config_path()?,
        };

        let path = Utf8PathBuf::from(path);
        if path.exists() {
            return Err(miette!("Path already exists: {path}"));
        }
        fs_err::write(path, DEFAULT_CONFIG).into_diagnostic()?;

        Ok(())
    }

    pub fn run_mode(&self) -> RunMode {
        match self.args.dry {
            true => RunMode::Dry,
            false => RunMode::Wet,
        }
    }

    pub fn diff_derivations(&self) -> bool {
        self.switch_args
            .profile
            .diff_derivations
            .or(self.file.profile.diff_derivations)
            .unwrap_or(false)
    }
}
