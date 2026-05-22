use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use vertex_ir::{Capability, GenerationManifest, Service, StateVolume};

#[derive(Debug, Clone)]
pub struct HostedCapability {
    pub id: String,
    pub kind: String,
    pub provider: String,
    pub rights: Vec<String>,
    pub endpoint: HostedEndpoint,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HostedEndpoint {
    UnixSocket { path: PathBuf },
    TcpListener { host: String, port: u16 },
    StateDir { path: PathBuf },
    HostClock { name: String },
    Opaque { value: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedCapabilityGrant {
    pub id: String,
    pub kind: String,
    pub provider: String,
    pub consumer: String,
    pub rights: Vec<String>,
    pub endpoint: HostedEndpoint,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedStateGrant {
    pub id: String,
    pub kind: String,
    pub owner: String,
    pub consumer: String,
    pub path: PathBuf,
    pub endpoint: HostedEndpoint,
}

impl HostedEndpoint {
    pub fn env_value(&self) -> String {
        match self {
            HostedEndpoint::UnixSocket { path } => format!("unix:{}", path.display()),
            HostedEndpoint::TcpListener { host, port } => format!("tcp:{host}:{port}"),
            HostedEndpoint::StateDir { path } => format!("state-dir:{}", path.display()),
            HostedEndpoint::HostClock { name } => format!("host-clock:{name}"),
            HostedEndpoint::Opaque { value } => value.clone(),
        }
    }

    pub fn unix_socket_path(&self) -> Option<&Path> {
        match self {
            HostedEndpoint::UnixSocket { path } => Some(path),
            _ => None,
        }
    }

    pub fn tcp_target(&self) -> Option<(&str, u16)> {
        match self {
            HostedEndpoint::TcpListener { host, port } => Some((host.as_str(), *port)),
            _ => None,
        }
    }
}

impl HostedCapabilityGrant {
    pub fn env_value(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.id,
            self.kind,
            self.rights.join(","),
            self.endpoint.env_value()
        )
    }
}

impl HostedStateGrant {
    pub fn env_value(&self) -> String {
        format!("{}|{}", self.id, self.path.display())
    }
}

pub fn build_hosted_capabilities(
    manifest: &GenerationManifest,
    runtime_dir: &Path,
) -> BTreeMap<String, HostedCapability> {
    manifest
        .capabilities
        .iter()
        .map(|capability| {
            (
                capability.id.clone(),
                HostedCapability {
                    id: capability.id.clone(),
                    kind: capability.kind.clone(),
                    provider: capability.provider.clone(),
                    rights: capability.rights.clone(),
                    endpoint: hosted_endpoint(capability, runtime_dir),
                },
            )
        })
        .collect()
}

pub fn capability_grants_for_service(
    service: &Service,
    capabilities: &BTreeMap<String, HostedCapability>,
) -> Result<Vec<HostedCapabilityGrant>, String> {
    let mut grants = Vec::new();

    for requirement in &service.requires {
        let Some(capability) = capabilities.get(&requirement.capability) else {
            return Err(format!(
                "service {} requires unknown hosted capability {}",
                service.id, requirement.capability
            ));
        };
        grants.push(HostedCapabilityGrant {
            id: capability.id.clone(),
            kind: capability.kind.clone(),
            provider: capability.provider.clone(),
            consumer: service.id.clone(),
            rights: requirement.rights.clone(),
            endpoint: capability.endpoint.clone(),
        });
    }

    Ok(grants)
}

pub fn provided_capability_grants_for_service(
    service: &Service,
    capabilities: &BTreeMap<String, HostedCapability>,
) -> Result<Vec<HostedCapabilityGrant>, String> {
    let mut grants = Vec::new();

    for capability_id in &service.provides {
        let Some(capability) = capabilities.get(capability_id) else {
            return Err(format!(
                "service {} provides unknown hosted capability {}",
                service.id, capability_id
            ));
        };
        grants.push(HostedCapabilityGrant {
            id: capability.id.clone(),
            kind: capability.kind.clone(),
            provider: capability.provider.clone(),
            consumer: service.id.clone(),
            rights: capability.rights.clone(),
            endpoint: capability.endpoint.clone(),
        });
    }

    Ok(grants)
}

pub fn state_grants_for_service(
    service: &Service,
    manifest: &GenerationManifest,
    state_root: &Path,
) -> Result<Vec<HostedStateGrant>, String> {
    let mut grants = Vec::new();
    for state_id in &service.state {
        let Some(state) = manifest.state_volume(state_id) else {
            return Err(format!(
                "service {} references unknown state volume {}",
                service.id, state_id
            ));
        };

        if state.owner != service.id {
            return Err(format!(
                "state volume {} is owned by {}, so it cannot be granted to {} without an explicit sharing policy",
                state.id, state.owner, service.id
            ));
        }

        if state.kind != "hosted-local-directory" {
            return Err(format!(
                "state volume {} has unsupported hosted kind {}",
                state.id, state.kind
            ));
        }

        let current = state_current_path(state_root, state);
        fs::create_dir_all(&current).map_err(|source| {
            format!(
                "failed to create state volume {} for {}: {source}",
                current.display(),
                service.id
            )
        })?;
        reject_symlink(&current)?;
        let canonical_state_root = state_root.canonicalize().map_err(|source| {
            format!(
                "failed to canonicalize state root {}: {source}",
                state_root.display()
            )
        })?;
        let canonical_current = current.canonicalize().map_err(|source| {
            format!(
                "failed to canonicalize state volume {}: {source}",
                current.display()
            )
        })?;
        if !canonical_current.starts_with(&canonical_state_root) {
            return Err(format!(
                "state volume {} escapes state root: {}",
                state.id,
                canonical_current.display()
            ));
        }

        grants.push(HostedStateGrant {
            id: state.id.clone(),
            kind: state.kind.clone(),
            owner: state.owner.clone(),
            consumer: service.id.clone(),
            path: canonical_current.clone(),
            endpoint: HostedEndpoint::StateDir {
                path: canonical_current,
            },
        });
    }
    Ok(grants)
}

pub fn encode_capability_grants(grants: &[HostedCapabilityGrant]) -> String {
    grants
        .iter()
        .map(HostedCapabilityGrant::env_value)
        .collect::<Vec<_>>()
        .join(";")
}

pub fn encode_state_grants(grants: &[HostedStateGrant]) -> String {
    grants
        .iter()
        .map(HostedStateGrant::env_value)
        .collect::<Vec<_>>()
        .join(";")
}

pub fn capability_grants_json(grants: &[HostedCapabilityGrant]) -> Value {
    serde_json::to_value(grants).unwrap_or(Value::Null)
}

pub fn state_grants_json(grants: &[HostedStateGrant]) -> Value {
    serde_json::to_value(grants).unwrap_or(Value::Null)
}

pub fn state_current_path(state_root: &Path, state: &StateVolume) -> PathBuf {
    state_root
        .join("state-volumes")
        .join(sanitize_id(&state.id))
        .join("current")
}

pub fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn hosted_endpoint(capability: &Capability, runtime_dir: &Path) -> HostedEndpoint {
    match capability.kind.as_str() {
        "ipc-endpoint" | "log-sink" => {
            let file_name = format!("{}.sock", sanitize_id(&capability.id));
            HostedEndpoint::UnixSocket {
                path: runtime_dir.join(file_name),
            }
        }
        "network-port" => {
            let port = capability
                .properties
                .get("port")
                .and_then(|value| value.as_u64())
                .and_then(|port| u16::try_from(port).ok())
                .unwrap_or(0);
            let host = capability
                .properties
                .get("host")
                .and_then(|value| value.as_str())
                .unwrap_or("127.0.0.1")
                .to_owned();
            HostedEndpoint::TcpListener { host, port }
        }
        "clock" => {
            let name = capability
                .properties
                .get("clock")
                .and_then(|value| value.as_str())
                .unwrap_or("host")
                .to_owned();
            HostedEndpoint::HostClock { name }
        }
        _ => HostedEndpoint::Opaque {
            value: format!("opaque:{}", capability.id),
        },
    }
}

fn reject_symlink(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| format!("failed to inspect {}: {source}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "state volume path rejects symlink {}",
            path.display()
        ));
    }
    Ok(())
}
