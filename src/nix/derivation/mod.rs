use ::serde::Deserialize;
use camino::Utf8Path;
use camino::Utf8PathBuf;
use iddqd::IdHashItem;
use iddqd::IdHashMap;
use iddqd::id_upcast;
use rustc_hash::FxBuildHasher;
use rustc_hash::FxHashMap;

mod diff;
mod serde;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "serde::DerivationsWire")]
pub struct Derivations(pub IdHashMap<Derivation, FxBuildHasher>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derivation {
    pub path: Utf8PathBuf,
    pub args: Vec<String>,
    /// I think these can be any path, but don't quote me on that?
    pub builder: Utf8PathBuf,
    pub env: FxHashMap<String, String>,
    pub input_drvs: FxHashMap<Utf8PathBuf, Input>,
    pub input_srcs: Vec<Utf8PathBuf>,
    pub name: String,
    /// Keys are output names, e.g. `out`, `man`, `bin`, etc.
    pub outputs: FxHashMap<String, Output>,
    /// Technically structured, but I don't think I care for this.
    pub system: String,
}

impl IdHashItem for Derivation {
    type Key<'a> = &'a Utf8Path;

    fn key(&self) -> Self::Key<'_> {
        &self.path
    }

    id_upcast! {}
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    // Always empty. Possibly by spec.
    pub dynamic_outputs: serde_json::Value,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub path: Utf8PathBuf,
}
