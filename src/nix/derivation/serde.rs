use camino::Utf8PathBuf;
use rustc_hash::FxHashMap;
use serde::Deserialize;

use crate::nix::Derivation;
use crate::nix::Derivations;
use crate::nix::derivation::Input;
use crate::nix::derivation::Output;

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
pub struct DerivationsWire(FxHashMap<Utf8PathBuf, DerivationWire>);

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
