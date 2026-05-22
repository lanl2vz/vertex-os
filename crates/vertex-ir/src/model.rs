use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub type Id = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationManifest {
    pub schema: String,
    pub generation: Generation,
    pub kernel: Kernel,
    pub init: Init,
    #[serde(default)]
    pub store: Vec<StoreObject>,
    #[serde(default)]
    pub executables: Vec<Executable>,
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(default)]
    pub state_volumes: Vec<StateVolume>,
    #[serde(default)]
    pub secrets: Vec<Secret>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub services: Vec<Service>,
    pub activation: Activation,
    pub policies: Policies,
}

impl GenerationManifest {
    pub fn store_object(&self, id: &str) -> Option<&StoreObject> {
        self.store.iter().find(|item| item.id == id)
    }

    pub fn executable(&self, id: &str) -> Option<&Executable> {
        self.executables.iter().find(|item| item.id == id)
    }

    pub fn service(&self, id: &str) -> Option<&Service> {
        self.services.iter().find(|item| item.id == id)
    }

    pub fn capability(&self, id: &str) -> Option<&Capability> {
        self.capabilities.iter().find(|item| item.id == id)
    }

    pub fn state_volume(&self, id: &str) -> Option<&StateVolume> {
        self.state_volumes.iter().find(|item| item.id == id)
    }

    pub fn secret(&self, id: &str) -> Option<&Secret> {
        self.secrets.iter().find(|item| item.id == id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Generation {
    pub id: Id,
    pub created_utc: String,
    pub description: String,
    #[serde(default)]
    pub parent: Option<Id>,
    #[serde(default)]
    pub manifest_hash: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kernel {
    pub id: Id,
    pub kind: String,
    pub store_object: Id,
    pub abi: String,
    pub target: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Init {
    pub id: Id,
    pub executable: Id,
    pub mode: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreObject {
    pub id: Id,
    pub name: String,
    pub kind: String,
    pub path: String,
    pub hash_algorithm: String,
    pub hash: String,
    pub size_bytes: u64,
    #[serde(default)]
    pub references: Vec<Id>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Executable {
    pub id: Id,
    pub store_object: Id,
    pub entrypoint: String,
    pub abi: String,
    #[serde(default)]
    pub args_default: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub id: Id,
    pub kind: String,
    pub provider: Id,
    #[serde(default)]
    pub rights: Vec<String>,
    #[serde(default)]
    pub properties: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub id: Id,
    pub name: String,
    pub executable: Id,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub requires: Vec<CapabilityRequirement>,
    #[serde(default)]
    pub provides: Vec<Id>,
    #[serde(default)]
    pub state: Vec<Id>,
    #[serde(default)]
    pub secrets: Vec<Id>,
    pub restart: String,
    #[serde(default)]
    pub resources: Value,
    #[serde(default)]
    pub health: Option<HealthCheck>,
    pub lifecycle: Lifecycle,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Service {
    pub fn required_capability(&self, id: &str) -> Option<&CapabilityRequirement> {
        self.requires
            .iter()
            .find(|requirement| requirement.capability == id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityRequirement {
    pub capability: Id,
    #[serde(default)]
    pub rights: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub kind: String,
    #[serde(default)]
    pub target: Option<Id>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lifecycle {
    #[serde(default)]
    pub start_after: Vec<Id>,
    #[serde(default)]
    pub stop_before: Vec<Id>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateVolume {
    pub id: Id,
    pub name: String,
    pub kind: String,
    pub owner: Id,
    pub mount_intent: String,
    #[serde(default)]
    pub snapshot_policy: Value,
    #[serde(default)]
    pub backup_policy: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Secret {
    pub id: Id,
    pub name: String,
    pub provider: String,
    pub required_at: String,
    pub rotation_policy: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: Id,
    pub kind: String,
    #[serde(default)]
    pub selector: Value,
    pub driver: Id,
    #[serde(default)]
    pub properties: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activation {
    pub root_service: Id,
    #[serde(default)]
    pub start_order: Vec<Id>,
    #[serde(default)]
    pub rollback_policy: Value,
    pub on_failure: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Policies {
    pub default_authority: String,
    pub allow_ambient_filesystem: bool,
    pub allow_ambient_network: bool,
    pub allow_ambient_devices: bool,
    pub capability_delegation: String,
    pub unknown_references: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
