use camino::Utf8Path;
use camino::Utf8PathBuf;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use tap::Tap;
use xdg::BaseDirectories;

pub struct ProjectPaths {
    /// The user's home directory.
    home_dir: Utf8PathBuf,
    /// The XDG base directories for the system (e.g. `~/.config`).
    xdg: BaseDirectories,
    /// The XDG base directories for `npingler` specifically (e.g. `~/.config/npingler`).
    project_xdg: BaseDirectories,
}

impl ProjectPaths {
    pub fn new() -> miette::Result<Self> {
        let home_dir = match std::env::home_dir() {
            Some(home_dir) => Utf8PathBuf::try_from(home_dir)
                .into_diagnostic()
                .wrap_err("Invalid home directory")?,
            None => {
                return Err(miette!("Couldn't find home directory"));
            }
        };

        let xdg = BaseDirectories::new();
        tracing::debug!(?xdg, "Resolved XDG base dirs");
        let project_xdg = BaseDirectories::with_prefix("npingler");
        tracing::debug!(?project_xdg, "Resolved project XDG base dirs");
        Ok(Self {
            home_dir,
            xdg,
            project_xdg,
        })
    }

    fn find_config_paths(&self, name: impl AsRef<Utf8Path>) -> miette::Result<Vec<Utf8PathBuf>> {
        let config_dirs = self.project_xdg.get_config_dirs();

        let mut ret = Vec::with_capacity(config_dirs.len() + 1);

        let config_home = self.project_xdg.get_config_home();

        if let Some(path) = config_home {
            let mut path = Utf8PathBuf::try_from(path).into_diagnostic()?;
            path.push(&name);
            ret.push(path);
        }

        for path in config_dirs {
            let mut path = Utf8PathBuf::try_from(path).into_diagnostic()?;
            path.push(&name);
            ret.push(path);
        }

        Ok(ret)
    }

    pub fn config_paths(&self) -> miette::Result<Vec<Utf8PathBuf>> {
        self.find_config_paths("config.toml")
    }

    pub fn default_config_path(&self) -> miette::Result<Utf8PathBuf> {
        let mut config_dir: Utf8PathBuf = self
            .project_xdg
            .get_config_home()
            .ok_or_else(|| miette!("No home directory found (this should never happen)"))?
            .try_into()
            .into_diagnostic()?;

        config_dir.push("config.toml");

        Ok(config_dir)
    }

    pub fn nix_paths(&self) -> miette::Result<Vec<Utf8PathBuf>> {
        self.find_config_paths("default.nix")
    }

    pub fn home_dir(&self) -> &Utf8Path {
        &self.home_dir
    }

    /// Get the path to the user's Nix profile, if it exists.
    pub fn nix_profile(&self) -> miette::Result<Option<Utf8PathBuf>> {
        if let Some(state_home) = self.xdg.get_state_home() {
            let state_home = Utf8PathBuf::try_from(state_home).into_diagnostic()?;
            let nix_profile_home = state_home.join("nix/profiles/profile");
            if nix_profile_home.symlink_metadata().is_ok() {
                return Ok(Some(nix_profile_home));
            }
        }

        let home_dir = self.home_dir().to_path_buf();

        let default_profile = home_dir.tap_mut(|p| p.push(".nix-profile"));

        // NB: This will return `false` if the profile is a symlink to a nonexistent directory.
        // https://docs.rs/camino/latest/camino/struct.Utf8PathBuf.html#method.exists
        // This matters because Nix will create a symlink to a nonexistent directory:
        // https://github.com/NixOS/nix/issues/3051
        if default_profile.exists() {
            return Ok(Some(default_profile));
        }

        let user_profile: Utf8PathBuf = [
            "/nix",
            "var",
            "nix",
            "profiles",
            "per-user",
            &whoami::username(),
            "profile",
        ]
        .iter()
        .collect();

        if user_profile.exists() {
            return Ok(Some(user_profile));
        }

        Ok(None)
    }

    pub fn expand_tilde(&self, path: &str) -> miette::Result<Utf8PathBuf> {
        Ok(shellexpand::full(path)
            .into_diagnostic()
            .wrap_err_with(|| {
                format!("Failed to expand `~/` and environment variables in path: {path}")
            })?
            .into_owned()
            .into())
    }
}
