use camino::Utf8Path;
use camino::Utf8PathBuf;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "UnknownRegistry")]
pub enum Registry {
    V2(RegistryV2),
}

impl Registry {
    pub fn id_to_path<'s>(&'s self, id: &str) -> Option<&'s Utf8Path> {
        match self {
            Registry::V2(registry_v2) => registry_v2.id_to_path(id),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryDeserializeError {
    #[error("I don't know how to deserialize Nix registries with version {version}")]
    UnknownVersion { version: usize },
    #[error("{error}")]
    Serde { error: serde_json::Error },
}

impl TryFrom<UnknownRegistry> for Registry {
    type Error = RegistryDeserializeError;

    fn try_from(registry: UnknownRegistry) -> Result<Self, Self::Error> {
        match registry.version {
            2 => match serde_json::from_value::<RegistryV2>(registry.rest) {
                Ok(registry_v2) => Ok(Registry::V2(registry_v2)),
                Err(error) => Err(RegistryDeserializeError::Serde { error }),
            },
            _ => Err(RegistryDeserializeError::UnknownVersion {
                version: registry.version,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct UnknownRegistry {
    version: usize,

    #[serde(flatten)]
    rest: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RegistryV2 {
    flakes: Vec<RegistryEntryV2>,
}

impl RegistryV2 {
    pub fn id_to_path<'s>(&'s self, id: &str) -> Option<&'s Utf8Path> {
        for flake in &self.flakes {
            if let ReferenceV2::Indirect { id: current_id } = &flake.from
                && current_id == id
            {
                return match &flake.to {
                    ReferenceV2::Path { path } => Some(path.as_path()),
                    _ => None,
                };
            }
        }

        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RegistryEntryV2 {
    from: ReferenceV2,
    to: ReferenceV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum ReferenceV2 {
    Indirect {
        id: String,
    },

    Path {
        path: Utf8PathBuf,
    },

    /// This is an anti-pattern but it's fine here; we don't care about any `Other` variants, we
    /// just don't want deserialization to explode.
    ///
    /// See: <https://sunshowers.io/posts/open-closed-universes/#a-pattern-to-avoid-unknown-or-other-variants>
    #[serde(untagged)]
    Other(serde_json::Value),
}
