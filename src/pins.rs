use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct NixPins {
    pub entries: BTreeMap<String, Utf8PathBuf>,
}
