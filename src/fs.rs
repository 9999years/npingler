use std::path::Path;

use camino::Utf8PathBuf;
use miette::IntoDiagnostic;
use miette::miette;

/// Basically [`std::fs::exists`] but it returns a [`std::fs::Metadata`] and it's backed by
/// [`fs_err::metadata`] instead.
///
/// See:
/// <https://github.com/rust-lang/rust/blob/425a9c0a0e365c0b8c6cfd00c2ded83a73bed9a0/library/std/src/sys/fs/common.rs#L54-L60>
pub fn exists_metadata(path: impl AsRef<Path>) -> std::io::Result<Option<std::fs::Metadata>> {
    match fs_err::metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn resolve_symlink_utf8(mut path: Utf8PathBuf) -> miette::Result<Utf8PathBuf> {
    while fs_err::symlink_metadata(&path)
        .into_diagnostic()?
        .is_symlink()
    {
        path = resolve_symlink_once_utf8(path)?;
    }

    Ok(path)
}

pub fn resolve_symlink_once_utf8(path: Utf8PathBuf) -> miette::Result<Utf8PathBuf> {
    let dest = fs_err::read_link(&path).into_diagnostic()?;
    if dest.is_absolute() {
        dest.try_into().into_diagnostic()
    } else {
        Ok(path
            .parent()
            .ok_or_else(|| miette!("Path has no parent: {path}"))?
            .join(&Utf8PathBuf::try_from(dest).into_diagnostic()?))
    }
}
