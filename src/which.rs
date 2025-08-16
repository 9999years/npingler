use camino::Utf8PathBuf;
use miette::Context;
use miette::IntoDiagnostic;
use tap::TryConv;
use tracing::instrument;

#[instrument(level = "debug")]
pub fn which_global(name: &str) -> miette::Result<Utf8PathBuf> {
    let mut program = which::which_global(name)
        .into_diagnostic()
        .wrap_err_with(|| format!("Could not find executable: {name}"))?
        .try_conv::<Utf8PathBuf>()
        .into_diagnostic()?;
    if program.is_symlink() {
        tracing::debug!(path = %program, "{name} is symlink");
        program = program
            .read_link_utf8()
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {name} symlink: {program:?}"))?;
    }
    tracing::debug!(path = %program, "Found {name}");

    Ok(program)
}
