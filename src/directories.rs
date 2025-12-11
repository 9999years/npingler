use camino::Utf8Path;
use camino::Utf8PathBuf;
use miette::Context;
use miette::IntoDiagnostic;
use miette::miette;
use tap::Tap;
use xdg::BaseDirectories;

use crate::nix::Nix;

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
    pub fn nix_profile(&self, nix: &Nix) -> miette::Result<Utf8PathBuf> {
        // I'm _pretty_ sure this does the same thing as upstream (minus the root profile
        // handling).
        //
        // See: https://git.lix.systems/lix-project/lix/src/commit/5dc847b47b4e0e970d6a1cf2da0abd7a4e1bad2e/lix/libstore/profiles.cc#L331-L349

        let profile_link = if nix.use_xdg_base_directories()?
            && let Some(profile) = self.xdg_nix_profile()?
        {
            profile
        } else {
            let home_dir = self.home_dir().to_path_buf();
            home_dir.tap_mut(|p| p.push(".nix-profile"))
        };

        match self.nix_profile_link_inner(&profile_link) {
            Ok(()) => crate::fs::resolve_symlink_once_utf8(profile_link),
            Err(err) => {
                tracing::debug!("Failed to get Nix profile link:\n{err}");
                Ok(profile_link)
            }
        }
    }

    fn nix_profile_link_inner(&self, profile_link: &Utf8Path) -> miette::Result<()> {
        if let Some(profile) = self.nix_profiles_dir()?.map(|mut dir| {
            dir.push("profile");
            dir
        }) && profile_link.symlink_metadata().is_err()
        {
            fs_err::os::unix::fs::symlink(&profile, profile_link).into_diagnostic()?;
        }

        Ok(())
    }

    /// Get `~/.local/state/nix/profiles`.
    fn nix_profiles_dir(&self) -> miette::Result<Option<Utf8PathBuf>> {
        Ok(self.xdg_nix_dir()?.map(|mut dir| {
            dir.push("profiles");
            dir
        }))
    }

    /// Get the new `use-xdg-base-directories` Nix profile path,
    /// `~/.local/state/nix/profile`.
    fn xdg_nix_profile(&self) -> miette::Result<Option<Utf8PathBuf>> {
        Ok(self.xdg_nix_dir()?.and_then(|mut dir| {
            dir.push("profile");
            if dir.symlink_metadata().is_ok() {
                Some(dir)
            } else {
                None
            }
        }))
    }

    /// Get `~/.local/state/nix`.
    fn xdg_nix_dir(&self) -> miette::Result<Option<Utf8PathBuf>> {
        if let Some(state_home) = self.xdg.get_state_home() {
            let state_home = Utf8PathBuf::try_from(state_home).into_diagnostic()?;
            Ok(Some(state_home.join("nix")))
        } else {
            Ok(None)
        }
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
