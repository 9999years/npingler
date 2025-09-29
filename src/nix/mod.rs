use std::collections::BTreeSet;
use std::os::unix::process::CommandExt as _;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use command_error::CommandExt;
use command_error::OutputContext;
use miette::Context;
use miette::IntoDiagnostic;
use miette::miette;
use serde::de::DeserializeOwned;
use tracing::instrument;
use utf8_command::Utf8Output;

mod registry;
pub use registry::Registry;

mod derivation;
pub use derivation::Derivation;
pub use derivation::Derivations;

#[derive(Debug, Clone)]
pub struct Nix {
    /// Path to the `nix` binary.
    nix_program: Utf8PathBuf,
    /// Path to the `nix-env` binary.
    nix_env_program: Utf8PathBuf,
}

impl Nix {
    pub fn new() -> miette::Result<Self> {
        let nix_program = crate::which::which_global("nix")?;
        let nix_env_program = crate::which::which_global("nix-env")?;
        Ok(Self {
            nix_program,
            nix_env_program,
        })
    }

    pub fn nix_command(&self) -> Command {
        let mut command = Command::new(&self.nix_program);
        command.arg("--extra-experimental-features");
        command.arg("nix-command");
        command
    }

    pub fn sudo_nix_command(&self) -> Command {
        let mut command = Command::new("sudo");
        let inner = self.nix_command();
        command.arg(inner.get_program());
        command.args(inner.get_args());
        command
    }

    pub fn nix_env_command(&self) -> Command {
        let mut command = Command::new(&self.nix_env_program);
        command.arg0("nix-env");
        command
    }

    pub fn sudo_nix_env_command(&self) -> Command {
        let mut command = Command::new("sudo");
        let inner = self.nix_env_command();
        command.arg(inner.get_program());
        command.args(inner.get_args());
        command
    }

    /// Build something and return the out paths.
    #[instrument(level = "debug", skip(self))]
    pub fn build(&self, args: &[&str]) -> miette::Result<BTreeSet<Utf8PathBuf>> {
        let stdout = self
            .nix_command()
            .args([
                "build",
                "--print-build-logs",
                "--no-link",
                "--print-out-paths",
            ])
            .args(args)
            .stderr(Stdio::inherit())
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout;

        Ok(stdout.lines().map(Utf8PathBuf::from).collect())
    }

    #[instrument(level = "debug", skip(self))]
    pub fn eval<T>(&self, args: &[&str]) -> miette::Result<T>
    where
        T: DeserializeOwned,
    {
        let mut command = self.nix_command();
        command.arg("eval");
        command.arg("--json");
        command.args(args);

        command
            .output_checked_as(|context: OutputContext<Output>| {
                serde_json::from_slice(&context.output().stdout)
                    .map_err(|err| context.error_msg(err))
            })
            .into_diagnostic()
    }

    /// Get a configuration setting by name.
    pub fn get_config(&self, setting: &str) -> miette::Result<Option<String>> {
        let mut command = self.nix_command();
        command.arg("config");
        command.arg("show");
        command.arg("--");
        command.arg(setting);

        command
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if context.output().status.success() {
                    Ok(Some(context.output().stdout.trim().to_owned()))
                } else if context.output().stderr.contains("could not find setting") {
                    Ok(None)
                } else {
                    Err(context.error())
                }
            })
            .into_diagnostic()
    }

    pub fn use_xdg_base_directories(&self) -> miette::Result<bool> {
        Ok(self.get_config("use-xdg-base-directories")?.as_deref() == Some("true"))
    }

    pub fn system_registry_path() -> &'static Utf8Path {
        Utf8Path::new("/etc/nix/registry.json")
    }

    pub fn parse_registry(&self, path: &Utf8Path) -> miette::Result<Registry> {
        let contents = fs_err::read_to_string(path)
            .into_diagnostic()
            .wrap_err("Failed to read Flake registry")?;

        serde_json::from_str(&contents)
            .into_diagnostic()
            .wrap_err("Failed to deserialize Flake registry")
    }

    // `nix derivation show` wrapper.
    pub fn derivation_infos<'p>(
        &self,
        paths: impl IntoIterator<Item = &'p Utf8Path>,
    ) -> miette::Result<Derivations> {
        self.nix_command()
            .args(["derivation", "show", "--"])
            .args(paths)
            .output_checked_as(|context: OutputContext<Output>| {
                serde_json::from_slice(&context.output().stdout)
                    .map_err(|err| context.error_msg(err))
            })
            .into_diagnostic()
    }

    pub fn derivation_info(&self, path: &Utf8Path) -> miette::Result<Derivation> {
        self.derivation_infos(std::iter::once(path))?
            .0
            .into_iter()
            .next()
            .ok_or_else(|| miette!("No derivation info given for {path}?"))
    }
}
