use camino::Utf8Path;
use camino::Utf8PathBuf;
use iddqd::IdHashItem;
use iddqd::IdHashMap;
use iddqd::id_upcast;
use rustc_hash::FxBuildHasher;
use rustc_hash::FxHashMap;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "DerivationsWire")]
pub struct Derivations(pub IdHashMap<Derivation, FxBuildHasher>);

impl From<DerivationsWire> for Derivations {
    fn from(wire: DerivationsWire) -> Self {
        Derivations(
            wire.0
                .into_iter()
                .map(
                    |(
                        path,
                        DerivationWire {
                            args,
                            builder,
                            env,
                            input_drvs,
                            input_srcs,
                            name,
                            outputs,
                            system,
                        },
                    )| Derivation {
                        path,
                        args,
                        builder,
                        env,
                        input_drvs,
                        input_srcs,
                        name,
                        outputs,
                        system,
                    },
                )
                .collect(),
        )
    }
}

#[derive(Deserialize)]
struct DerivationsWire(FxHashMap<Utf8PathBuf, DerivationWire>);

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
struct DerivationWire {
    args: Vec<String>,
    builder: Utf8PathBuf,
    env: FxHashMap<String, String>,
    input_drvs: FxHashMap<Utf8PathBuf, Input>,
    input_srcs: Vec<Utf8PathBuf>,
    name: String,
    outputs: FxHashMap<String, Output>,
    system: String,
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
