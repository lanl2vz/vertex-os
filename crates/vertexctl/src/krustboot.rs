use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use vertex_abi::{graph as graph_abi, krustboot as krustboot_abi};
use vertex_ir::{GenerationManifest, Service};

const COMPACT_MAGIC: &[u8; 16] = krustboot_abi::COMPACT_MAGIC;
const COMPACT_VERSION: u16 = krustboot_abi::COMPACT_VERSION;
const POLICY_VERSION: u16 = krustboot_abi::POLICY_VERSION;
const V1_MAGIC: &[u8; 16] = krustboot_abi::V1_MAGIC;
const V1_VERSION: u16 = krustboot_abi::V1_VERSION;
const V1_HEADER_SIZE: usize = krustboot_abi::V1_HEADER_SIZE;
const V1_CHECKSUM_OFFSET: usize = krustboot_abi::V1_CHECKSUM_OFFSET;
const V1_RECORD_SIZE: usize = krustboot_abi::V1_RECORD_SIZE;
const V1_RECORD_COUNT: usize = krustboot_abi::V1_RECORD_COUNT;
const V1_PAYLOAD_OFFSET: usize = krustboot_abi::V1_PAYLOAD_OFFSET;
const COMPACT_GRAPH_NODE_COUNT_OFFSET: usize = krustboot_abi::COMPACT_GRAPH_NODE_COUNT_OFFSET;
const COMPACT_GRAPH_EDGE_COUNT_OFFSET: usize = krustboot_abi::COMPACT_GRAPH_EDGE_COUNT_OFFSET;
const COMPACT_GRAPH_CHECKSUM_OFFSET: usize = krustboot_abi::COMPACT_GRAPH_CHECKSUM_OFFSET;
const COMPACT_HEADER_SIZE: usize = krustboot_abi::COMPACT_HEADER_SIZE;
const STRING_LEN: usize = graph_abi::STRING_LEN;
const BOOT_MODULE_RECORD_LEN: usize = STRING_LEN * 2;
const PROCESS_REF_LIST_LEN: usize = 2 + MAX_PROCESS_REFS * 2;
const ENDPOINT_REQUIREMENT_LIST_LEN: usize = 2 + MAX_PROCESS_REFS * 4;
const PROCESS_MOUNT_RECORD_LEN: usize = STRING_LEN * 2 + 4;
const PROCESS_MOUNT_LIST_LEN: usize = 2 + MAX_PROCESS_MOUNTS * PROCESS_MOUNT_RECORD_LEN;
const PROCESS_MOUNT_ROOT_OFFSET: usize = STRING_LEN * 4 + 4;
const PROCESS_RECORD_LEN: usize = STRING_LEN * 5
    + 4
    + PROCESS_MOUNT_LIST_LEN
    + PROCESS_REF_LIST_LEN * 2
    + ENDPOINT_REQUIREMENT_LIST_LEN;
const PROCESS_PROVIDES_COUNT_OFFSET: usize = STRING_LEN * 5
    + 4
    + PROCESS_MOUNT_LIST_LEN
    + PROCESS_REF_LIST_LEN
    + ENDPOINT_REQUIREMENT_LIST_LEN;
const ENDPOINT_RECORD_LEN: usize = STRING_LEN;
const GRANT_RECORD_LEN: usize = 12;
const STORE_OBJECT_RECORD_LEN: usize = STRING_LEN * 3 + 8;
const STATE_VOLUME_RECORD_LEN: usize = STRING_LEN * 7;
const NETWORK_PORT_RECORD_LEN: usize = STRING_LEN;
const IO_PORT_RECORD_LEN: usize = STRING_LEN + 16;
const MMIO_REGION_RECORD_LEN: usize = STRING_LEN + 16;
const FRAMEBUFFER_RECORD_LEN: usize = STRING_LEN;
const INTERRUPT_LINE_RECORD_LEN: usize = STRING_LEN + 8;
const DMA_REGION_RECORD_LEN: usize = STRING_LEN + 16;
const PCI_DEVICE_RECORD_LEN: usize = STRING_LEN * 2;
const VIRTIO_DEVICE_RECORD_LEN: usize = STRING_LEN * 2;
const NAMESPACE_ENTRY_RECORD_LEN: usize = STRING_LEN + 8;
const VFS_ROOT_RECORD_LEN: usize = STRING_LEN * 2;
const GRAPH_NODE_RECORD_LEN: usize = graph_abi::NODE_RECORD_LEN;
const GRAPH_EDGE_RECORD_LEN: usize = graph_abi::EDGE_RECORD_LEN;
const POLICY_CAPABILITY_RECORD_LEN: usize = STRING_LEN * 2 + 8;
const POLICY_REQUIREMENT_RECORD_LEN: usize = STRING_LEN * 2 + 4;
const POLICY_PROVIDE_RECORD_LEN: usize = STRING_LEN * 2;
const POLICY_MOUNT_RECORD_LEN: usize = STRING_LEN * 4 + 4;
const POLICY_STATE_PATH_RECORD_LEN: usize = STRING_LEN * 3 + 4;
const POLICY_BOOTSTRAP_RECORD_LEN: usize = STRING_LEN * 3 + 8;
const MAX_BOOT_MODULES: usize = 16;
const MAX_PROCESSES: usize = 16;
const MAX_ENDPOINTS: usize = 16;
const MAX_GRANTS: usize = 96;
const MAX_STORE_OBJECTS: usize = 32;
const MAX_STATE_VOLUMES: usize = 4;
const MAX_NETWORK_PORTS: usize = 4;
const MAX_IO_PORT_RANGES: usize = 4;
const MAX_MMIO_REGIONS: usize = 4;
const MAX_FRAMEBUFFERS: usize = 1;
const MAX_INTERRUPT_LINES: usize = 4;
const MAX_DMA_REGIONS: usize = 4;
const MAX_PCI_DEVICES: usize = 4;
const MAX_VIRTIO_DEVICES: usize = 4;
const MAX_NAMESPACES: usize = 4;
const MAX_VFS_ROOTS: usize = 8;
const MAX_GRAPH_NODES: usize = 128;
const MAX_GRAPH_EDGES: usize = 224;
const MAX_POLICY_CAPABILITIES: usize = 128;
const MAX_POLICY_REQUIREMENTS: usize = 160;
const MAX_POLICY_PROVIDES: usize = 64;
const MAX_POLICY_MOUNTS: usize = 96;
const MAX_POLICY_STATE_PATHS: usize = 96;
const MAX_POLICY_BOOTSTRAPS: usize = 96;
const MAX_NAMESPACE_ENTRIES: usize = 4;
const MAX_PROCESS_REFS: usize = 5;
const MAX_PROCESS_MOUNTS: usize = 4;
const PAGE_SIZE: u64 = 4096;
const MAX_DEVICE_MAPPING_LENGTH: u64 = 1 << 30;
const MAX_LEGACY_IRQ_LINE: u64 = 15;
const DMA_KERNEL_ALLOCATED_BASE: u64 = u64::MAX;
const RIGHT_SEND: u16 = 1 << 0;
const RIGHT_RECEIVE: u16 = 1 << 1;
const RIGHT_READ: u16 = 1 << 2;
const RIGHT_WRITE: u16 = 1 << 3;
const RIGHT_SNAPSHOT: u16 = 1 << 4;
const RIGHT_RESTORE: u16 = 1 << 5;
const RIGHT_CONTROL: u16 = 1 << 6;
const RIGHT_BIND: u16 = 1 << 7;
const RIGHT_LISTEN: u16 = 1 << 8;
const RIGHT_MAP: u16 = 1 << 9;
const RIGHT_RESOLVE: u16 = 1 << 10;
const RIGHT_CREATE: u16 = 1 << 11;
const RIGHT_UNLINK: u16 = 1 << 12;
const RIGHT_RENAME: u16 = 1 << 13;
const RIGHT_MOUNT: u16 = 1 << 14;
const BOOTSTRAP_RIGHT_READ: u64 = 1 << 0;
const BOOTSTRAP_RIGHT_WRITE: u64 = 1 << 1;
const BOOTSTRAP_RIGHT_MAP: u64 = 1 << 2;
const BOOTSTRAP_RIGHT_SEND: u64 = 1 << 4;
const BOOTSTRAP_RIGHT_RECEIVE: u64 = 1 << 5;
const BOOTSTRAP_RIGHT_CONTROL: u64 = 1 << 6;
const BOOTSTRAP_RIGHT_ALLOCATE: u64 = 1 << 7;
const BOOTSTRAP_RIGHT_SNAPSHOT: u64 = 1 << 8;
const BOOTSTRAP_RIGHT_RESTORE: u64 = 1 << 9;
const BOOTSTRAP_RIGHT_BIND: u64 = 1 << 10;
const BOOTSTRAP_RIGHT_LISTEN: u64 = 1 << 11;
const BOOTSTRAP_RIGHT_DELEGATE: u64 = 1 << 12;
const BOOTSTRAP_RIGHT_REVOKE: u64 = 1 << 13;
const BOOTSTRAP_RIGHT_INSPECT: u64 = 1 << 14;
const BOOTSTRAP_RIGHT_CREATE: u64 = 1 << 15;
const BOOTSTRAP_RIGHT_START: u64 = 1 << 16;
const BOOTSTRAP_RIGHT_KILL: u64 = 1 << 17;
const BOOTSTRAP_RIGHT_WAIT: u64 = 1 << 18;
const BOOTSTRAP_RIGHT_INSPECT_METADATA: u64 = 1 << 22;
const BOOTSTRAP_RIGHT_RESOLVE: u64 = 1 << 23;
const BOOTSTRAP_RIGHT_UNLINK: u64 = 1 << 24;
const BOOTSTRAP_RIGHT_RENAME: u64 = 1 << 25;
const BOOTSTRAP_RIGHT_MOUNT: u64 = 1 << 26;
const PROCESS_MOUNT_FLAG_BIND: u16 = 1;
const PROCESS_MOUNT_FLAG_READ_ONLY: u16 = 1 << 1;
const OBJECT_ENDPOINT: u16 = 1;
const OBJECT_STORE: u16 = 2;
const OBJECT_STATE: u16 = 3;
const OBJECT_TIMER: u16 = 4;
const OBJECT_NETWORK_PORT: u16 = 5;
const OBJECT_IO_PORT_RANGE: u16 = 6;
const OBJECT_MMIO_REGION: u16 = 7;
const OBJECT_INTERRUPT_LINE: u16 = 8;
const OBJECT_DMA_REGION: u16 = 9;
const OBJECT_PCI_DEVICE: u16 = 10;
const OBJECT_VIRTIO_DEVICE: u16 = 11;
const OBJECT_NAMESPACE: u16 = 12;
const OBJECT_VFS_ROOT: u16 = 13;
const OBJECT_FRAMEBUFFER: u16 = 14;
const OBJECT_SECRET: u16 = 15;
const RECORD_BOOT_MODULE: u16 = 1;
const RECORD_PROCESS: u16 = 2;
const RECORD_ENDPOINT: u16 = 3;
const RECORD_GRANT: u16 = 4;
const RECORD_STORE_OBJECT: u16 = 5;
const RECORD_STATE_VOLUME: u16 = 6;
const RECORD_TIMER: u16 = 7;
const RECORD_GENERATION: u16 = 8;
const RECORD_POLICY: u16 = 9;
const GRAPH_NODE_GENERATION: u16 = graph_abi::NODE_GENERATION;
const GRAPH_NODE_SERVICE: u16 = graph_abi::NODE_SERVICE;
const GRAPH_NODE_ENDPOINT: u16 = graph_abi::NODE_ENDPOINT;
const GRAPH_NODE_STORE_OBJECT: u16 = graph_abi::NODE_STORE_OBJECT;
const GRAPH_NODE_CONFIG: u16 = graph_abi::NODE_CONFIG;
const GRAPH_NODE_STATE_VOLUME: u16 = graph_abi::NODE_STATE_VOLUME;
const GRAPH_NODE_DEVICE: u16 = graph_abi::NODE_DEVICE;
const GRAPH_NODE_NAMESPACE: u16 = graph_abi::NODE_NAMESPACE;
const GRAPH_NODE_VFS_ROOT: u16 = graph_abi::NODE_VFS_ROOT;
const GRAPH_NODE_TIMER: u16 = graph_abi::NODE_TIMER;
const GRAPH_NODE_SECRET: u16 = graph_abi::NODE_SECRET;
const GRAPH_NODE_PACKAGE: u16 = graph_abi::NODE_PACKAGE;
const GRAPH_EDGE_ACTIVATION: u16 = graph_abi::EDGE_ACTIVATION;
const GRAPH_EDGE_CAPABILITY: u16 = graph_abi::EDGE_CAPABILITY;
const RESTART_NEVER: u16 = 0;
const RESTART_ON_FAILURE: u16 = 1;
const RESTART_ALWAYS: u16 = 2;
const SERVICE_CAP_SLOT: u16 = 0;
const SERIAL_CAP_SLOT: u16 = 1;
const READINESS_CAP_SLOT: u16 = 2;
const INIT_SERIAL_CAP_SLOT: u16 = 1;
const INIT_READINESS_CAP_SLOT: u16 = 3;
const INIT_ENDPOINT_AUTH_BASE_SLOT: u16 = 4;
const SERIAL_RESERVED_CAP_SLOT: u16 = 1;
const READINESS_RESERVED_CAP_SLOT: u16 = 2;
const VERTEX_STORE_LOGD_OBJECT_CAP_SLOT: u16 = 7;
const VERTEX_STORE_ECHO_OBJECT_CAP_SLOT: u16 = 8;
const BLOCK_DRIVER_VERTEXFS_FSYNC_FAULT_CAP_SLOT: u16 = 15;
const LOGD_CONFIG_MODULE: &str = "config-logd-v0";
const LOGD_CONFIG_BYTES: &[u8] = b"{\"level\":\"info\",\"sink\":\"serial\"}\n";

pub struct KrustBootIdentity {
    compact_version: u16,
}

pub struct GraphStoreImage {
    pub generation_id: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub records: Vec<u8>,
    pub checksum: u32,
    pub hash: String,
}

impl KrustBootIdentity {
    pub fn release_profile_label(&self) -> String {
        format!(
            "Manifest v1 compact KRUSTBOOTM86 version {}",
            self.compact_version
        )
    }
}

pub fn graph_store_image(manifest: &GenerationManifest) -> Result<GraphStoreImage, String> {
    let plan = derive_plan(manifest)?;
    validate_plan(&plan)?;
    let records = serialize_graph_records(&plan)?;
    let checksum = checksum32(&records);
    let hash = store_hash_hex(&records);
    Ok(GraphStoreImage {
        generation_id: manifest.generation.id.clone(),
        node_count: plan.graph_nodes.len(),
        edge_count: plan.graph_edges.len(),
        records,
        checksum,
        hash,
    })
}

pub fn compile(manifest: &GenerationManifest) -> Result<Vec<u8>, String> {
    let plan = derive_plan(manifest)?;
    validate_plan(&plan)?;
    let graph_records = serialize_graph_records(&plan)?;
    let graph_checksum = checksum32(&graph_records);
    let policy_records = serialize_policy_records(&plan)?;
    let policy_hash = store_hash_hex(&policy_records);

    let mut body = Vec::new();
    body.extend_from_slice(COMPACT_MAGIC);
    push_u16(&mut body, COMPACT_VERSION);
    push_count(&mut body, plan.boot_modules.len(), "boot_modules")?;
    push_count(&mut body, plan.processes.len(), "processes")?;
    push_count(&mut body, plan.endpoints.len(), "endpoints")?;
    push_count(&mut body, plan.grants.len(), "grants")?;
    push_count(&mut body, plan.store_objects.len(), "store_objects")?;
    push_count(&mut body, plan.state_volumes.len(), "state_volumes")?;
    push_count(&mut body, plan.network_ports.len(), "network_ports")?;
    push_count(&mut body, plan.io_ports.len(), "io_ports")?;
    push_count(&mut body, plan.mmio_regions.len(), "mmio_regions")?;
    push_count(&mut body, plan.framebuffers.len(), "framebuffers")?;
    push_count(&mut body, plan.interrupt_lines.len(), "interrupt_lines")?;
    push_count(&mut body, plan.dma_regions.len(), "dma_regions")?;
    push_count(&mut body, plan.pci_devices.len(), "pci_devices")?;
    push_count(&mut body, plan.virtio_devices.len(), "virtio_devices")?;
    push_count(&mut body, plan.namespaces.len(), "namespaces")?;
    push_count(&mut body, plan.vfs_roots.len(), "vfs_roots")?;
    push_fixed_str(&mut body, &manifest.generation.id)?;
    push_fixed_str(
        &mut body,
        manifest.generation.parent.as_deref().unwrap_or_default(),
    )?;
    push_count(&mut body, plan.graph_nodes.len(), "graph_nodes")?;
    push_count(&mut body, plan.graph_edges.len(), "graph_edges")?;
    push_u32(&mut body, graph_checksum);

    for module in &plan.boot_modules {
        push_fixed_str(&mut body, &module.name)?;
        push_fixed_str(&mut body, &module.module_string)?;
    }

    for process in &plan.processes {
        push_fixed_str(&mut body, &process.name)?;
        push_fixed_str(&mut body, &process.module_string)?;
        push_u16(&mut body, u16::from(process.initial));
        push_u16(&mut body, process.restart);
        push_fixed_str(&mut body, &process.service_id)?;
        push_fixed_str(&mut body, &process.health_kind)?;
        push_fixed_str(&mut body, &process.mount_root)?;
        push_process_mount_list(&mut body, &process.mounts)?;
        push_process_ref_list(&mut body, &process.start_after, &plan)?;
        push_endpoint_requirement_list(&mut body, &process.requires_endpoints, &plan)?;
        push_endpoint_ref_list(&mut body, &process.provides_endpoints, &plan)?;
    }

    for endpoint in &plan.endpoints {
        push_fixed_str(&mut body, &endpoint.name)?;
    }

    for grant in &plan.grants {
        push_u16(&mut body, process_index(&plan, &grant.process)? as u16);
        push_u16(&mut body, grant.object_kind);
        push_u16(&mut body, object_index(&plan, grant)? as u16);
        push_u16(&mut body, grant.cap_slot);
        push_u16(&mut body, grant.rights);
        push_u16(&mut body, 0);
    }

    for object in &plan.store_objects {
        push_fixed_str(&mut body, &object.id)?;
        push_fixed_str(&mut body, &object.module_string)?;
        push_fixed_str(&mut body, &object.hash)?;
        push_u64(&mut body, object.size);
    }

    for state in &plan.state_volumes {
        push_fixed_str(&mut body, &state.id)?;
        push_fixed_str(&mut body, &state.owner)?;
        push_fixed_str(&mut body, &state.schema_version)?;
        push_fixed_str(&mut body, &state.storage_class)?;
        push_fixed_str(&mut body, &state.migration_policy)?;
        push_fixed_str(&mut body, &state.retention_policy)?;
        push_fixed_str(&mut body, &state.sharing_policy)?;
    }

    for port in &plan.network_ports {
        push_fixed_str(&mut body, &port.id)?;
    }

    for port in &plan.io_ports {
        push_fixed_str(&mut body, &port.id)?;
        push_u64(&mut body, port.base);
        push_u64(&mut body, port.length);
    }

    for region in &plan.mmio_regions {
        push_fixed_str(&mut body, &region.id)?;
        push_u64(&mut body, region.base);
        push_u64(&mut body, region.length);
    }

    for framebuffer in &plan.framebuffers {
        push_fixed_str(&mut body, &framebuffer.id)?;
    }

    for line in &plan.interrupt_lines {
        push_fixed_str(&mut body, &line.id)?;
        push_u64(&mut body, line.line);
    }

    for region in &plan.dma_regions {
        push_fixed_str(&mut body, &region.id)?;
        push_u64(&mut body, region.base);
        push_u64(&mut body, region.length);
    }

    for device in &plan.pci_devices {
        push_fixed_str(&mut body, &device.id)?;
        push_fixed_str(&mut body, &device.kind)?;
    }

    for device in &plan.virtio_devices {
        push_fixed_str(&mut body, &device.id)?;
        push_fixed_str(&mut body, &device.transport)?;
    }

    for namespace in &plan.namespaces {
        push_fixed_str(&mut body, &namespace.id)?;
        push_count(&mut body, namespace.entries.len(), "namespace_entries")?;
        for entry in &namespace.entries {
            push_fixed_str(&mut body, &entry.path)?;
            push_u16(&mut body, entry.object_kind);
            push_u16(
                &mut body,
                object_index_for_kind(&plan, entry.object_kind, &entry.object_name)? as u16,
            );
            push_u16(&mut body, entry.rights);
            push_u16(&mut body, 0);
        }
    }

    for root in &plan.vfs_roots {
        push_fixed_str(&mut body, &root.id)?;
        push_fixed_str(&mut body, &root.root_path)?;
    }

    body.extend_from_slice(&graph_records);
    push_u16(&mut body, POLICY_VERSION);
    push_count(
        &mut body,
        plan.policy_capabilities.len(),
        "policy_capabilities",
    )?;
    push_count(
        &mut body,
        plan.policy_requirements.len(),
        "policy_requirements",
    )?;
    push_count(&mut body, plan.policy_provides.len(), "policy_provides")?;
    push_count(&mut body, plan.policy_mounts.len(), "policy_mounts")?;
    push_count(
        &mut body,
        plan.policy_state_paths.len(),
        "policy_state_paths",
    )?;
    push_count(&mut body, plan.policy_bootstraps.len(), "policy_bootstraps")?;
    push_fixed_str(&mut body, &policy_hash)?;
    body.extend_from_slice(&policy_records);

    wrap_v1(manifest, &plan, &body)
}

pub fn summary(manifest: &GenerationManifest, output_path: &str, byte_len: usize) -> String {
    let plan = derive_plan(manifest).expect("summary is only called after compile succeeds");

    format!(
        "wrote {output_path}\n\
         format: KrustBoot Manifest v1\n\
         generation: {}\n\
         parent_generation: {}\n\
         boot_modules: {}\n\
         processes: {}\n\
         endpoints: {}\n\
         grants: {}\n\
         store_objects: {}\n\
         state_volumes: {}\n\
         network_ports: {}\n\
         io_ports: {}\n\
         mmio_regions: {}\n\
         framebuffers: {}\n\
         interrupt_lines: {}\n\
         dma_regions: {}\n\
         pci_devices: {}\n\
         virtio_devices: {}\n\
         namespaces: {}\n\
         vfs_roots: {}\n\
         graph_nodes: {}\n\
         graph_edges: {}\n\
         bytes: {byte_len}",
        manifest.generation.id,
        manifest.generation.parent.as_deref().unwrap_or("<none>"),
        plan.boot_modules.len(),
        plan.processes.len(),
        plan.endpoints.len(),
        plan.grants.len(),
        plan.store_objects.len(),
        plan.state_volumes.len(),
        plan.network_ports.len(),
        plan.io_ports.len(),
        plan.mmio_regions.len(),
        plan.framebuffers.len(),
        plan.interrupt_lines.len(),
        plan.dma_regions.len(),
        plan.pci_devices.len(),
        plan.virtio_devices.len(),
        plan.namespaces.len(),
        plan.vfs_roots.len(),
        plan.graph_nodes.len(),
        plan.graph_edges.len()
    )
}

pub fn corrupt(bytes: &[u8], mode: &str) -> Result<Vec<u8>, String> {
    let mut out = bytes.to_vec();
    match mode {
        "truncated" => {
            out.truncate(V1_HEADER_SIZE / 2);
        }
        "bad-magic" => {
            let first = out
                .first_mut()
                .ok_or_else(|| "cannot corrupt empty KrustBoot manifest".to_owned())?;
            *first = b'X';
        }
        "unsupported-version" => {
            if out.len() < 18 {
                return Err("KrustBoot manifest is too short to corrupt version".to_owned());
            }
            out[16..18].copy_from_slice(&u16::MAX.to_le_bytes());
        }
        "out-of-bounds-record" => {
            if out.len() < V1_HEADER_SIZE + V1_RECORD_SIZE {
                return Err("KrustBoot manifest is too short to corrupt record table".to_owned());
            }
            let bad_offset = (out.len() as u32).saturating_add(4096);
            out[V1_HEADER_SIZE + 4..V1_HEADER_SIZE + 8].copy_from_slice(&bad_offset.to_le_bytes());
            rewrite_v1_checksum(&mut out)?;
        }
        "raw-compact" => {
            if out.len() < V1_PAYLOAD_OFFSET {
                return Err("KrustBoot manifest is too short to unwrap compact payload".to_owned());
            }
            out = out[V1_PAYLOAD_OFFSET..].to_vec();
        }
        "old-compact-magic" => {
            if out.len() < V1_PAYLOAD_OFFSET + COMPACT_MAGIC.len() {
                return Err("KrustBoot manifest is too short to rewrite compact magic".to_owned());
            }
            out[V1_PAYLOAD_OFFSET..V1_PAYLOAD_OFFSET + COMPACT_MAGIC.len()]
                .copy_from_slice(b"KRUSTBOOTM79\0\0\0\0");
            rewrite_v1_checksum(&mut out)?;
        }
        "graph-store-checksum" => {
            if out.len() < V1_PAYLOAD_OFFSET + COMPACT_GRAPH_CHECKSUM_OFFSET + 4 {
                return Err(
                    "KrustBoot manifest is too short to corrupt graph-store checksum".to_owned(),
                );
            }
            let offset = V1_PAYLOAD_OFFSET + COMPACT_GRAPH_CHECKSUM_OFFSET;
            let checksum = read_u32_at(&out, offset)? ^ 0x8000_0001;
            out[offset..offset + 4].copy_from_slice(&checksum.to_le_bytes());
            rewrite_v1_checksum(&mut out)?;
        }
        "graph-store-record" => {
            corrupt_graph_store_record(&mut out)?;
            rewrite_v1_checksum(&mut out)?;
        }
        "missing-provider" => {
            corrupt_missing_provider(&mut out)?;
            rewrite_v1_checksum(&mut out)?;
        }
        "policy-version" => {
            corrupt_policy_version(&mut out)?;
            rewrite_v1_checksum(&mut out)?;
        }
        "policy-hash" => {
            corrupt_policy_hash(&mut out)?;
            rewrite_v1_checksum(&mut out)?;
        }
        "policy-excess-grant" => {
            corrupt_policy_excess_grant(&mut out)?;
            rewrite_v1_checksum(&mut out)?;
        }
        "policy-mount-root" => {
            corrupt_policy_mount_root(&mut out)?;
            rewrite_v1_checksum(&mut out)?;
        }
        "policy-state-root" => {
            corrupt_policy_state_root(&mut out)?;
            rewrite_v1_checksum(&mut out)?;
        }
        other => {
            return Err(format!(
                "unknown KrustBoot corruption mode {other}; expected truncated, bad-magic, unsupported-version, out-of-bounds-record, raw-compact, old-compact-magic, graph-store-checksum, graph-store-record, missing-provider, policy-version, policy-hash, policy-excess-grant, policy-mount-root, or policy-state-root"
            ));
        }
    }
    Ok(out)
}

pub fn validate_release_artifact(bytes: &[u8]) -> Result<KrustBootIdentity, String> {
    if bytes.len() < V1_PAYLOAD_OFFSET + COMPACT_MAGIC.len() + 2 {
        return Err("KrustBoot artifact is too short for Manifest v1 compact payload".to_owned());
    }
    if &bytes[..V1_MAGIC.len()] != V1_MAGIC {
        return Err("KrustBoot artifact is not a Manifest v1 wrapper".to_owned());
    }
    let version = read_u16_at(bytes, 16)?;
    if version != V1_VERSION {
        return Err(format!(
            "unsupported KrustBoot Manifest v1 wrapper version {version}; expected {V1_VERSION}"
        ));
    }
    let header_size = read_u16_at(bytes, 18)? as usize;
    if header_size != V1_HEADER_SIZE {
        return Err(format!(
            "unsupported KrustBoot Manifest v1 header size {header_size}; expected {V1_HEADER_SIZE}"
        ));
    }
    let total_size = read_u32_at(bytes, 20)? as usize;
    if total_size != bytes.len() {
        return Err(format!(
            "KrustBoot Manifest v1 total size {total_size} does not match artifact size {}",
            bytes.len()
        ));
    }
    let record_table_offset = read_u32_at(bytes, 24)? as usize;
    if record_table_offset != V1_HEADER_SIZE {
        return Err(format!(
            "unsupported KrustBoot Manifest v1 record table offset {record_table_offset}; expected {V1_HEADER_SIZE}"
        ));
    }
    let record_count = read_u16_at(bytes, 28)? as usize;
    if record_count != V1_RECORD_COUNT {
        return Err(format!(
            "unsupported KrustBoot Manifest v1 record count {record_count}; expected {V1_RECORD_COUNT}"
        ));
    }
    let checksum = read_u32_at(bytes, V1_CHECKSUM_OFFSET)?;
    let computed_checksum = v1_checksum(bytes);
    if checksum != computed_checksum {
        return Err(format!(
            "KrustBoot Manifest v1 checksum mismatch: artifact={checksum:#010x} computed={computed_checksum:#010x}"
        ));
    }

    let mut record = 0;
    while record < V1_RECORD_COUNT {
        let offset = V1_HEADER_SIZE + record * V1_RECORD_SIZE;
        let section_offset = read_u32_at(bytes, offset + 4)? as usize;
        let section_len = read_u32_at(bytes, offset + 8)? as usize;
        if section_offset < V1_PAYLOAD_OFFSET || section_offset > bytes.len() {
            return Err(format!(
                "KrustBoot Manifest v1 record {record} has out-of-bounds offset {section_offset}"
            ));
        }
        if section_offset
            .checked_add(section_len)
            .filter(|end| *end <= bytes.len())
            .is_none()
        {
            return Err(format!(
                "KrustBoot Manifest v1 record {record} has out-of-bounds length {section_len}"
            ));
        }
        record += 1;
    }

    let payload = V1_PAYLOAD_OFFSET;
    if &bytes[payload..payload + COMPACT_MAGIC.len()] != COMPACT_MAGIC {
        return Err("unsupported KrustBoot compact magic; expected KRUSTBOOTM86".to_owned());
    }
    let compact_version = read_u16_at(bytes, payload + COMPACT_MAGIC.len())?;
    if compact_version != COMPACT_VERSION {
        return Err(format!(
            "unsupported KrustBoot compact version {compact_version}; expected {COMPACT_VERSION}"
        ));
    }

    Ok(KrustBootIdentity { compact_version })
}

pub fn explain(manifest: &GenerationManifest) -> Result<String, String> {
    let plan = derive_plan(manifest)?;
    validate_plan(&plan)?;

    let mut out = String::new();
    for service in &manifest.services {
        if service.id == manifest.activation.root_service {
            continue;
        }

        for requirement in &service.requires {
            let Some(capability) = manifest.capability(&requirement.capability) else {
                continue;
            };
            if capability.kind != "ipc-endpoint" {
                continue;
            }

            let Some(provider) = manifest.service(&capability.provider) else {
                continue;
            };
            if !plan
                .processes
                .iter()
                .any(|process| process.service_id == service.id)
            {
                continue;
            }

            let endpoint = endpoint_name(&capability.id);
            let service_label = service_label(service);
            let rights = if requirement.rights.is_empty() {
                "send".to_owned()
            } else {
                requirement.rights.join("|")
            };

            out.push_str(&format!(
                "{} receives {rights} authority to endpoint {endpoint}\n\
                 because it requires {}/{}\n\
                 and {} provides {}\n",
                service.id, requirement.capability, rights, provider.id, capability.id
            ));
            if service_label != service.id {
                out.push_str(&format!("native process: {service_label}\n"));
            }
        }
    }

    if out.is_empty() {
        out.push_str("no native ipc authority derivations\n");
    }

    Ok(out)
}

#[derive(Debug, Clone)]
struct BootPlan {
    boot_modules: Vec<BootModule>,
    processes: Vec<NativeProcess>,
    endpoints: Vec<Endpoint>,
    grants: Vec<Grant>,
    store_objects: Vec<StoreObject>,
    state_volumes: Vec<StateVolume>,
    network_ports: Vec<NetworkPort>,
    io_ports: Vec<IoPortRange>,
    mmio_regions: Vec<MmioRegion>,
    framebuffers: Vec<Framebuffer>,
    interrupt_lines: Vec<InterruptLine>,
    dma_regions: Vec<DmaRegion>,
    pci_devices: Vec<PciDevice>,
    virtio_devices: Vec<VirtioDevice>,
    namespaces: Vec<Namespace>,
    vfs_roots: Vec<VfsRoot>,
    graph_nodes: Vec<GraphNode>,
    graph_edges: Vec<GraphEdge>,
    policy_capabilities: Vec<PolicyCapability>,
    policy_requirements: Vec<PolicyRequirement>,
    policy_provides: Vec<PolicyProvide>,
    policy_mounts: Vec<PolicyMount>,
    policy_state_paths: Vec<PolicyStatePath>,
    policy_bootstraps: Vec<PolicyBootstrap>,
}

#[derive(Debug, Clone)]
struct BootModule {
    name: String,
    module_string: String,
}

#[derive(Debug, Clone)]
struct NativeProcess {
    name: String,
    module_string: String,
    initial: bool,
    service_id: String,
    start_after: Vec<String>,
    requires_endpoints: Vec<EndpointRequirement>,
    provides_endpoints: Vec<String>,
    health_kind: String,
    mount_root: String,
    mounts: Vec<ProcessMount>,
    restart: u16,
}

#[derive(Debug, Clone)]
struct ProcessMount {
    path: String,
    source: String,
    flags: u16,
}

#[derive(Debug, Clone)]
struct Endpoint {
    name: String,
}

#[derive(Debug, Clone)]
struct EndpointRequirement {
    endpoint: String,
    rights: u16,
}

#[derive(Debug, Clone)]
struct Grant {
    process: String,
    object_kind: u16,
    object_name: String,
    cap_slot: u16,
    rights: u16,
}

#[derive(Debug, Clone)]
struct StoreObject {
    id: String,
    module_string: String,
    hash: String,
    size: u64,
}

#[derive(Debug, Clone)]
struct StateVolume {
    id: String,
    owner: String,
    schema_version: String,
    storage_class: String,
    migration_policy: String,
    retention_policy: String,
    sharing_policy: String,
}

#[derive(Debug, Clone)]
struct NetworkPort {
    id: String,
}

#[derive(Debug, Clone)]
struct IoPortRange {
    id: String,
    base: u64,
    length: u64,
}

#[derive(Debug, Clone)]
struct MmioRegion {
    id: String,
    base: u64,
    length: u64,
}

#[derive(Debug, Clone)]
struct Framebuffer {
    id: String,
}

#[derive(Debug, Clone)]
struct InterruptLine {
    id: String,
    line: u64,
}

#[derive(Debug, Clone)]
struct DmaRegion {
    id: String,
    base: u64,
    length: u64,
}

#[derive(Debug, Clone)]
struct PciDevice {
    id: String,
    kind: String,
}

#[derive(Debug, Clone)]
struct VirtioDevice {
    id: String,
    transport: String,
}

#[derive(Debug, Clone)]
struct Namespace {
    id: String,
    entries: Vec<NamespaceEntry>,
}

#[derive(Debug, Clone)]
struct NamespaceEntry {
    path: String,
    object_kind: u16,
    object_name: String,
    rights: u16,
}

#[derive(Debug, Clone)]
struct VfsRoot {
    id: String,
    root_path: String,
}

#[derive(Debug, Clone)]
struct GraphNode {
    kind: u16,
    object_kind: u16,
    id: String,
    label: String,
}

#[derive(Debug, Clone)]
struct GraphEdge {
    kind: u16,
    from: String,
    to: String,
    rights: u16,
    id: String,
}

#[derive(Debug, Clone)]
struct PolicyCapability {
    id: String,
    provider: String,
    object_kind: u16,
    object_index: usize,
    rights: u16,
}

#[derive(Debug, Clone)]
struct PolicyRequirement {
    service: String,
    capability: String,
    rights: u16,
}

#[derive(Debug, Clone)]
struct PolicyProvide {
    service: String,
    capability: String,
}

#[derive(Debug, Clone)]
struct PolicyMount {
    service: String,
    mount_root: String,
    path: String,
    source: String,
    flags: u16,
}

#[derive(Debug, Clone)]
struct PolicyStatePath {
    service: String,
    state: String,
    root: String,
    rights: u16,
}

#[derive(Debug, Clone)]
struct PolicyBootstrap {
    service: String,
    authority: String,
    rule: String,
    rights: u64,
}

fn derive_plan(manifest: &GenerationManifest) -> Result<BootPlan, String> {
    let root_service = manifest
        .service(&manifest.activation.root_service)
        .ok_or_else(|| {
            format!(
                "activation root service {} is not declared",
                manifest.activation.root_service
            )
        })?;
    let init_executable = manifest
        .executable(&manifest.init.executable)
        .ok_or_else(|| {
            format!(
                "init executable {} is not declared",
                manifest.init.executable
            )
        })?;

    let service_order = native_service_closure(manifest, &root_service.id)?;

    let mut processes = Vec::new();
    let mut executable_store_objects = Vec::new();
    let init_name = module_basename(&init_executable.entrypoint);
    push_executable_store_object(
        &mut executable_store_objects,
        manifest,
        &init_executable.store_object,
        &init_name,
    )?;
    processes.push(NativeProcess {
        name: init_name.clone(),
        module_string: init_name.clone(),
        initial: true,
        service_id: root_service.id.clone(),
        start_after: Vec::new(),
        requires_endpoints: Vec::new(),
        provides_endpoints: Vec::new(),
        health_kind: String::new(),
        mount_root: service_mount_root(root_service)?,
        mounts: service_mounts(root_service)?,
        restart: RESTART_NEVER,
    });

    for service_id in &service_order {
        if service_id == &root_service.id {
            continue;
        }
        let service = manifest
            .service(service_id)
            .ok_or_else(|| format!("activation references unknown service {service_id}"))?;
        let executable = manifest.executable(&service.executable).ok_or_else(|| {
            format!(
                "service {} references unknown executable {}",
                service.id, service.executable
            )
        })?;

        let process_name = service_label(service);
        let module_string = module_basename(&executable.entrypoint);
        push_executable_store_object(
            &mut executable_store_objects,
            manifest,
            &executable.store_object,
            &module_string,
        )?;
        processes.push(NativeProcess {
            name: process_name,
            module_string,
            initial: false,
            service_id: service.id.clone(),
            start_after: Vec::new(),
            requires_endpoints: Vec::new(),
            provides_endpoints: Vec::new(),
            health_kind: service
                .health
                .as_ref()
                .map(|health| health.kind.clone())
                .unwrap_or_default(),
            mount_root: service_mount_root(service)?,
            mounts: service_mounts(service)?,
            restart: restart_policy(&service.restart)?,
        });
    }

    let mut endpoints = vec![
        Endpoint {
            name: "serial-log".to_owned(),
        },
        Endpoint {
            name: "readiness".to_owned(),
        },
    ];

    for capability in &manifest.capabilities {
        if capability.kind == "ipc-endpoint"
            && native_process_for_service(&processes, &capability.provider).is_some()
            && native_requirement_exists(manifest, &processes, &capability.id, &root_service.id)
        {
            push_unique_endpoint(&mut endpoints, endpoint_name(&capability.id));
        }
    }

    let mut grants = Vec::new();
    grants.push(Grant {
        process: init_name.clone(),
        object_kind: OBJECT_ENDPOINT,
        object_name: "serial-log".to_owned(),
        cap_slot: INIT_SERIAL_CAP_SLOT,
        rights: RIGHT_SEND,
    });
    grants.push(Grant {
        process: init_name.clone(),
        object_kind: OBJECT_ENDPOINT,
        object_name: "readiness".to_owned(),
        cap_slot: INIT_READINESS_CAP_SLOT,
        rights: RIGHT_RECEIVE,
    });

    for service in &manifest.services {
        let Some(process_name) = native_process_for_service(&processes, &service.id) else {
            continue;
        };
        if service.id == root_service.id {
            continue;
        }

        grants.push(Grant {
            process: process_name.clone(),
            object_kind: OBJECT_ENDPOINT,
            object_name: "serial-log".to_owned(),
            cap_slot: SERIAL_CAP_SLOT,
            rights: RIGHT_SEND,
        });
        if service.health.is_some() {
            grants.push(Grant {
                process: process_name,
                object_kind: OBJECT_ENDPOINT,
                object_name: "readiness".to_owned(),
                cap_slot: READINESS_CAP_SLOT,
                rights: RIGHT_SEND,
            });
        }
    }

    for capability in &manifest.capabilities {
        if capability.kind != "ipc-endpoint" {
            continue;
        }
        if !native_requirement_exists(manifest, &processes, &capability.id, &root_service.id) {
            continue;
        }
        let endpoint = endpoint_name(&capability.id);
        let Some(provider_process) = native_process_for_service(&processes, &capability.provider)
        else {
            continue;
        };
        let provider_slot = provided_endpoint_target_slot_for_capability(
            manifest,
            &processes,
            &root_service.id,
            &capability.provider,
            &capability.id,
        )?;
        let provider_rights = RIGHT_RECEIVE;

        add_process_endpoint_ref(
            &mut processes,
            &capability.provider,
            endpoint.clone(),
            EndpointRefKind::Provides,
        );
        grants.push(Grant {
            process: provider_process,
            object_kind: OBJECT_ENDPOINT,
            object_name: endpoint.clone(),
            cap_slot: provider_slot,
            rights: provider_rights,
        });

        grants.push(Grant {
            process: init_name.clone(),
            object_kind: OBJECT_ENDPOINT,
            object_name: endpoint.clone(),
            cap_slot: init_endpoint_auth_slot(&endpoints, &endpoint)?,
            rights: RIGHT_SEND,
        });

        for service in &manifest.services {
            if service.id == root_service.id {
                continue;
            }
            for requirement in service
                .requires
                .iter()
                .filter(|requirement| requirement.capability == capability.id)
            {
                if native_process_for_service(&processes, &service.id).is_none() {
                    continue;
                }
                let consumer_rights =
                    endpoint_rights_mask(&requirement.rights, &capability.rights, &capability.id)?;
                add_process_endpoint_ref(
                    &mut processes,
                    &service.id,
                    endpoint.clone(),
                    EndpointRefKind::Requires(consumer_rights),
                );
                if requirement_starts_after_provider(capability) {
                    add_process_start_after(&mut processes, &service.id, &capability.provider)?;
                }
            }
        }
    }

    for service in &manifest.services {
        if service.id == root_service.id
            || native_process_for_service(&processes, &service.id).is_none()
        {
            continue;
        }
        for dependency in &service.lifecycle.start_after {
            add_process_start_after(&mut processes, &service.id, dependency)?;
        }
        for requirement in &service.requires {
            let Some(capability) = manifest.capability(&requirement.capability) else {
                return Err(format!(
                    "service {} requires unknown capability {}",
                    service.id, requirement.capability
                ));
            };
            if manifest.service(&capability.provider).is_some()
                && requirement_starts_after_provider(capability)
            {
                add_process_start_after(&mut processes, &service.id, &capability.provider)?;
            }
        }
    }

    let mut store_objects = executable_store_objects;
    let state_volumes = manifest
        .state_volumes
        .iter()
        .map(|state| {
            Ok(StateVolume {
                id: state.id.clone(),
                owner: state.owner.clone(),
                schema_version: compact_state_field(state, "schemaVersion", &state.schema_version)?,
                storage_class: compact_state_field(state, "storageClass", &state.storage_class)?,
                migration_policy: compact_policy_mode(
                    state,
                    "migrationPolicy",
                    &state.migration_policy,
                )?,
                retention_policy: compact_policy_mode(
                    state,
                    "retentionPolicy",
                    &state.retention_policy,
                )?,
                sharing_policy: compact_policy_mode(state, "sharingPolicy", &state.sharing_policy)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut network_ports = Vec::new();
    let mut io_ports = Vec::new();
    let mut mmio_regions = Vec::new();
    let mut framebuffers = Vec::new();
    let mut interrupt_lines = Vec::new();
    let mut dma_regions = Vec::new();
    let mut pci_devices = Vec::new();
    let mut virtio_devices = Vec::new();
    let mut namespaces = Vec::new();
    let mut vfs_roots = Vec::new();
    let mut next_object_slots = initial_object_cap_slots(&processes);
    add_vertex_store_verifier_grants(&mut grants, &store_objects, &processes);
    reserve_vertex_store_verifier_slots(&mut next_object_slots, &processes);
    for service in &manifest.services {
        let Some(process_name) = native_process_for_service(&processes, &service.id) else {
            continue;
        };
        if service.id == root_service.id {
            continue;
        }
        for requirement in &service.requires {
            let Some(capability) = manifest.capability(&requirement.capability) else {
                return Err(format!(
                    "service {} requires unknown capability {}",
                    service.id, requirement.capability
                ));
            };
            match capability.kind.as_str() {
                "store-object" => {
                    let store = manifest.store_object(&capability.provider).ok_or_else(|| {
                        format!(
                            "capability {} references unknown store object {}",
                            capability.id, capability.provider
                        )
                    })?;
                    push_unique_store_object(&mut store_objects, store)?;
                    let cap_slot = if process_name == "block-driver"
                        && capability.id == "cap:block-driver.vertexfs-fsync-fault-token"
                    {
                        BLOCK_DRIVER_VERTEXFS_FSYNC_FAULT_CAP_SLOT
                    } else {
                        next_object_cap_slot(&mut next_object_slots, &process_name)?
                    };
                    grants.push(Grant {
                        process: process_name.clone(),
                        object_kind: OBJECT_STORE,
                        object_name: store.id.clone(),
                        cap_slot,
                        rights: rights_mask(
                            &requirement.rights,
                            &capability.rights,
                            &capability.id,
                        )?,
                    });
                }
                "state-volume" => {
                    return Err(format!(
                        "native KrustBoot does not grant direct state-volume capability {}; use a VFS-root capability for mounted state instead",
                        capability.id
                    ));
                }
                "timer" => {
                    grants.push(Grant {
                        process: process_name.clone(),
                        object_kind: OBJECT_TIMER,
                        object_name: "monotonic-timer".to_owned(),
                        cap_slot: next_object_cap_slot(&mut next_object_slots, &process_name)?,
                        rights: rights_mask(
                            &requirement.rights,
                            &capability.rights,
                            &capability.id,
                        )?,
                    });
                }
                "network-port" => {
                    push_unique_network_port(&mut network_ports, capability.id.clone());
                    grants.push(Grant {
                        process: process_name.clone(),
                        object_kind: OBJECT_NETWORK_PORT,
                        object_name: capability.id.clone(),
                        cap_slot: next_object_cap_slot(&mut next_object_slots, &process_name)?,
                        rights: rights_mask(
                            &requirement.rights,
                            &capability.rights,
                            &capability.id,
                        )?,
                    });
                }
                "namespace" => {
                    push_unique_namespace(&mut namespaces, manifest, capability)?;
                    grants.push(Grant {
                        process: process_name.clone(),
                        object_kind: OBJECT_NAMESPACE,
                        object_name: capability.id.clone(),
                        cap_slot: next_object_cap_slot(&mut next_object_slots, &process_name)?,
                        rights: rights_mask(
                            &requirement.rights,
                            &capability.rights,
                            &capability.id,
                        )?,
                    });
                }
                "vfs-root" => {
                    push_unique_vfs_root(&mut vfs_roots, capability)?;
                    let rights =
                        rights_mask(&requirement.rights, &capability.rights, &capability.id)?;
                    validate_state_vfs_grant_policy(manifest, service, capability, rights)?;
                    grants.push(Grant {
                        process: process_name.clone(),
                        object_kind: OBJECT_VFS_ROOT,
                        object_name: capability.id.clone(),
                        cap_slot: next_object_cap_slot(&mut next_object_slots, &process_name)?,
                        rights,
                    });
                }
                "io-port" => {
                    push_unique_io_port(&mut io_ports, manifest, capability)?;
                    grants.push(Grant {
                        process: process_name.clone(),
                        object_kind: OBJECT_IO_PORT_RANGE,
                        object_name: capability.id.clone(),
                        cap_slot: next_object_cap_slot(&mut next_object_slots, &process_name)?,
                        rights: rights_mask(
                            &requirement.rights,
                            &capability.rights,
                            &capability.id,
                        )?,
                    });
                }
                "mmio-region" => {
                    push_unique_mmio_region(&mut mmio_regions, manifest, capability)?;
                    grants.push(Grant {
                        process: process_name.clone(),
                        object_kind: OBJECT_MMIO_REGION,
                        object_name: capability.id.clone(),
                        cap_slot: next_object_cap_slot(&mut next_object_slots, &process_name)?,
                        rights: rights_mask(
                            &requirement.rights,
                            &capability.rights,
                            &capability.id,
                        )?,
                    });
                }
                "framebuffer" => {
                    push_unique_framebuffer(&mut framebuffers, manifest, capability)?;
                    grants.push(Grant {
                        process: process_name.clone(),
                        object_kind: OBJECT_FRAMEBUFFER,
                        object_name: capability.id.clone(),
                        cap_slot: next_object_cap_slot(&mut next_object_slots, &process_name)?,
                        rights: rights_mask(
                            &requirement.rights,
                            &capability.rights,
                            &capability.id,
                        )?,
                    });
                }
                "interrupt-line" => {
                    push_unique_interrupt_line(&mut interrupt_lines, manifest, capability)?;
                    grants.push(Grant {
                        process: process_name.clone(),
                        object_kind: OBJECT_INTERRUPT_LINE,
                        object_name: capability.id.clone(),
                        cap_slot: next_object_cap_slot(&mut next_object_slots, &process_name)?,
                        rights: rights_mask(
                            &requirement.rights,
                            &capability.rights,
                            &capability.id,
                        )?,
                    });
                }
                "dma-region" => {
                    push_unique_dma_region(&mut dma_regions, manifest, capability)?;
                    grants.push(Grant {
                        process: process_name.clone(),
                        object_kind: OBJECT_DMA_REGION,
                        object_name: capability.id.clone(),
                        cap_slot: next_object_cap_slot(&mut next_object_slots, &process_name)?,
                        rights: rights_mask(
                            &requirement.rights,
                            &capability.rights,
                            &capability.id,
                        )?,
                    });
                }
                "ipc-endpoint" | "clock" => {}
                other => {
                    return Err(format!(
                        "native KrustBoot does not implement capability kind {other} required by {} via {}",
                        service.id, capability.id
                    ));
                }
            }
        }

        for config_id in &service.configs {
            let store = manifest.store_object(config_id).ok_or_else(|| {
                format!(
                    "service {} references unknown native config object {}",
                    service.id, config_id
                )
            })?;
            if store.kind != "config" {
                return Err(format!(
                    "service {} config {} references store object kind {}",
                    service.id, config_id, store.kind
                ));
            }
            push_unique_store_object(&mut store_objects, store)?;
            grants.push(Grant {
                process: process_name.clone(),
                object_kind: OBJECT_STORE,
                object_name: store.id.clone(),
                cap_slot: next_object_cap_slot(&mut next_object_slots, &process_name)?,
                rights: RIGHT_READ,
            });
        }
    }

    grant_native_driver_devices(
        manifest,
        &processes,
        &mut grants,
        &mut pci_devices,
        &mut virtio_devices,
        &mut next_object_slots,
        &root_service.id,
    )?;
    grant_network_port_provider_caps(
        manifest,
        &processes,
        &network_ports,
        &mut grants,
        &mut next_object_slots,
        &root_service.id,
    )?;

    let mut boot_modules = Vec::new();
    for process in &processes {
        if !boot_modules
            .iter()
            .any(|module: &BootModule| module.module_string == process.module_string)
        {
            boot_modules.push(BootModule {
                name: process.name.clone(),
                module_string: process.module_string.clone(),
            });
        }
    }

    let mut plan = BootPlan {
        boot_modules,
        processes,
        endpoints,
        grants,
        store_objects,
        state_volumes,
        network_ports,
        io_ports,
        mmio_regions,
        framebuffers,
        interrupt_lines,
        dma_regions,
        pci_devices,
        virtio_devices,
        namespaces,
        vfs_roots,
        graph_nodes: Vec::new(),
        graph_edges: Vec::new(),
        policy_capabilities: Vec::new(),
        policy_requirements: Vec::new(),
        policy_provides: Vec::new(),
        policy_mounts: Vec::new(),
        policy_state_paths: Vec::new(),
        policy_bootstraps: Vec::new(),
    };
    derive_graph_store(manifest, &mut plan)?;
    derive_policy_facts(manifest, &mut plan)?;
    Ok(plan)
}

fn derive_graph_store(manifest: &GenerationManifest, plan: &mut BootPlan) -> Result<(), String> {
    push_graph_node(
        &mut plan.graph_nodes,
        GRAPH_NODE_GENERATION,
        0,
        manifest.generation.id.clone(),
        manifest.generation.description.clone(),
    )?;

    for package in linked_package_ids(manifest)? {
        push_graph_node(
            &mut plan.graph_nodes,
            GRAPH_NODE_PACKAGE,
            0,
            package.to_owned(),
            package.to_owned(),
        )?;
    }

    for process in &plan.processes {
        push_graph_node(
            &mut plan.graph_nodes,
            GRAPH_NODE_SERVICE,
            0,
            process_graph_node_id(process),
            process.name.clone(),
        )?;
    }

    for endpoint in &plan.endpoints {
        push_graph_node(
            &mut plan.graph_nodes,
            GRAPH_NODE_ENDPOINT,
            OBJECT_ENDPOINT,
            endpoint.name.clone(),
            endpoint.name.clone(),
        )?;
    }

    push_graph_node(
        &mut plan.graph_nodes,
        GRAPH_NODE_TIMER,
        OBJECT_TIMER,
        "monotonic-timer".to_owned(),
        "monotonic-timer".to_owned(),
    )?;

    for object in &plan.store_objects {
        let kind = if object.id.starts_with("config:") {
            GRAPH_NODE_CONFIG
        } else {
            GRAPH_NODE_STORE_OBJECT
        };
        push_graph_node(
            &mut plan.graph_nodes,
            kind,
            OBJECT_STORE,
            object.id.clone(),
            object.module_string.clone(),
        )?;
    }

    for state in &plan.state_volumes {
        push_graph_node(
            &mut plan.graph_nodes,
            GRAPH_NODE_STATE_VOLUME,
            OBJECT_STATE,
            state.id.clone(),
            state_policy_label(state),
        )?;
    }

    for port in &plan.network_ports {
        push_graph_device_node(
            &mut plan.graph_nodes,
            OBJECT_NETWORK_PORT,
            &port.id,
            &port.id,
        )?;
    }
    for port in &plan.io_ports {
        push_graph_device_node(
            &mut plan.graph_nodes,
            OBJECT_IO_PORT_RANGE,
            &port.id,
            &port.id,
        )?;
    }
    for region in &plan.mmio_regions {
        push_graph_device_node(
            &mut plan.graph_nodes,
            OBJECT_MMIO_REGION,
            &region.id,
            &region.id,
        )?;
    }
    for framebuffer in &plan.framebuffers {
        push_graph_device_node(
            &mut plan.graph_nodes,
            OBJECT_FRAMEBUFFER,
            &framebuffer.id,
            "limine-boot-framebuffer",
        )?;
    }
    for line in &plan.interrupt_lines {
        push_graph_device_node(
            &mut plan.graph_nodes,
            OBJECT_INTERRUPT_LINE,
            &line.id,
            &line.id,
        )?;
    }
    for region in &plan.dma_regions {
        push_graph_device_node(
            &mut plan.graph_nodes,
            OBJECT_DMA_REGION,
            &region.id,
            &region.id,
        )?;
    }
    for device in &plan.pci_devices {
        push_graph_device_node(
            &mut plan.graph_nodes,
            OBJECT_PCI_DEVICE,
            &device.id,
            &device.kind,
        )?;
    }
    for device in &plan.virtio_devices {
        push_graph_device_node(
            &mut plan.graph_nodes,
            OBJECT_VIRTIO_DEVICE,
            &device.id,
            &device.transport,
        )?;
    }

    for namespace in &plan.namespaces {
        push_graph_node(
            &mut plan.graph_nodes,
            GRAPH_NODE_NAMESPACE,
            OBJECT_NAMESPACE,
            namespace.id.clone(),
            namespace.id.clone(),
        )?;
    }

    for root in &plan.vfs_roots {
        push_graph_node(
            &mut plan.graph_nodes,
            GRAPH_NODE_VFS_ROOT,
            OBJECT_VFS_ROOT,
            root.id.clone(),
            root.root_path.clone(),
        )?;
    }

    for secret in &manifest.secrets {
        push_graph_node(
            &mut plan.graph_nodes,
            GRAPH_NODE_SECRET,
            0,
            secret.id.clone(),
            secret.name.clone(),
        )?;
    }
    if plan.processes.iter().any(|process| process.name == "logd")
        && manifest.secret("secret:logd-token").is_none()
    {
        push_graph_node(
            &mut plan.graph_nodes,
            GRAPH_NODE_SECRET,
            0,
            "secret:logd-token".to_owned(),
            "logd-token".to_owned(),
        )?;
    }

    for (index, process) in plan.processes.iter().enumerate() {
        push_graph_edge(
            &plan.graph_nodes,
            &mut plan.graph_edges,
            GRAPH_EDGE_ACTIVATION,
            &manifest.generation.id,
            &process_graph_node_id(process),
            0,
            format!("activate:{index}"),
        )?;
        for dependency in &process.start_after {
            let dependency = plan
                .processes
                .iter()
                .find(|candidate| candidate.name == *dependency)
                .ok_or_else(|| {
                    format!(
                        "graph activation edge for {} references unknown process {}",
                        process.name, dependency
                    )
                })?;
            let edge_index = plan.graph_edges.len();
            push_graph_edge(
                &plan.graph_nodes,
                &mut plan.graph_edges,
                GRAPH_EDGE_ACTIVATION,
                &process_graph_node_id(dependency),
                &process_graph_node_id(process),
                0,
                format!("start:{edge_index}"),
            )?;
        }
    }

    for (index, grant) in plan.grants.iter().enumerate() {
        let process = plan
            .processes
            .iter()
            .find(|process| process.name == grant.process)
            .ok_or_else(|| format!("graph grant references unknown process {}", grant.process))?;
        let target = graph_target_id_for_grant(grant);
        push_graph_edge(
            &plan.graph_nodes,
            &mut plan.graph_edges,
            GRAPH_EDGE_CAPABILITY,
            &process_graph_node_id(process),
            &target,
            grant.rights,
            format!("grant:{index}"),
        )?;
    }

    if plan.processes.iter().any(|process| process.name == "logd") {
        let logd = plan
            .processes
            .iter()
            .find(|process| process.name == "logd")
            .map(process_graph_node_id)
            .unwrap_or_else(|| "svc:logd".to_owned());
        push_graph_edge(
            &plan.graph_nodes,
            &mut plan.graph_edges,
            GRAPH_EDGE_CAPABILITY,
            &logd,
            "secret:logd-token",
            RIGHT_READ,
            "grant:secret-logd-token".to_owned(),
        )?;
    }

    Ok(())
}

fn derive_policy_facts(manifest: &GenerationManifest, plan: &mut BootPlan) -> Result<(), String> {
    for process in &plan.processes {
        let service = process_graph_node_id(process);
        push_policy_mount(
            &mut plan.policy_mounts,
            PolicyMount {
                service: service.clone(),
                mount_root: process.mount_root.clone(),
                path: String::new(),
                source: String::new(),
                flags: 0,
            },
        )?;
        for mount in &process.mounts {
            push_policy_mount(
                &mut plan.policy_mounts,
                PolicyMount {
                    service: service.clone(),
                    mount_root: process.mount_root.clone(),
                    path: mount.path.clone(),
                    source: mount.source.clone(),
                    flags: mount.flags,
                },
            )?;
        }
    }

    for capability in &manifest.capabilities {
        let Some((object_kind, object_index)) = policy_object_ref_for_capability(plan, capability)
        else {
            continue;
        };
        let rights = declared_rights_mask(&capability.rights, &capability.id)?;
        push_policy_capability(
            &mut plan.policy_capabilities,
            PolicyCapability {
                id: capability.id.clone(),
                provider: capability.provider.clone(),
                object_kind,
                object_index,
                rights,
            },
        )?;

        if capability.kind == "network-port"
            && native_process_for_service(&plan.processes, &capability.provider).is_some()
        {
            push_policy_capability(
                &mut plan.policy_capabilities,
                PolicyCapability {
                    id: capability.id.clone(),
                    provider: capability.provider.clone(),
                    object_kind,
                    object_index,
                    rights: RIGHT_CONTROL,
                },
            )?;
            push_policy_requirement(
                &mut plan.policy_requirements,
                PolicyRequirement {
                    service: capability.provider.clone(),
                    capability: capability.id.clone(),
                    rights: RIGHT_CONTROL,
                },
            )?;
        }
    }

    for object in &plan.store_objects {
        let object_index = store_index(plan, &object.id)?;
        push_policy_capability(
            &mut plan.policy_capabilities,
            PolicyCapability {
                id: object.id.clone(),
                provider: object.id.clone(),
                object_kind: OBJECT_STORE,
                object_index,
                rights: RIGHT_READ,
            },
        )?;
    }

    for service in &manifest.services {
        if native_process_for_service(&plan.processes, &service.id).is_none() {
            continue;
        }

        for requirement in &service.requires {
            let Some(capability) = manifest.capability(&requirement.capability) else {
                return Err(format!(
                    "service {} requires unknown capability {}",
                    service.id, requirement.capability
                ));
            };
            if policy_object_ref_for_capability(plan, capability).is_none() {
                continue;
            }
            let rights = if capability.kind == "ipc-endpoint" {
                endpoint_rights_mask(&requirement.rights, &capability.rights, &capability.id)?
            } else {
                rights_mask(&requirement.rights, &capability.rights, &capability.id)?
            };
            push_policy_requirement(
                &mut plan.policy_requirements,
                PolicyRequirement {
                    service: service.id.clone(),
                    capability: capability.id.clone(),
                    rights,
                },
            )?;
        }

        for config_id in &service.configs {
            if plan
                .store_objects
                .iter()
                .any(|object| object.id == *config_id)
            {
                push_policy_requirement(
                    &mut plan.policy_requirements,
                    PolicyRequirement {
                        service: service.id.clone(),
                        capability: config_id.clone(),
                        rights: RIGHT_READ,
                    },
                )?;
            }
        }

        for provided in &service.provides {
            let Some(capability) = manifest.capability(provided) else {
                return Err(format!(
                    "service {} provides unknown capability {}",
                    service.id, provided
                ));
            };
            if capability.kind == "ipc-endpoint"
                && capability.provider == service.id
                && let Some((object_kind, object_index)) =
                    policy_object_ref_for_capability(plan, capability)
            {
                push_policy_capability(
                    &mut plan.policy_capabilities,
                    PolicyCapability {
                        id: capability.id.clone(),
                        provider: capability.provider.clone(),
                        object_kind,
                        object_index,
                        rights: RIGHT_RECEIVE,
                    },
                )?;
                push_policy_provide(
                    &mut plan.policy_provides,
                    PolicyProvide {
                        service: service.id.clone(),
                        capability: provided.clone(),
                    },
                )?;
            }
        }
    }

    add_state_path_policy(plan)?;
    add_secret_policy(plan)?;
    add_bootstrap_policy(plan)?;

    for device in &manifest.devices {
        if native_process_for_service(&plan.processes, &device.driver).is_none() {
            continue;
        }
        if let Some(index) = plan
            .pci_devices
            .iter()
            .position(|candidate| candidate.id == device.id)
        {
            push_driver_policy_fact(plan, &device.driver, &device.id, OBJECT_PCI_DEVICE, index)?;
        }
        if let Some(index) = plan
            .virtio_devices
            .iter()
            .position(|candidate| candidate.id == device.id)
        {
            push_driver_policy_fact(
                plan,
                &device.driver,
                &device.id,
                OBJECT_VIRTIO_DEVICE,
                index,
            )?;
        }
    }

    add_vertex_store_verifier_policy(plan)?;
    Ok(())
}

fn add_state_path_policy(plan: &mut BootPlan) -> Result<(), String> {
    let grants = plan.grants.clone();
    for grant in grants {
        if grant.object_kind != OBJECT_VFS_ROOT {
            continue;
        }
        let Some(process) = plan
            .processes
            .iter()
            .find(|candidate| candidate.name == grant.process)
        else {
            continue;
        };
        let root = plan
            .vfs_roots
            .iter()
            .find(|candidate| candidate.id == grant.object_name)
            .ok_or_else(|| {
                format!(
                    "VFS-root grant references unknown root {}",
                    grant.object_name
                )
            })?;
        let effective_root = root.root_path.clone();
        for state in state_volumes_covered_by_compact_root(&plan.state_volumes, &effective_root)? {
            push_policy_state_path(
                &mut plan.policy_state_paths,
                PolicyStatePath {
                    service: process_graph_node_id(process),
                    state: state.id.clone(),
                    root: effective_root.clone(),
                    rights: grant.rights,
                },
            )?;
        }
    }
    Ok(())
}

fn add_secret_policy(plan: &mut BootPlan) -> Result<(), String> {
    let Some(logd) = plan.processes.iter().find(|process| process.name == "logd") else {
        return Ok(());
    };
    let service = process_graph_node_id(logd);
    let rights = RIGHT_READ;
    push_policy_capability(
        &mut plan.policy_capabilities,
        PolicyCapability {
            id: "secret:logd-token".to_owned(),
            provider: service.clone(),
            object_kind: OBJECT_SECRET,
            object_index: 0,
            rights,
        },
    )?;
    push_policy_requirement(
        &mut plan.policy_requirements,
        PolicyRequirement {
            service: service.clone(),
            capability: "secret:logd-token".to_owned(),
            rights,
        },
    )?;
    push_policy_bootstrap(
        &mut plan.policy_bootstraps,
        PolicyBootstrap {
            service,
            authority: "secret:logd-token".to_owned(),
            rule: "native-secret".to_owned(),
            rights: BOOTSTRAP_RIGHT_READ | BOOTSTRAP_RIGHT_INSPECT_METADATA,
        },
    )
}

fn add_bootstrap_policy(plan: &mut BootPlan) -> Result<(), String> {
    let grants = plan.grants.clone();
    for grant in grants {
        if grant.object_kind != OBJECT_ENDPOINT {
            continue;
        }
        let Some(process) = plan
            .processes
            .iter()
            .find(|candidate| candidate.name == grant.process)
        else {
            continue;
        };
        let rule = if grant.object_name == "serial-log" && grant.rights == RIGHT_SEND {
            "serial-log"
        } else if grant.object_name == "readiness"
            && process.initial
            && grant.rights == RIGHT_RECEIVE
        {
            "readiness-receive"
        } else if grant.object_name == "readiness" && !process.initial && grant.rights == RIGHT_SEND
        {
            "readiness-send"
        } else if process.initial && grant.rights == RIGHT_SEND {
            "init-endpoint-delegation"
        } else {
            continue;
        };
        push_policy_bootstrap(
            &mut plan.policy_bootstraps,
            PolicyBootstrap {
                service: process_graph_node_id(process),
                authority: format!("endpoint:{}", grant.object_name),
                rule: rule.to_owned(),
                rights: bootstrap_rights_from_compact(grant.rights),
            },
        )?;
    }

    if let Some(init) = plan.processes.iter().find(|process| process.initial) {
        let init_service = process_graph_node_id(init);
        for (authority, rule, rights) in [
            (
                "boot-module:krustboot-manifest",
                "initial-manifest",
                RIGHT_READ,
            ),
            ("process-control", "initial-process-control", RIGHT_CONTROL),
            (
                "timer:monotonic-timer",
                "initial-restart-timer",
                RIGHT_CONTROL,
            ),
        ] {
            push_policy_bootstrap(
                &mut plan.policy_bootstraps,
                PolicyBootstrap {
                    service: init_service.clone(),
                    authority: authority.to_owned(),
                    rule: rule.to_owned(),
                    rights: if rule == "initial-process-control" {
                        BOOTSTRAP_RIGHT_CONTROL
                            | BOOTSTRAP_RIGHT_ALLOCATE
                            | BOOTSTRAP_RIGHT_DELEGATE
                            | BOOTSTRAP_RIGHT_REVOKE
                            | BOOTSTRAP_RIGHT_INSPECT
                            | BOOTSTRAP_RIGHT_CREATE
                            | BOOTSTRAP_RIGHT_START
                            | BOOTSTRAP_RIGHT_KILL
                            | BOOTSTRAP_RIGHT_WAIT
                    } else {
                        bootstrap_rights_from_compact(rights)
                    },
                },
            )?;
        }
    }

    if !plan.state_volumes.is_empty() {
        add_service_bootstrap_pair(
            plan,
            "vertex-state",
            "endpoint:state-vfs-request",
            "state-vfs-request",
            RIGHT_RECEIVE,
        )?;
        add_service_bootstrap_pair(
            plan,
            "vertex-state",
            "endpoint:state-vfs-reply",
            "state-vfs-reply",
            RIGHT_SEND,
        )?;
    }
    add_service_bootstrap_pair(
        plan,
        "block-driver",
        "endpoint:vertexfs-device-request",
        "vertexfs-device-request",
        RIGHT_RECEIVE,
    )?;
    add_service_bootstrap_pair(
        plan,
        "block-driver",
        "endpoint:vertexfs-device-reply",
        "vertexfs-device-reply",
        RIGHT_SEND,
    )?;
    add_service_bootstrap_pair(
        plan,
        "block-driver",
        "endpoint:generation-metadata-block-request",
        "generation-metadata-block-request",
        RIGHT_RECEIVE,
    )?;
    add_service_bootstrap_pair(
        plan,
        "block-driver",
        "endpoint:generation-metadata-block-reply",
        "generation-metadata-block-reply",
        RIGHT_SEND,
    )?;
    add_service_bootstrap_pair(
        plan,
        "gen-manager",
        "endpoint:generation-metadata-block-request",
        "generation-metadata-block-request",
        RIGHT_SEND,
    )?;
    add_service_bootstrap_pair(
        plan,
        "gen-manager",
        "endpoint:generation-metadata-block-reply",
        "generation-metadata-block-reply",
        RIGHT_RECEIVE,
    )
}

fn add_service_bootstrap_pair(
    plan: &mut BootPlan,
    process_name: &str,
    authority: &str,
    rule: &str,
    rights: u16,
) -> Result<(), String> {
    let Some(process) = plan
        .processes
        .iter()
        .find(|candidate| candidate.name == process_name)
    else {
        return Ok(());
    };
    push_policy_bootstrap(
        &mut plan.policy_bootstraps,
        PolicyBootstrap {
            service: process_graph_node_id(process),
            authority: authority.to_owned(),
            rule: rule.to_owned(),
            rights: bootstrap_rights_from_compact(rights),
        },
    )
}

fn bootstrap_rights_from_compact(rights: u16) -> u64 {
    let mut out = 0;
    if rights & RIGHT_SEND != 0 {
        out |= BOOTSTRAP_RIGHT_SEND;
    }
    if rights & RIGHT_RECEIVE != 0 {
        out |= BOOTSTRAP_RIGHT_RECEIVE;
    }
    if rights & RIGHT_READ != 0 {
        out |= BOOTSTRAP_RIGHT_READ;
    }
    if rights & RIGHT_WRITE != 0 {
        out |= BOOTSTRAP_RIGHT_WRITE;
    }
    if rights & RIGHT_SNAPSHOT != 0 {
        out |= BOOTSTRAP_RIGHT_SNAPSHOT;
    }
    if rights & RIGHT_RESTORE != 0 {
        out |= BOOTSTRAP_RIGHT_RESTORE;
    }
    if rights & RIGHT_CONTROL != 0 {
        out |= BOOTSTRAP_RIGHT_CONTROL;
    }
    if rights & RIGHT_BIND != 0 {
        out |= BOOTSTRAP_RIGHT_BIND;
    }
    if rights & RIGHT_LISTEN != 0 {
        out |= BOOTSTRAP_RIGHT_LISTEN;
    }
    if rights & RIGHT_MAP != 0 {
        out |= BOOTSTRAP_RIGHT_MAP;
    }
    if rights & RIGHT_RESOLVE != 0 {
        out |= BOOTSTRAP_RIGHT_RESOLVE;
    }
    if rights & RIGHT_CREATE != 0 {
        out |= BOOTSTRAP_RIGHT_CREATE;
    }
    if rights & RIGHT_UNLINK != 0 {
        out |= BOOTSTRAP_RIGHT_UNLINK;
    }
    if rights & RIGHT_RENAME != 0 {
        out |= BOOTSTRAP_RIGHT_RENAME;
    }
    if rights & RIGHT_MOUNT != 0 {
        out |= BOOTSTRAP_RIGHT_MOUNT;
    }
    out
}

fn policy_object_ref_for_capability(
    plan: &BootPlan,
    capability: &vertex_ir::Capability,
) -> Option<(u16, usize)> {
    let (kind, object_name) = match capability.kind.as_str() {
        "ipc-endpoint" => (OBJECT_ENDPOINT, endpoint_name(&capability.id)),
        "store-object" => (OBJECT_STORE, capability.provider.clone()),
        "timer" => (OBJECT_TIMER, "monotonic-timer".to_owned()),
        "network-port" => (OBJECT_NETWORK_PORT, capability.id.clone()),
        "io-port" => (OBJECT_IO_PORT_RANGE, capability.id.clone()),
        "mmio-region" => (OBJECT_MMIO_REGION, capability.id.clone()),
        "framebuffer" => (OBJECT_FRAMEBUFFER, capability.id.clone()),
        "interrupt-line" => (OBJECT_INTERRUPT_LINE, capability.id.clone()),
        "dma-region" => (OBJECT_DMA_REGION, capability.id.clone()),
        "pci-device" => (OBJECT_PCI_DEVICE, capability.provider.clone()),
        "virtio-device" => (OBJECT_VIRTIO_DEVICE, capability.provider.clone()),
        "namespace" => (OBJECT_NAMESPACE, capability.id.clone()),
        "vfs-root" => (OBJECT_VFS_ROOT, capability.id.clone()),
        "clock" | "state-volume" => return None,
        _ => return None,
    };
    object_index_for_kind(plan, kind, &object_name)
        .ok()
        .map(|index| (kind, index))
}

fn push_driver_policy_fact(
    plan: &mut BootPlan,
    service: &str,
    device_id: &str,
    object_kind: u16,
    object_index: usize,
) -> Result<(), String> {
    let capability_id = policy_device_capability_id(device_id, object_kind)?;
    push_policy_capability(
        &mut plan.policy_capabilities,
        PolicyCapability {
            id: capability_id.clone(),
            provider: service.to_owned(),
            object_kind,
            object_index,
            rights: RIGHT_CONTROL,
        },
    )?;
    push_policy_requirement(
        &mut plan.policy_requirements,
        PolicyRequirement {
            service: service.to_owned(),
            capability: capability_id,
            rights: RIGHT_CONTROL,
        },
    )
}

fn policy_device_capability_id(device_id: &str, object_kind: u16) -> Result<String, String> {
    let suffix = match object_kind {
        OBJECT_PCI_DEVICE => "pci",
        OBJECT_VIRTIO_DEVICE => "virtio",
        other => return Err(format!("unsupported policy device object kind {other}")),
    };
    let id = format!("{device_id}#{suffix}");
    if id.len() > STRING_LEN {
        return Err(format!(
            "synthetic policy capability id {id} exceeds compact string length"
        ));
    }
    Ok(id)
}

fn add_vertex_store_verifier_policy(plan: &mut BootPlan) -> Result<(), String> {
    if native_process_for_service(&plan.processes, "svc:vertex-store").is_none() {
        return Ok(());
    }
    for object_id in [
        "store:logd-demo",
        "store:echo-server-demo",
        "store:echo-demo",
    ] {
        if plan
            .store_objects
            .iter()
            .any(|object| object.id == object_id)
        {
            push_policy_requirement(
                &mut plan.policy_requirements,
                PolicyRequirement {
                    service: "svc:vertex-store".to_owned(),
                    capability: object_id.to_owned(),
                    rights: RIGHT_READ,
                },
            )?;
        }
    }
    Ok(())
}

fn push_policy_capability(
    capabilities: &mut Vec<PolicyCapability>,
    capability: PolicyCapability,
) -> Result<(), String> {
    if let Some(existing) = capabilities
        .iter_mut()
        .find(|existing| existing.id == capability.id)
    {
        if existing.provider != capability.provider
            || existing.object_kind != capability.object_kind
            || existing.object_index != capability.object_index
        {
            return Err(format!(
                "policy capability {} has conflicting object/provider facts",
                capability.id
            ));
        }
        existing.rights |= capability.rights;
        return Ok(());
    }
    capabilities.push(capability);
    Ok(())
}

fn push_policy_requirement(
    requirements: &mut Vec<PolicyRequirement>,
    requirement: PolicyRequirement,
) -> Result<(), String> {
    if let Some(existing) = requirements.iter_mut().find(|existing| {
        existing.service == requirement.service && existing.capability == requirement.capability
    }) {
        existing.rights |= requirement.rights;
        return Ok(());
    }
    requirements.push(requirement);
    Ok(())
}

fn push_policy_provide(
    provides: &mut Vec<PolicyProvide>,
    provide: PolicyProvide,
) -> Result<(), String> {
    if provides.iter().any(|existing| {
        existing.service == provide.service && existing.capability == provide.capability
    }) {
        return Ok(());
    }
    provides.push(provide);
    Ok(())
}

fn push_policy_mount(mounts: &mut Vec<PolicyMount>, mount: PolicyMount) -> Result<(), String> {
    if mounts.iter().any(|existing| {
        existing.service == mount.service
            && existing.mount_root == mount.mount_root
            && existing.path == mount.path
            && existing.source == mount.source
            && existing.flags == mount.flags
    }) {
        return Ok(());
    }
    mounts.push(mount);
    Ok(())
}

fn push_policy_state_path(
    state_paths: &mut Vec<PolicyStatePath>,
    state_path: PolicyStatePath,
) -> Result<(), String> {
    if let Some(existing) = state_paths.iter_mut().find(|existing| {
        existing.service == state_path.service
            && existing.state == state_path.state
            && existing.root == state_path.root
    }) {
        existing.rights |= state_path.rights;
        return Ok(());
    }
    state_paths.push(state_path);
    Ok(())
}

fn push_policy_bootstrap(
    bootstraps: &mut Vec<PolicyBootstrap>,
    bootstrap: PolicyBootstrap,
) -> Result<(), String> {
    if let Some(existing) = bootstraps.iter_mut().find(|existing| {
        existing.service == bootstrap.service
            && existing.authority == bootstrap.authority
            && existing.rule == bootstrap.rule
    }) {
        existing.rights |= bootstrap.rights;
        return Ok(());
    }
    bootstraps.push(bootstrap);
    Ok(())
}

fn push_graph_device_node(
    nodes: &mut Vec<GraphNode>,
    object_kind: u16,
    id: &str,
    label: &str,
) -> Result<(), String> {
    push_graph_node(
        nodes,
        GRAPH_NODE_DEVICE,
        object_kind,
        id.to_owned(),
        label.to_owned(),
    )
}

fn push_graph_node(
    nodes: &mut Vec<GraphNode>,
    kind: u16,
    object_kind: u16,
    id: String,
    label: String,
) -> Result<(), String> {
    if id.is_empty() {
        return Err("graph node id must not be empty".to_owned());
    }
    if id.len() > STRING_LEN || label.len() > STRING_LEN {
        return Err(format!("graph node {id} exceeds compact string length"));
    }
    if !id.is_ascii() || !label.is_ascii() {
        return Err(format!("graph node {id} must be ASCII"));
    }
    if nodes.iter().any(|node| node.id == id) {
        return Ok(());
    }
    nodes.push(GraphNode {
        kind,
        object_kind,
        id,
        label,
    });
    Ok(())
}

fn linked_package_ids(manifest: &GenerationManifest) -> Result<Vec<&str>, String> {
    let Some(value) = manifest.generation.extra.get("linkedPackages") else {
        return Ok(Vec::new());
    };
    let Some(items) = value.as_array() else {
        return Err("generation.linkedPackages must be an array".to_owned());
    };
    let mut packages = Vec::new();
    for item in items {
        let Some(package) = item.as_str() else {
            return Err("generation.linkedPackages entries must be strings".to_owned());
        };
        if package.is_empty() || package.len() > STRING_LEN || !package.is_ascii() {
            return Err(format!(
                "linked package {package} is not a compact ASCII id"
            ));
        }
        packages.push(package);
    }
    packages.sort_unstable();
    packages.dedup();
    Ok(packages)
}

fn push_graph_edge(
    nodes: &[GraphNode],
    edges: &mut Vec<GraphEdge>,
    kind: u16,
    from: &str,
    to: &str,
    rights: u16,
    id: String,
) -> Result<(), String> {
    if id.is_empty() || id.len() > STRING_LEN || !id.is_ascii() {
        return Err(format!("invalid graph edge id {id}"));
    }
    graph_node_index(nodes, from)?;
    graph_node_index(nodes, to)?;
    edges.push(GraphEdge {
        kind,
        from: from.to_owned(),
        to: to.to_owned(),
        rights,
        id,
    });
    Ok(())
}

fn process_graph_node_id(process: &NativeProcess) -> String {
    if process.service_id.is_empty() {
        format!("proc:{}", process.name)
    } else {
        process.service_id.clone()
    }
}

fn compact_state_field(
    state: &vertex_ir::StateVolume,
    field: &str,
    value: &str,
) -> Result<String, String> {
    if value.is_empty() || value.len() > STRING_LEN || !value.is_ascii() {
        return Err(format!(
            "state volume {} {field} must be non-empty ASCII and fit in the compact record",
            state.id
        ));
    }
    Ok(value.to_owned())
}

fn compact_policy_mode(
    state: &vertex_ir::StateVolume,
    field: &str,
    value: &Value,
) -> Result<String, String> {
    let mode = value
        .as_object()
        .and_then(|object| object.get("mode"))
        .and_then(Value::as_str)
        .ok_or_else(|| format!("state volume {} {field} must declare mode", state.id))?;
    compact_state_field(state, field, mode)
}

fn state_policy_label(state: &StateVolume) -> String {
    let mut label = format!(
        "owner={} schema={} storage={}",
        compact_service_label(&state.owner),
        state.schema_version,
        state.storage_class
    );
    if label.len() > STRING_LEN {
        label.truncate(STRING_LEN);
    }
    label
}

fn compact_service_label(service: &str) -> &str {
    service.strip_prefix("svc:").unwrap_or(service)
}

fn state_volumes_covered_by_compact_root<'a>(
    states: &'a [StateVolume],
    root: &str,
) -> Result<Vec<&'a StateVolume>, String> {
    if root == "/state" {
        return Ok(states.iter().collect());
    }
    let Some(rest) = root.strip_prefix("/state/") else {
        return Ok(Vec::new());
    };
    let component = rest.split('/').next().unwrap_or_default();
    if component.is_empty() {
        return Err(format!("state VFS root {root} has empty state component"));
    }
    Ok(states
        .iter()
        .filter(|state| state_mount_component(&state.id) == component)
        .collect())
}

fn graph_target_id_for_grant(grant: &Grant) -> String {
    match grant.object_kind {
        OBJECT_TIMER => "monotonic-timer".to_owned(),
        _ => grant.object_name.clone(),
    }
}

fn graph_node_index(nodes: &[GraphNode], id: &str) -> Result<usize, String> {
    nodes
        .iter()
        .position(|node| node.id == id)
        .ok_or_else(|| format!("graph edge references unknown node {id}"))
}

fn native_service_closure(
    manifest: &GenerationManifest,
    root_service_id: &str,
) -> Result<Vec<String>, String> {
    let mut service_order = if manifest.activation.start_order.is_empty() {
        manifest
            .services
            .iter()
            .map(|service| service.id.clone())
            .collect::<Vec<_>>()
    } else {
        manifest.activation.start_order.clone()
    };

    if !service_order.iter().any(|id| id == root_service_id) {
        service_order.insert(0, root_service_id.to_owned());
    }

    let mut cursor = 0;
    while cursor < service_order.len() {
        let service_id = service_order[cursor].clone();
        let service = manifest
            .service(&service_id)
            .ok_or_else(|| format!("activation references unknown service {service_id}"))?;

        for dependency in &service.lifecycle.start_after {
            if manifest.service(dependency).is_none() {
                return Err(format!(
                    "service {} depends on unknown service {}",
                    service.id, dependency
                ));
            }
            push_unique_service(&mut service_order, dependency.clone());
        }

        for requirement in &service.requires {
            let Some(capability) = manifest.capability(&requirement.capability) else {
                return Err(format!(
                    "service {} requires unknown capability {}",
                    service.id, requirement.capability
                ));
            };
            if manifest.service(&capability.provider).is_some() {
                push_unique_service(&mut service_order, capability.provider.clone());
            }
        }

        cursor += 1;
    }

    Ok(service_order)
}

fn push_unique_service(services: &mut Vec<String>, service_id: String) {
    if !services.iter().any(|existing| existing == &service_id) {
        services.push(service_id);
    }
}

fn add_vertex_store_verifier_grants(
    grants: &mut Vec<Grant>,
    store_objects: &[StoreObject],
    processes: &[NativeProcess],
) {
    let Some(process) = native_process_for_service(processes, "svc:vertex-store") else {
        return;
    };

    if store_objects
        .iter()
        .any(|object| object.id == "store:logd-demo")
    {
        grants.push(Grant {
            process: process.clone(),
            object_kind: OBJECT_STORE,
            object_name: "store:logd-demo".to_owned(),
            cap_slot: VERTEX_STORE_LOGD_OBJECT_CAP_SLOT,
            rights: RIGHT_READ,
        });
    }

    let echo_object = ["store:echo-server-demo", "store:echo-demo"]
        .iter()
        .find(|id| store_objects.iter().any(|object| object.id == **id));
    if let Some(object_name) = echo_object {
        grants.push(Grant {
            process,
            object_kind: OBJECT_STORE,
            object_name: (*object_name).to_owned(),
            cap_slot: VERTEX_STORE_ECHO_OBJECT_CAP_SLOT,
            rights: RIGHT_READ,
        });
    }
}

fn reserve_vertex_store_verifier_slots(
    slots: &mut BTreeMap<String, u16>,
    processes: &[NativeProcess],
) {
    let Some(process) = native_process_for_service(processes, "svc:vertex-store") else {
        return;
    };
    let Some(slot) = slots.get_mut(&process) else {
        return;
    };
    let next_after_verifier = VERTEX_STORE_ECHO_OBJECT_CAP_SLOT + 1;
    if *slot < next_after_verifier {
        *slot = next_after_verifier;
    }
}

fn push_executable_store_object(
    objects: &mut Vec<StoreObject>,
    manifest: &GenerationManifest,
    store_id: &str,
    module_string: &str,
) -> Result<(), String> {
    if objects.iter().any(|object| object.id == store_id) {
        return Ok(());
    }
    let store = manifest
        .store_object(store_id)
        .ok_or_else(|| format!("executable references unknown store object {store_id}"))?;
    let bytes = native_store_bytes(module_string)?;
    objects.push(StoreObject {
        id: store.id.clone(),
        module_string: module_string.to_owned(),
        hash: store_hash_hex(&bytes),
        size: bytes.len() as u64,
    });
    Ok(())
}

fn push_unique_store_object(
    objects: &mut Vec<StoreObject>,
    store: &vertex_ir::StoreObject,
) -> Result<(), String> {
    if objects.iter().any(|object| object.id == store.id) {
        return Ok(());
    }

    let bytes = native_store_bytes(&store.name)?;

    objects.push(StoreObject {
        id: store.id.clone(),
        module_string: store.name.clone(),
        hash: store_hash_hex(&bytes),
        size: bytes.len() as u64,
    });
    Ok(())
}

fn native_store_bytes(module_string: &str) -> Result<Vec<u8>, String> {
    if module_string == LOGD_CONFIG_MODULE {
        return Ok(LOGD_CONFIG_BYTES.to_vec());
    }

    let mut candidates = Vec::new();
    for path in native_store_candidate_paths(module_string) {
        candidates.push(path.display().to_string());
        if let Ok(bytes) = fs::read(&path) {
            return Ok(bytes);
        }
    }
    Err(format!(
        "native store artifact {module_string} missing; checked {}",
        candidates.join(", ")
    ))
}

fn native_store_candidate_paths(module_string: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if module_string == "store-hello-text" {
        paths.push(PathBuf::from("assets/hello-text.txt"));
        paths.push(PathBuf::from("kernel/krust/assets/hello-text.txt"));
        return paths;
    }
    if module_string == "store-block-driver-fault-token" {
        paths.push(PathBuf::from("assets/block-driver-fault-token.txt"));
        paths.push(PathBuf::from(
            "kernel/krust/assets/block-driver-fault-token.txt",
        ));
        return paths;
    }
    if module_string == "store-vertexfs-fsync-fault-token" {
        paths.push(PathBuf::from("assets/vertexfs-fsync-fault-token.txt"));
        paths.push(PathBuf::from(
            "kernel/krust/assets/vertexfs-fsync-fault-token.txt",
        ));
        return paths;
    }
    if module_string == "package-fragment-logd" {
        paths.push(PathBuf::from("assets/package-fragment-logd.txt"));
        paths.push(PathBuf::from(
            "kernel/krust/assets/package-fragment-logd.txt",
        ));
        return paths;
    }
    if module_string == "package-fragment-missing-dependency" {
        paths.push(PathBuf::from(
            "assets/package-fragment-missing-dependency.txt",
        ));
        paths.push(PathBuf::from(
            "kernel/krust/assets/package-fragment-missing-dependency.txt",
        ));
        return paths;
    }
    if module_string == "package-fragment-excess-authority" {
        paths.push(PathBuf::from(
            "assets/package-fragment-excess-authority.txt",
        ));
        paths.push(PathBuf::from(
            "kernel/krust/assets/package-fragment-excess-authority.txt",
        ));
        return paths;
    }

    let crate_dir = match module_string {
        "vertex-init" => "init",
        other => other,
    };
    paths.push(PathBuf::from(format!(
        "targets/krust/user/target/x86_64-unknown-none/debug/{module_string}"
    )));
    paths.push(PathBuf::from(format!(
        "../../targets/krust/user/target/x86_64-unknown-none/debug/{module_string}"
    )));
    paths.push(PathBuf::from(format!(
        "targets/krust/user/{crate_dir}/target/x86_64-unknown-none/debug/{module_string}"
    )));
    paths.push(PathBuf::from(format!(
        "../../targets/krust/user/{crate_dir}/target/x86_64-unknown-none/debug/{module_string}"
    )));
    paths
}

fn store_hash_hex(bytes: &[u8]) -> String {
    let digest = blake3::hash(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest.as_bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn push_unique_network_port(ports: &mut Vec<NetworkPort>, port_id: String) {
    if !ports.iter().any(|port| port.id == port_id) {
        ports.push(NetworkPort { id: port_id });
    }
}

fn push_unique_namespace(
    namespaces: &mut Vec<Namespace>,
    manifest: &GenerationManifest,
    capability: &vertex_ir::Capability,
) -> Result<(), String> {
    if namespaces
        .iter()
        .any(|namespace| namespace.id == capability.id)
    {
        return Ok(());
    }

    let entries = capability
        .properties
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("namespace capability {} missing entries", capability.id))?;
    if entries.is_empty() {
        return Err(format!(
            "namespace capability {} has no entries",
            capability.id
        ));
    }
    if entries.len() > MAX_NAMESPACE_ENTRIES {
        return Err(format!(
            "namespace capability {} exceeds {MAX_NAMESPACE_ENTRIES} entries",
            capability.id
        ));
    }

    let mut parsed = Vec::new();
    let mut paths = BTreeSet::new();
    for entry in entries {
        let path = value_str(entry, "path")
            .ok_or_else(|| format!("namespace capability {} entry missing path", capability.id))?;
        if !path.starts_with('/') {
            return Err(format!(
                "namespace capability {} entry path {path} must be absolute",
                capability.id
            ));
        }
        if !paths.insert(path.to_owned()) {
            return Err(format!(
                "namespace capability {} duplicates path {path}",
                capability.id
            ));
        }
        let capability_id = value_str(entry, "capability").ok_or_else(|| {
            format!(
                "namespace capability {} entry {path} missing capability",
                capability.id
            )
        })?;
        let target = manifest.capability(capability_id).ok_or_else(|| {
            format!(
                "namespace capability {} entry {path} references unknown capability {capability_id}",
                capability.id
            )
        })?;
        let rights = entry
            .get("rights")
            .and_then(Value::as_array)
            .map(|rights| {
                rights
                    .iter()
                    .map(|right| {
                        right.as_str().map(str::to_owned).ok_or_else(|| {
                            format!(
                                "namespace capability {} entry {path} has non-string right",
                                capability.id
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_else(|| target.rights.clone());
        let object_kind = object_kind_for_namespace_entry(target)?;
        parsed.push(NamespaceEntry {
            path: path.to_owned(),
            object_kind,
            object_name: object_name_for_capability(target),
            rights: rights_mask(&rights, &target.rights, capability_id)?,
        });
    }

    namespaces.push(Namespace {
        id: capability.id.clone(),
        entries: parsed,
    });
    Ok(())
}

fn object_kind_for_namespace_entry(capability: &vertex_ir::Capability) -> Result<u16, String> {
    match capability.kind.as_str() {
        "ipc-endpoint" => Ok(OBJECT_ENDPOINT),
        "store-object" => Ok(OBJECT_STORE),
        "timer" => Ok(OBJECT_TIMER),
        "network-port" => Ok(OBJECT_NETWORK_PORT),
        "io-port" | "mmio-region" | "framebuffer" | "interrupt-line" | "dma-region"
        | "pci-device" | "virtio-device" => Err(format!(
            "namespace entries cannot resolve hardware capability kind {}",
            capability.kind
        )),
        "namespace" => Err("namespace entries cannot resolve namespace capabilities".to_owned()),
        other => Err(format!(
            "namespace entries cannot resolve capability kind {other}"
        )),
    }
}

fn push_unique_vfs_root(
    roots: &mut Vec<VfsRoot>,
    capability: &vertex_ir::Capability,
) -> Result<(), String> {
    if roots.iter().any(|root| root.id == capability.id) {
        return Ok(());
    }
    let root_path = capability
        .properties
        .get("root")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("vfs-root capability {} missing root", capability.id))?;
    validate_manifest_vfs_path(&capability.id, root_path)?;
    roots.push(VfsRoot {
        id: capability.id.clone(),
        root_path: root_path.to_owned(),
    });
    Ok(())
}

fn validate_state_vfs_grant_policy(
    manifest: &GenerationManifest,
    service: &Service,
    capability: &vertex_ir::Capability,
    rights: u16,
) -> Result<(), String> {
    let Some(root) = capability.properties.get("root").and_then(Value::as_str) else {
        return Ok(());
    };
    let covered = state_volumes_covered_by_root(manifest, root)?;
    for state in covered {
        if state.owner == service.id {
            continue;
        }
        if !state_sharing_allows_vfs_rights(state, &service.id, rights, root) {
            return Err(format!(
                "service {} receives VFS root {} over state volume {} owned by {} without matching sharingPolicy",
                service.id, capability.id, state.id, state.owner
            ));
        }
    }
    Ok(())
}

fn state_volumes_covered_by_root<'a>(
    manifest: &'a GenerationManifest,
    root: &str,
) -> Result<Vec<&'a vertex_ir::StateVolume>, String> {
    if root == "/state" {
        return Ok(manifest.state_volumes.iter().collect());
    }
    let Some(rest) = root.strip_prefix("/state/") else {
        return Ok(Vec::new());
    };
    let component = rest.split('/').next().unwrap_or_default();
    if component.is_empty() {
        return Err(format!("state VFS root {root} has empty state component"));
    }
    Ok(manifest
        .state_volumes
        .iter()
        .filter(|state| state_mount_component(&state.id) == component)
        .collect())
}

fn state_mount_component(state_id: &str) -> &str {
    state_id.strip_prefix("state:").unwrap_or(state_id)
}

fn state_sharing_allows_vfs_rights(
    state: &vertex_ir::StateVolume,
    service_id: &str,
    rights: u16,
    root: &str,
) -> bool {
    let Some(policy) = state.sharing_policy.as_object() else {
        return false;
    };
    if policy.get("mode").and_then(Value::as_str) != Some("explicit") {
        return false;
    }
    let needs_control = rights & RIGHT_CONTROL != 0 || root.ends_with("/control");
    let needs_write =
        rights & (RIGHT_WRITE | RIGHT_CREATE | RIGHT_UNLINK | RIGHT_RENAME | RIGHT_MOUNT) != 0;
    let field = if needs_control {
        "controllers"
    } else if needs_write {
        "writers"
    } else {
        "readers"
    };
    policy
        .get(field)
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .any(|entry| entry.as_str() == Some(service_id))
        })
        .unwrap_or(false)
}

fn validate_manifest_vfs_path(context: &str, path: &str) -> Result<(), String> {
    if !path.starts_with('/') || path.len() > STRING_LEN - 1 || (path.ends_with('/') && path != "/")
    {
        return Err(format!(
            "vfs-root capability {context} root {path} must be an absolute non-trailing-slash path"
        ));
    }
    if path.as_bytes().windows(2).any(|window| window == b"//") {
        return Err(format!(
            "vfs-root capability {context} root {path} must not contain empty components"
        ));
    }
    Ok(())
}

fn object_name_for_capability(capability: &vertex_ir::Capability) -> String {
    match capability.kind.as_str() {
        "timer" => "monotonic-timer".to_owned(),
        "store-object" | "io-port" | "mmio-region" | "framebuffer" | "interrupt-line"
        | "dma-region" => capability.id.clone(),
        "ipc-endpoint" => endpoint_name(&capability.id),
        "network-port" | "virtio-device" | "namespace" | "vfs-root" => capability.id.clone(),
        _ => capability.id.clone(),
    }
}

fn push_unique_io_port(
    ports: &mut Vec<IoPortRange>,
    manifest: &GenerationManifest,
    capability: &vertex_ir::Capability,
) -> Result<(), String> {
    if ports.iter().any(|port| port.id == capability.id) {
        return Ok(());
    }
    let device = manifest
        .devices
        .iter()
        .find(|device| device.id == capability.provider)
        .ok_or_else(|| {
            format!(
                "capability {} references unknown I/O device {}",
                capability.id, capability.provider
            )
        })?;
    let base = value_u64(&capability.properties, "base")
        .or_else(|| value_u64(&capability.properties, "portBase"))
        .or_else(|| value_u64(&capability.properties, "port"))
        .or_else(|| value_u64(&device.selector, "base"))
        .or_else(|| value_u64(&device.selector, "portBase"))
        .or_else(|| value_u64(&device.selector, "port"))
        .ok_or_else(|| format!("io-port capability {} missing base port", capability.id))?;
    let length = value_u64(&capability.properties, "length")
        .or_else(|| value_u64(&capability.properties, "ports"))
        .or_else(|| value_u64(&device.selector, "length"))
        .or_else(|| value_u64(&device.selector, "ports"))
        .unwrap_or(1);
    validate_io_port_range(&capability.id, base, length)?;
    ports.push(IoPortRange {
        id: capability.id.clone(),
        base,
        length,
    });
    Ok(())
}

fn validate_io_port_range(id: &str, base: u64, length: u64) -> Result<(), String> {
    if length == 0 {
        return Err(format!("io-port capability {id} length must be nonzero"));
    }
    let last = base
        .checked_add(length - 1)
        .ok_or_else(|| format!("io-port capability {id} range overflows"))?;
    if last > u16::MAX as u64 {
        return Err(format!(
            "io-port capability {id} range exceeds x86 I/O port space"
        ));
    }
    Ok(())
}

fn push_unique_mmio_region(
    regions: &mut Vec<MmioRegion>,
    manifest: &GenerationManifest,
    capability: &vertex_ir::Capability,
) -> Result<(), String> {
    if regions.iter().any(|region| region.id == capability.id) {
        return Ok(());
    }
    let device = manifest
        .devices
        .iter()
        .find(|device| device.id == capability.provider)
        .ok_or_else(|| {
            format!(
                "capability {} references unknown MMIO device {}",
                capability.id, capability.provider
            )
        })?;
    regions.push(MmioRegion {
        id: capability.id.clone(),
        base: value_u64(&capability.properties, "base")
            .or_else(|| value_u64(&device.selector, "base"))
            .ok_or_else(|| format!("mmio-region capability {} missing base", capability.id))?,
        length: value_u64(&capability.properties, "length")
            .or_else(|| value_u64(&device.selector, "length"))
            .unwrap_or(4096),
    });
    Ok(())
}

fn push_unique_framebuffer(
    framebuffers: &mut Vec<Framebuffer>,
    manifest: &GenerationManifest,
    capability: &vertex_ir::Capability,
) -> Result<(), String> {
    if framebuffers
        .iter()
        .any(|framebuffer| framebuffer.id == capability.id)
    {
        return Ok(());
    }
    let device = manifest
        .devices
        .iter()
        .find(|device| device.id == capability.provider)
        .ok_or_else(|| {
            format!(
                "capability {} references unknown framebuffer device {}",
                capability.id, capability.provider
            )
        })?;
    if device.kind != "framebuffer" {
        return Err(format!(
            "framebuffer capability {} provider {} has device kind {}; expected framebuffer",
            capability.id, device.id, device.kind
        ));
    }
    if value_u64(&capability.properties, "base").is_some()
        || value_u64(&capability.properties, "length").is_some()
        || value_u64(&device.selector, "base").is_some()
        || value_u64(&device.selector, "length").is_some()
    {
        return Err(format!(
            "framebuffer capability {} must not declare base/length; Krust maps the Limine boot framebuffer",
            capability.id
        ));
    }
    framebuffers.push(Framebuffer {
        id: capability.id.clone(),
    });
    Ok(())
}

fn push_unique_interrupt_line(
    lines: &mut Vec<InterruptLine>,
    manifest: &GenerationManifest,
    capability: &vertex_ir::Capability,
) -> Result<(), String> {
    if lines.iter().any(|line| line.id == capability.id) {
        return Ok(());
    }
    let device = manifest
        .devices
        .iter()
        .find(|device| device.id == capability.provider)
        .ok_or_else(|| {
            format!(
                "capability {} references unknown interrupt device {}",
                capability.id, capability.provider
            )
        })?;
    lines.push(InterruptLine {
        id: capability.id.clone(),
        line: value_u64(&capability.properties, "line")
            .or_else(|| value_u64(&capability.properties, "irq"))
            .or_else(|| value_u64(&device.selector, "line"))
            .or_else(|| value_u64(&device.selector, "irq"))
            .ok_or_else(|| format!("interrupt-line capability {} missing line", capability.id))?,
    });
    Ok(())
}

fn push_unique_dma_region(
    regions: &mut Vec<DmaRegion>,
    manifest: &GenerationManifest,
    capability: &vertex_ir::Capability,
) -> Result<(), String> {
    if regions.iter().any(|region| region.id == capability.id) {
        return Ok(());
    }
    let device = manifest
        .devices
        .iter()
        .find(|device| device.id == capability.provider)
        .ok_or_else(|| {
            format!(
                "capability {} references unknown DMA device {}",
                capability.id, capability.provider
            )
        })?;
    let kernel_allocated = value_str(&capability.properties, "allocation") == Some("kernel-dma");
    let base = if kernel_allocated {
        DMA_KERNEL_ALLOCATED_BASE
    } else {
        let base = value_u64(&capability.properties, "base")
            .or_else(|| value_u64(&device.selector, "base"))
            .ok_or_else(|| {
                format!(
                    "dma-region capability {} must set allocation=kernel-dma or provide base",
                    capability.id
                )
            })?;
        if base == 0 {
            return Err(format!(
                "dma-region capability {} must use allocation=kernel-dma instead of base=0",
                capability.id
            ));
        }
        if base == DMA_KERNEL_ALLOCATED_BASE {
            return Err(format!(
                "dma-region capability {} base is reserved; use allocation=kernel-dma",
                capability.id
            ));
        }
        base
    };
    regions.push(DmaRegion {
        id: capability.id.clone(),
        base,
        length: value_u64(&capability.properties, "length")
            .or_else(|| value_u64(&device.selector, "length"))
            .unwrap_or(4096),
    });
    Ok(())
}

fn grant_native_driver_devices(
    manifest: &GenerationManifest,
    processes: &[NativeProcess],
    grants: &mut Vec<Grant>,
    pci_devices: &mut Vec<PciDevice>,
    virtio_devices: &mut Vec<VirtioDevice>,
    next_object_slots: &mut BTreeMap<String, u16>,
    root_service_id: &str,
) -> Result<(), String> {
    for device in &manifest.devices {
        if device.driver == root_service_id {
            return Err(format!(
                "device {} cannot be driven by activation root service {}",
                device.id, root_service_id
            ));
        }
        let Some(process_name) = native_process_for_service(processes, &device.driver) else {
            continue;
        };

        if is_pci_device(device) {
            push_unique_pci_device(pci_devices, device);
            grants.push(Grant {
                process: process_name.clone(),
                object_kind: OBJECT_PCI_DEVICE,
                object_name: device.id.clone(),
                cap_slot: next_object_cap_slot(next_object_slots, &process_name)?,
                rights: RIGHT_CONTROL,
            });
        }

        if is_virtio_device(device) {
            push_unique_virtio_device(virtio_devices, device);
            grants.push(Grant {
                process: process_name.clone(),
                object_kind: OBJECT_VIRTIO_DEVICE,
                object_name: device.id.clone(),
                cap_slot: next_object_cap_slot(next_object_slots, &process_name)?,
                rights: RIGHT_CONTROL,
            });
        }
    }

    Ok(())
}

fn grant_network_port_provider_caps(
    manifest: &GenerationManifest,
    processes: &[NativeProcess],
    network_ports: &[NetworkPort],
    grants: &mut Vec<Grant>,
    next_object_slots: &mut BTreeMap<String, u16>,
    root_service_id: &str,
) -> Result<(), String> {
    for port in network_ports {
        let capability = manifest
            .capability(&port.id)
            .ok_or_else(|| format!("network-port {} has no manifest capability", port.id))?;
        if capability.kind != "network-port" || capability.provider == root_service_id {
            continue;
        }
        let Some(process_name) = native_process_for_service(processes, &capability.provider) else {
            continue;
        };
        grants.push(Grant {
            process: process_name.clone(),
            object_kind: OBJECT_NETWORK_PORT,
            object_name: port.id.clone(),
            cap_slot: next_object_cap_slot(next_object_slots, &process_name)?,
            rights: RIGHT_CONTROL,
        });
    }
    Ok(())
}

fn push_unique_pci_device(devices: &mut Vec<PciDevice>, device: &vertex_ir::Device) {
    if devices.iter().any(|existing| existing.id == device.id) {
        return;
    }
    devices.push(PciDevice {
        id: device.id.clone(),
        kind: device.kind.clone(),
    });
}

fn push_unique_virtio_device(devices: &mut Vec<VirtioDevice>, device: &vertex_ir::Device) {
    if devices.iter().any(|existing| existing.id == device.id) {
        return;
    }
    devices.push(VirtioDevice {
        id: device.id.clone(),
        transport: value_str(&device.properties, "transport")
            .unwrap_or("virtio-pci-io")
            .to_owned(),
    });
}

fn is_pci_device(device: &vertex_ir::Device) -> bool {
    device.kind.contains("pci")
        || value_str(&device.properties, "transport")
            .map(|transport| transport.contains("pci"))
            .unwrap_or(false)
}

fn is_virtio_device(device: &vertex_ir::Device) -> bool {
    device.kind.starts_with("virtio")
        || value_str(&device.properties, "transport")
            .map(|transport| transport.starts_with("virtio"))
            .unwrap_or(false)
}

fn value_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(parse_u64_literal))
    })
}

fn value_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn value_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn requirement_starts_after_provider(capability: &vertex_ir::Capability) -> bool {
    if capability.kind != "ipc-endpoint" {
        return true;
    }
    if value_bool(&capability.properties, "startAfterProvider") == Some(false) {
        return false;
    }
    value_str(&capability.properties, "role") != Some("reply")
}

fn parse_u64_literal(value: &str) -> Option<u64> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok()
    } else {
        value.parse::<u64>().ok()
    }
}

fn initial_object_cap_slots(processes: &[NativeProcess]) -> BTreeMap<String, u16> {
    let mut slots = BTreeMap::new();
    for process in processes {
        let mut next_slot =
            if process.requires_endpoints.is_empty() && process.provides_endpoints.is_empty() {
                SERVICE_CAP_SLOT
            } else {
                first_non_endpoint_service_slot(
                    process.provides_endpoints.len(),
                    process.requires_endpoints.len(),
                )
            };
        if next_slot == SERIAL_RESERVED_CAP_SLOT || next_slot == READINESS_RESERVED_CAP_SLOT {
            next_slot = READINESS_RESERVED_CAP_SLOT + 1;
        }
        slots.insert(process.name.clone(), next_slot);
    }
    slots
}

fn first_non_endpoint_service_slot(provided_count: usize, endpoint_count: usize) -> u16 {
    let mut slot = SERVICE_CAP_SLOT;
    if provided_count > 0 {
        slot = provided_endpoint_target_slot(provided_count - 1) + 1;
    }
    if endpoint_count > 0 {
        let after_required = endpoint_target_slot(provided_count, endpoint_count - 1) + 1;
        if after_required > slot {
            slot = after_required;
        }
    }
    if slot <= READINESS_RESERVED_CAP_SLOT {
        slot = READINESS_RESERVED_CAP_SLOT + 1;
    }
    slot
}

fn next_object_cap_slot(slots: &mut BTreeMap<String, u16>, process: &str) -> Result<u16, String> {
    let slot = slots
        .get_mut(process)
        .ok_or_else(|| format!("unknown process {process} for cap slot allocation"))?;
    while *slot == SERIAL_RESERVED_CAP_SLOT || *slot == READINESS_RESERVED_CAP_SLOT {
        *slot = slot
            .checked_add(1)
            .ok_or_else(|| format!("cap slot overflow for process {process}"))?;
    }
    let out = *slot;
    *slot = slot
        .checked_add(1)
        .ok_or_else(|| format!("cap slot overflow for process {process}"))?;
    Ok(out)
}

fn validate_plan(plan: &BootPlan) -> Result<(), String> {
    if plan.boot_modules.is_empty() {
        return Err("native boot plan must contain boot modules".to_owned());
    }
    if plan.processes.is_empty() {
        return Err("native boot plan must contain processes".to_owned());
    }
    if plan.endpoints.is_empty() {
        return Err("native boot plan must contain endpoints".to_owned());
    }
    if plan.boot_modules.len() > MAX_BOOT_MODULES {
        return Err(format!(
            "native boot plan exceeds {MAX_BOOT_MODULES} boot modules"
        ));
    }
    if plan.processes.len() > MAX_PROCESSES {
        return Err(format!(
            "native boot plan exceeds {MAX_PROCESSES} processes"
        ));
    }
    if plan.endpoints.len() > MAX_ENDPOINTS {
        return Err(format!(
            "native boot plan exceeds {MAX_ENDPOINTS} endpoints"
        ));
    }
    if plan.grants.len() > MAX_GRANTS {
        return Err(format!("native boot plan exceeds {MAX_GRANTS} grants"));
    }
    if plan.store_objects.len() > MAX_STORE_OBJECTS {
        return Err(format!(
            "native boot plan exceeds {MAX_STORE_OBJECTS} store objects"
        ));
    }
    if plan.state_volumes.len() > MAX_STATE_VOLUMES {
        return Err(format!(
            "native boot plan exceeds {MAX_STATE_VOLUMES} state volumes"
        ));
    }
    if plan.network_ports.len() > MAX_NETWORK_PORTS {
        return Err(format!(
            "native boot plan exceeds {MAX_NETWORK_PORTS} network ports"
        ));
    }
    if plan.io_ports.len() > MAX_IO_PORT_RANGES {
        return Err(format!(
            "native boot plan exceeds {MAX_IO_PORT_RANGES} io port ranges"
        ));
    }
    if plan.mmio_regions.len() > MAX_MMIO_REGIONS {
        return Err(format!(
            "native boot plan exceeds {MAX_MMIO_REGIONS} mmio regions"
        ));
    }
    if plan.framebuffers.len() > MAX_FRAMEBUFFERS {
        return Err(format!(
            "native boot plan exceeds {MAX_FRAMEBUFFERS} framebuffers"
        ));
    }
    if plan.interrupt_lines.len() > MAX_INTERRUPT_LINES {
        return Err(format!(
            "native boot plan exceeds {MAX_INTERRUPT_LINES} interrupt lines"
        ));
    }
    if plan.dma_regions.len() > MAX_DMA_REGIONS {
        return Err(format!(
            "native boot plan exceeds {MAX_DMA_REGIONS} dma regions"
        ));
    }
    if plan.pci_devices.len() > MAX_PCI_DEVICES {
        return Err(format!(
            "native boot plan exceeds {MAX_PCI_DEVICES} pci devices"
        ));
    }
    if plan.virtio_devices.len() > MAX_VIRTIO_DEVICES {
        return Err(format!(
            "native boot plan exceeds {MAX_VIRTIO_DEVICES} virtio devices"
        ));
    }
    if plan.namespaces.len() > MAX_NAMESPACES {
        return Err(format!(
            "native boot plan exceeds {MAX_NAMESPACES} namespaces"
        ));
    }
    if plan.vfs_roots.len() > MAX_VFS_ROOTS {
        return Err(format!(
            "native boot plan exceeds {MAX_VFS_ROOTS} vfs roots"
        ));
    }
    if plan.graph_nodes.len() > MAX_GRAPH_NODES {
        return Err(format!(
            "native graph store exceeds {MAX_GRAPH_NODES} nodes"
        ));
    }
    if plan.graph_edges.len() > MAX_GRAPH_EDGES {
        return Err(format!(
            "native graph store exceeds {MAX_GRAPH_EDGES} edges"
        ));
    }
    if plan.policy_capabilities.len() > MAX_POLICY_CAPABILITIES {
        return Err(format!(
            "native policy exceeds {MAX_POLICY_CAPABILITIES} capability facts"
        ));
    }
    if plan.policy_requirements.len() > MAX_POLICY_REQUIREMENTS {
        return Err(format!(
            "native policy exceeds {MAX_POLICY_REQUIREMENTS} requirement facts"
        ));
    }
    if plan.policy_provides.len() > MAX_POLICY_PROVIDES {
        return Err(format!(
            "native policy exceeds {MAX_POLICY_PROVIDES} provide facts"
        ));
    }
    if plan.policy_mounts.len() > MAX_POLICY_MOUNTS {
        return Err(format!(
            "native policy exceeds {MAX_POLICY_MOUNTS} mount facts"
        ));
    }
    if plan.policy_state_paths.len() > MAX_POLICY_STATE_PATHS {
        return Err(format!(
            "native policy exceeds {MAX_POLICY_STATE_PATHS} state path facts"
        ));
    }
    if plan.policy_bootstraps.len() > MAX_POLICY_BOOTSTRAPS {
        return Err(format!(
            "native policy exceeds {MAX_POLICY_BOOTSTRAPS} bootstrap facts"
        ));
    }
    validate_hardware_authority(plan)?;

    let initial_count = plan
        .processes
        .iter()
        .filter(|process| process.initial)
        .count();
    if initial_count != 1 {
        return Err(format!(
            "native boot plan must contain exactly one initial process, found {initial_count}"
        ));
    }

    let mut module_strings = BTreeSet::new();
    for module in &plan.boot_modules {
        if !module_strings.insert(module.module_string.as_str()) {
            return Err(format!(
                "duplicate boot module string {}",
                module.module_string
            ));
        }
    }

    let mut process_names = BTreeSet::new();
    for process in &plan.processes {
        if !process_names.insert(process.name.as_str()) {
            return Err(format!("duplicate process {}", process.name));
        }
        if plan
            .boot_modules
            .iter()
            .all(|module| module.module_string != process.module_string)
        {
            return Err(format!(
                "process {} references unknown module string {}",
                process.name, process.module_string
            ));
        }
        validate_manifest_vfs_path(&process.name, &process.mount_root)?;
        if process.mounts.len() > MAX_PROCESS_MOUNTS {
            return Err(format!(
                "process {} has too many declared mounts; max {MAX_PROCESS_MOUNTS}",
                process.name
            ));
        }
        let mut mount_paths = BTreeSet::new();
        for mount in &process.mounts {
            validate_manifest_vfs_path(&process.name, &mount.path)?;
            validate_manifest_vfs_path(&process.name, &mount.source)?;
            if mount.path == "/" {
                return Err(format!(
                    "process {} declared mount path cannot replace namespace root",
                    process.name
                ));
            }
            if mount.flags & !known_process_mount_flags() != 0
                || mount.flags & PROCESS_MOUNT_FLAG_BIND == 0
            {
                return Err(format!(
                    "process {} declared mount has unsupported flags",
                    process.name
                ));
            }
            if !mount_paths.insert(mount.path.as_str()) {
                return Err(format!(
                    "process {} declares duplicate mount path {}",
                    process.name, mount.path
                ));
            }
        }
        if process.start_after.len() > MAX_PROCESS_REFS
            || process.requires_endpoints.len() > MAX_PROCESS_REFS
            || process.provides_endpoints.len() > MAX_PROCESS_REFS
        {
            return Err(format!(
                "process {} has too many compact references",
                process.name
            ));
        }
        for dependency in &process.start_after {
            process_index(plan, dependency)?;
        }
        for requirement in &process.requires_endpoints {
            endpoint_index(plan, &requirement.endpoint)?;
            if requirement.rights != RIGHT_SEND {
                return Err(format!(
                    "process {} has invalid endpoint requirement rights; native endpoint requirements are send-only",
                    process.name
                ));
            }
        }
        for endpoint in &process.provides_endpoints {
            endpoint_index(plan, endpoint)?;
        }
    }

    let mut endpoint_names = BTreeSet::new();
    for endpoint in &plan.endpoints {
        if !endpoint_names.insert(endpoint.name.as_str()) {
            return Err(format!("duplicate endpoint {}", endpoint.name));
        }
    }

    let mut state_volume_names = BTreeSet::new();
    for state in &plan.state_volumes {
        let mount_name = state_volume_mount_component(&state.id)?;
        if !state_volume_names.insert(mount_name) {
            return Err(format!(
                "duplicate state volume mount component {mount_name} from {}",
                state.id
            ));
        }
    }

    let mut pci_device_names = BTreeSet::new();
    for device in &plan.pci_devices {
        if !pci_device_names.insert(device.id.as_str()) {
            return Err(format!("duplicate pci device {}", device.id));
        }
    }

    let mut virtio_device_names = BTreeSet::new();
    for device in &plan.virtio_devices {
        if !virtio_device_names.insert(device.id.as_str()) {
            return Err(format!("duplicate virtio device {}", device.id));
        }
    }

    let mut namespace_names = BTreeSet::new();
    for namespace in &plan.namespaces {
        if !namespace_names.insert(namespace.id.as_str()) {
            return Err(format!("duplicate namespace {}", namespace.id));
        }
        if namespace.entries.is_empty() || namespace.entries.len() > MAX_NAMESPACE_ENTRIES {
            return Err(format!(
                "namespace {} must contain 1..={MAX_NAMESPACE_ENTRIES} entries",
                namespace.id
            ));
        }
        let mut paths = BTreeSet::new();
        for entry in &namespace.entries {
            if !paths.insert(entry.path.as_str()) {
                return Err(format!(
                    "namespace {} duplicates path {}",
                    namespace.id, entry.path
                ));
            }
            object_index_for_kind(plan, entry.object_kind, &entry.object_name)?;
            if entry.object_kind == OBJECT_NAMESPACE || entry.rights == 0 {
                return Err(format!(
                    "namespace {} entry {} has invalid target",
                    namespace.id, entry.path
                ));
            }
        }
    }

    let mut vfs_root_names = BTreeSet::new();
    for root in &plan.vfs_roots {
        if !vfs_root_names.insert(root.id.as_str()) {
            return Err(format!("duplicate vfs root {}", root.id));
        }
        validate_manifest_vfs_path(&root.id, &root.root_path)?;
    }

    for grant in &plan.grants {
        process_index(plan, &grant.process)?;
        object_index(plan, grant)?;
        if grant.object_kind == OBJECT_ENDPOINT {
            if grant.rights != RIGHT_SEND && grant.rights != RIGHT_RECEIVE {
                return Err(format!(
                    "endpoint grant to {} must be one-way send or receive",
                    grant.process
                ));
            }
        } else if grant.rights == 0 {
            return Err(format!("grant to {} has no rights", grant.process));
        }
    }

    let mut graph_node_ids = BTreeSet::new();
    for node in &plan.graph_nodes {
        if node.kind == 0 || node.id.is_empty() {
            return Err("native graph store contains invalid node".to_owned());
        }
        if !graph_node_ids.insert(node.id.as_str()) {
            return Err(format!("duplicate graph node {}", node.id));
        }
    }

    let mut graph_edge_ids = BTreeSet::new();
    for edge in &plan.graph_edges {
        if edge.kind == 0 || edge.id.is_empty() {
            return Err("native graph store contains invalid edge".to_owned());
        }
        if !graph_edge_ids.insert(edge.id.as_str()) {
            return Err(format!("duplicate graph edge {}", edge.id));
        }
        graph_node_index(&plan.graph_nodes, &edge.from)?;
        graph_node_index(&plan.graph_nodes, &edge.to)?;
        if edge.kind == GRAPH_EDGE_CAPABILITY && edge.rights == 0 {
            return Err(format!("capability graph edge {} has no rights", edge.id));
        }
    }

    let mut cap_slots = BTreeSet::new();
    for grant in &plan.grants {
        let key = (grant.process.as_str(), grant.cap_slot);
        if !cap_slots.insert(key) {
            return Err(format!(
                "duplicate grant cap slot {} for process {}",
                grant.cap_slot, grant.process
            ));
        }
    }

    Ok(())
}

fn validate_hardware_authority(plan: &BootPlan) -> Result<(), String> {
    let mut io_ids = BTreeSet::new();
    for (index, range) in plan.io_ports.iter().enumerate() {
        if !io_ids.insert(range.id.as_str()) {
            return Err(format!("duplicate io-port capability {}", range.id));
        }
        validate_io_port_range(&range.id, range.base, range.length)?;
        for previous in &plan.io_ports[..index] {
            if ranges_overlap(range.base, range.length, previous.base, previous.length)? {
                return Err(format!(
                    "io-port capability {} overlaps {}",
                    range.id, previous.id
                ));
            }
        }
    }

    let mut mmio_ids = BTreeSet::new();
    for (index, region) in plan.mmio_regions.iter().enumerate() {
        if !mmio_ids.insert(region.id.as_str()) {
            return Err(format!("duplicate mmio-region capability {}", region.id));
        }
        validate_device_range("mmio-region", &region.id, region.base, region.length, false)?;
        for previous in &plan.mmio_regions[..index] {
            if ranges_overlap(region.base, region.length, previous.base, previous.length)? {
                return Err(format!(
                    "mmio-region capability {} overlaps {}",
                    region.id, previous.id
                ));
            }
        }
    }

    let mut framebuffer_ids = BTreeSet::new();
    for framebuffer in &plan.framebuffers {
        if !framebuffer_ids.insert(framebuffer.id.as_str()) {
            return Err(format!(
                "duplicate framebuffer capability {}",
                framebuffer.id
            ));
        }
    }

    let mut irq_ids = BTreeSet::new();
    let mut irq_lines = BTreeSet::new();
    for line in &plan.interrupt_lines {
        if !irq_ids.insert(line.id.as_str()) {
            return Err(format!("duplicate interrupt-line capability {}", line.id));
        }
        if line.line > MAX_LEGACY_IRQ_LINE {
            return Err(format!(
                "interrupt-line capability {} uses unsupported legacy IRQ {}",
                line.id, line.line
            ));
        }
        if !irq_lines.insert(line.line) {
            return Err(format!(
                "duplicate interrupt-line ownership for legacy IRQ {}",
                line.line
            ));
        }
    }

    let mut dma_ids = BTreeSet::new();
    for (index, region) in plan.dma_regions.iter().enumerate() {
        if !dma_ids.insert(region.id.as_str()) {
            return Err(format!("duplicate dma-region capability {}", region.id));
        }
        if region.length == 0
            || region.length > MAX_DEVICE_MAPPING_LENGTH
            || region.length % PAGE_SIZE != 0
        {
            return Err(format!(
                "dma-region capability {} length must be page-aligned and <= {} bytes",
                region.id, MAX_DEVICE_MAPPING_LENGTH
            ));
        }
        if region.base != DMA_KERNEL_ALLOCATED_BASE {
            validate_device_range("dma-region", &region.id, region.base, region.length, true)?;
            for previous in &plan.dma_regions[..index] {
                if previous.base != DMA_KERNEL_ALLOCATED_BASE
                    && ranges_overlap(region.base, region.length, previous.base, previous.length)?
                {
                    return Err(format!(
                        "dma-region capability {} overlaps {}",
                        region.id, previous.id
                    ));
                }
            }
        }
    }

    Ok(())
}

fn validate_device_range(
    kind: &str,
    id: &str,
    base: u64,
    length: u64,
    page_aligned_base: bool,
) -> Result<(), String> {
    if length == 0 {
        return Err(format!("{kind} capability {id} length must be nonzero"));
    }
    if length > MAX_DEVICE_MAPPING_LENGTH {
        return Err(format!(
            "{kind} capability {id} exceeds max mapping length {MAX_DEVICE_MAPPING_LENGTH}"
        ));
    }
    base.checked_add(length - 1)
        .ok_or_else(|| format!("{kind} capability {id} range overflows"))?;
    if page_aligned_base && base % PAGE_SIZE != 0 {
        return Err(format!("{kind} capability {id} base must be page-aligned"));
    }
    Ok(())
}

fn ranges_overlap(
    base: u64,
    length: u64,
    other_base: u64,
    other_length: u64,
) -> Result<bool, String> {
    if length == 0 || other_length == 0 {
        return Ok(false);
    }
    let end = base
        .checked_add(length)
        .ok_or_else(|| "hardware authority range overflows".to_owned())?;
    let other_end = other_base
        .checked_add(other_length)
        .ok_or_else(|| "hardware authority range overflows".to_owned())?;
    Ok(base < other_end && other_base < end)
}

enum EndpointRefKind {
    Requires(u16),
    Provides,
}

fn add_process_endpoint_ref(
    processes: &mut [NativeProcess],
    service_id: &str,
    endpoint: String,
    kind: EndpointRefKind,
) {
    let Some(process) = processes
        .iter_mut()
        .find(|process| process.service_id == service_id)
    else {
        return;
    };

    let refs = match kind {
        EndpointRefKind::Requires(rights) => {
            if !process
                .requires_endpoints
                .iter()
                .any(|existing| existing.endpoint == endpoint)
            {
                process
                    .requires_endpoints
                    .push(EndpointRequirement { endpoint, rights });
            }
            return;
        }
        EndpointRefKind::Provides => &mut process.provides_endpoints,
    };
    if !refs.iter().any(|existing| existing == &endpoint) {
        refs.push(endpoint);
    }
}

fn add_process_start_after(
    processes: &mut [NativeProcess],
    service_id: &str,
    dependency_id: &str,
) -> Result<(), String> {
    let dependency_name =
        native_process_for_service(processes, dependency_id).ok_or_else(|| {
            format!("service {service_id} depends on non-native service {dependency_id}")
        })?;
    let Some(process) = processes
        .iter_mut()
        .find(|process| process.service_id == service_id)
    else {
        return Err(format!("unknown native service {service_id}"));
    };
    if process.name == dependency_name {
        return Ok(());
    }
    if !process
        .start_after
        .iter()
        .any(|existing| existing == &dependency_name)
    {
        process.start_after.push(dependency_name);
    }
    Ok(())
}

fn native_process_for_service(processes: &[NativeProcess], service_id: &str) -> Option<String> {
    processes
        .iter()
        .find(|process| process.service_id == service_id)
        .map(|process| process.name.clone())
}

fn native_requirement_exists(
    manifest: &GenerationManifest,
    processes: &[NativeProcess],
    capability_id: &str,
    root_service_id: &str,
) -> bool {
    manifest.services.iter().any(|service| {
        service.id != root_service_id
            && native_process_for_service(processes, &service.id).is_some()
            && service
                .requires
                .iter()
                .any(|requirement| requirement.capability == capability_id)
    })
}

fn push_unique_endpoint(endpoints: &mut Vec<Endpoint>, endpoint: String) {
    if !endpoints.iter().any(|existing| existing.name == endpoint) {
        endpoints.push(Endpoint { name: endpoint });
    }
}

fn restart_policy(policy: &str) -> Result<u16, String> {
    match policy {
        "never" => Ok(RESTART_NEVER),
        "on-failure" => Ok(RESTART_ON_FAILURE),
        "always" => Ok(RESTART_ALWAYS),
        other => Err(format!("unsupported restart policy {other}")),
    }
}

fn endpoint_rights_mask(
    required: &[String],
    capability: &[String],
    context: &str,
) -> Result<u16, String> {
    let mask = rights_mask(required, capability, context)?;
    if mask != RIGHT_SEND {
        return Err(format!(
            "ipc endpoint {context} native requirements are send-only"
        ));
    }
    Ok(mask)
}

fn rights_mask(required: &[String], capability: &[String], context: &str) -> Result<u16, String> {
    let capability_mask = declared_rights_mask(capability, context)?;
    let mask = if required.is_empty() {
        capability_mask
    } else {
        let required_mask = raw_rights_mask(required, context)?;
        if required_mask & !capability_mask != 0 {
            return Err(format!(
                "capability {context} requirement exceeds declared rights"
            ));
        }
        required_mask
    };

    if mask == 0 {
        return Err(format!("capability {context} has no native rights"));
    }

    Ok(mask)
}

fn declared_rights_mask(rights: &[String], context: &str) -> Result<u16, String> {
    let mask = raw_rights_mask(rights, context)?;
    if mask == 0 {
        return Err(format!("capability {context} has no native rights"));
    }
    Ok(mask)
}

fn raw_rights_mask(rights: &[String], context: &str) -> Result<u16, String> {
    let mut mask = 0;
    for right in rights {
        match right.as_str() {
            "send" => mask |= RIGHT_SEND,
            "receive" => mask |= RIGHT_RECEIVE,
            "read" => mask |= RIGHT_READ,
            "write" => mask |= RIGHT_WRITE,
            "readwrite" | "read-write" => mask |= RIGHT_READ | RIGHT_WRITE,
            "snapshot" => mask |= RIGHT_SNAPSHOT,
            "restore" => mask |= RIGHT_RESTORE,
            "control" => mask |= RIGHT_CONTROL,
            "bind" => mask |= RIGHT_BIND,
            "listen" => mask |= RIGHT_LISTEN,
            "map" => mask |= RIGHT_MAP,
            "resolve" => mask |= RIGHT_RESOLVE,
            "create" => mask |= RIGHT_CREATE,
            "unlink" => mask |= RIGHT_UNLINK,
            "rename" => mask |= RIGHT_RENAME,
            "mount" => mask |= RIGHT_MOUNT,
            other => return Err(format!("unsupported native right {other} for {context}")),
        }
    }

    Ok(mask)
}

fn service_mount_root(service: &Service) -> Result<String, String> {
    let Some(value) = service.extra.get("mountRoot") else {
        return Err(format!("service {} must declare mountRoot", service.id));
    };
    let Some(root) = value.as_str() else {
        return Err(format!("service {} mountRoot must be a string", service.id));
    };
    validate_manifest_vfs_path(&service.id, root)?;
    Ok(root.to_owned())
}

fn service_mounts(service: &Service) -> Result<Vec<ProcessMount>, String> {
    let Some(value) = service.extra.get("mounts") else {
        return Ok(Vec::new());
    };
    let Some(mounts) = value.as_array() else {
        return Err(format!("service {} mounts must be an array", service.id));
    };
    if mounts.len() > MAX_PROCESS_MOUNTS {
        return Err(format!(
            "service {} declares too many mounts; max {MAX_PROCESS_MOUNTS}",
            service.id
        ));
    }

    let mut result = Vec::new();
    for (index, mount) in mounts.iter().enumerate() {
        let context = format!("service {} mounts[{index}]", service.id);
        let Some(object) = mount.as_object() else {
            return Err(format!("{context} must be an object"));
        };
        let path = required_mount_path(object.get("path"), &context, "path")?;
        let source = required_mount_path(object.get("source"), &context, "source")?;
        if path == "/" {
            return Err(format!("{context} path cannot replace namespace root"));
        }
        let Some(read_only) = object.get("readOnly") else {
            return Err(format!("{context} must declare readOnly"));
        };
        let Some(read_only) = read_only.as_bool() else {
            return Err(format!("{context} readOnly must be a boolean"));
        };
        let mut flags = PROCESS_MOUNT_FLAG_BIND;
        if read_only {
            flags |= PROCESS_MOUNT_FLAG_READ_ONLY;
        }
        result.push(ProcessMount {
            path,
            source,
            flags,
        });
    }

    Ok(result)
}

fn required_mount_path(
    value: Option<&Value>,
    context: &str,
    field: &str,
) -> Result<String, String> {
    let Some(value) = value else {
        return Err(format!("{context} must declare {field}"));
    };
    let Some(path) = value.as_str() else {
        return Err(format!("{context} {field} must be a string"));
    };
    validate_manifest_vfs_path(context, path)?;
    Ok(path.to_owned())
}

fn known_process_mount_flags() -> u16 {
    PROCESS_MOUNT_FLAG_BIND | PROCESS_MOUNT_FLAG_READ_ONLY
}

fn service_label(service: &Service) -> String {
    if !service.name.is_empty() {
        service.name.clone()
    } else {
        module_basename(&service.id)
    }
}

fn state_volume_mount_component(id: &str) -> Result<&str, String> {
    let Some(component) = id.strip_prefix("state:") else {
        return Err(format!(
            "state volume {id} must use the state: id namespace"
        ));
    };
    if component.is_empty()
        || component.len() > STRING_LEN
        || component
            .as_bytes()
            .iter()
            .any(|byte| *byte == b'/' || *byte == 0)
    {
        return Err(format!(
            "state volume {id} has invalid VFS mount component {component}"
        ));
    }
    Ok(component)
}

fn endpoint_name(capability_id: &str) -> String {
    capability_id
        .trim_start_matches("cap:")
        .chars()
        .map(|ch| match ch {
            '.' | ':' | '_' => '-',
            other => other,
        })
        .collect()
}

fn init_endpoint_auth_slot(endpoints: &[Endpoint], endpoint: &str) -> Result<u16, String> {
    let index = endpoints
        .iter()
        .position(|existing| existing.name == endpoint)
        .ok_or_else(|| format!("unknown endpoint {endpoint}"))?;
    if index < 2 {
        return Err(format!(
            "endpoint {endpoint} is reserved and cannot receive delegated init authority"
        ));
    }
    Ok(INIT_ENDPOINT_AUTH_BASE_SLOT + (index as u16 - 2))
}

fn provided_endpoint_target_slot(provided_index: usize) -> u16 {
    if provided_index == 0 {
        SERVICE_CAP_SLOT
    } else {
        READINESS_RESERVED_CAP_SLOT + provided_index as u16
    }
}

fn provided_endpoint_target_slot_for_capability(
    manifest: &GenerationManifest,
    processes: &[NativeProcess],
    root_service_id: &str,
    service_id: &str,
    capability_id: &str,
) -> Result<u16, String> {
    let service = manifest.service(service_id).ok_or_else(|| {
        format!("endpoint capability {capability_id} has unknown provider {service_id}")
    })?;
    let mut provided_index = 0;
    for provided in &service.provides {
        let Some(capability) = manifest.capability(provided) else {
            continue;
        };
        if capability.kind != "ipc-endpoint"
            || !native_requirement_exists(manifest, processes, provided, root_service_id)
        {
            continue;
        }
        if provided == capability_id {
            return Ok(provided_endpoint_target_slot(provided_index));
        }
        provided_index += 1;
    }

    Err(format!(
        "service {service_id} does not provide endpoint capability {capability_id}"
    ))
}

fn endpoint_target_slot(provided_count: usize, requirement_index: usize) -> u16 {
    if provided_count == 0 && requirement_index == 0 {
        SERVICE_CAP_SLOT
    } else if provided_count == 0 {
        READINESS_RESERVED_CAP_SLOT + requirement_index as u16
    } else {
        READINESS_RESERVED_CAP_SLOT + provided_count as u16 + requirement_index as u16
    }
}

fn module_basename(value: &str) -> String {
    value
        .rsplit('/')
        .next()
        .unwrap_or(value)
        .trim_start_matches("svc:")
        .trim_start_matches("exe:")
        .to_owned()
}

fn process_index(plan: &BootPlan, name: &str) -> Result<usize, String> {
    plan.processes
        .iter()
        .position(|process| process.name == name)
        .ok_or_else(|| format!("unknown process {name}"))
}

fn endpoint_index(plan: &BootPlan, name: &str) -> Result<usize, String> {
    plan.endpoints
        .iter()
        .position(|endpoint| endpoint.name == name)
        .ok_or_else(|| format!("unknown endpoint {name}"))
}

fn store_index(plan: &BootPlan, id: &str) -> Result<usize, String> {
    plan.store_objects
        .iter()
        .position(|object| object.id == id)
        .ok_or_else(|| format!("unknown store object {id}"))
}

fn state_index(plan: &BootPlan, id: &str) -> Result<usize, String> {
    plan.state_volumes
        .iter()
        .position(|state| state.id == id)
        .ok_or_else(|| format!("unknown state volume {id}"))
}

fn network_port_index(plan: &BootPlan, id: &str) -> Result<usize, String> {
    plan.network_ports
        .iter()
        .position(|port| port.id == id)
        .ok_or_else(|| format!("unknown network port {id}"))
}

fn io_port_index(plan: &BootPlan, id: &str) -> Result<usize, String> {
    plan.io_ports
        .iter()
        .position(|port| port.id == id)
        .ok_or_else(|| format!("unknown io port {id}"))
}

fn mmio_region_index(plan: &BootPlan, id: &str) -> Result<usize, String> {
    plan.mmio_regions
        .iter()
        .position(|region| region.id == id)
        .ok_or_else(|| format!("unknown mmio region {id}"))
}

fn framebuffer_index(plan: &BootPlan, id: &str) -> Result<usize, String> {
    plan.framebuffers
        .iter()
        .position(|framebuffer| framebuffer.id == id)
        .ok_or_else(|| format!("unknown framebuffer {id}"))
}

fn interrupt_line_index(plan: &BootPlan, id: &str) -> Result<usize, String> {
    plan.interrupt_lines
        .iter()
        .position(|line| line.id == id)
        .ok_or_else(|| format!("unknown interrupt line {id}"))
}

fn dma_region_index(plan: &BootPlan, id: &str) -> Result<usize, String> {
    plan.dma_regions
        .iter()
        .position(|region| region.id == id)
        .ok_or_else(|| format!("unknown dma region {id}"))
}

fn pci_device_index(plan: &BootPlan, id: &str) -> Result<usize, String> {
    plan.pci_devices
        .iter()
        .position(|device| device.id == id)
        .ok_or_else(|| format!("unknown pci device {id}"))
}

fn virtio_device_index(plan: &BootPlan, id: &str) -> Result<usize, String> {
    plan.virtio_devices
        .iter()
        .position(|device| device.id == id)
        .ok_or_else(|| format!("unknown virtio device {id}"))
}

fn namespace_index(plan: &BootPlan, id: &str) -> Result<usize, String> {
    plan.namespaces
        .iter()
        .position(|namespace| namespace.id == id)
        .ok_or_else(|| format!("unknown namespace {id}"))
}

fn object_index_for_kind(
    plan: &BootPlan,
    object_kind: u16,
    object_name: &str,
) -> Result<usize, String> {
    match object_kind {
        OBJECT_ENDPOINT => endpoint_index(plan, object_name),
        OBJECT_STORE => store_index(plan, object_name),
        OBJECT_STATE => state_index(plan, object_name),
        OBJECT_TIMER if object_name == "monotonic-timer" => Ok(0),
        OBJECT_NETWORK_PORT => network_port_index(plan, object_name),
        OBJECT_IO_PORT_RANGE => io_port_index(plan, object_name),
        OBJECT_MMIO_REGION => mmio_region_index(plan, object_name),
        OBJECT_FRAMEBUFFER => framebuffer_index(plan, object_name),
        OBJECT_INTERRUPT_LINE => interrupt_line_index(plan, object_name),
        OBJECT_DMA_REGION => dma_region_index(plan, object_name),
        OBJECT_PCI_DEVICE => pci_device_index(plan, object_name),
        OBJECT_VIRTIO_DEVICE => virtio_device_index(plan, object_name),
        OBJECT_NAMESPACE => namespace_index(plan, object_name),
        OBJECT_VFS_ROOT => vfs_root_index(plan, object_name),
        other => Err(format!("unsupported native object kind {other}")),
    }
}

fn vfs_root_index(plan: &BootPlan, id: &str) -> Result<usize, String> {
    plan.vfs_roots
        .iter()
        .position(|root| root.id == id)
        .ok_or_else(|| format!("unknown vfs root {id}"))
}

fn object_index(plan: &BootPlan, grant: &Grant) -> Result<usize, String> {
    object_index_for_kind(plan, grant.object_kind, &grant.object_name)
}

fn push_process_ref_list(
    bytes: &mut Vec<u8>,
    values: &[String],
    plan: &BootPlan,
) -> Result<(), String> {
    if values.len() > MAX_PROCESS_REFS {
        return Err("too many process refs".to_owned());
    }
    push_u16(bytes, values.len() as u16);
    let mut written = 0;
    for value in values {
        push_u16(bytes, process_index(plan, value)? as u16);
        written += 1;
    }
    while written < MAX_PROCESS_REFS {
        push_u16(bytes, u16::MAX);
        written += 1;
    }
    Ok(())
}

fn push_endpoint_ref_list(
    bytes: &mut Vec<u8>,
    values: &[String],
    plan: &BootPlan,
) -> Result<(), String> {
    if values.len() > MAX_PROCESS_REFS {
        return Err("too many endpoint refs".to_owned());
    }
    push_u16(bytes, values.len() as u16);
    let mut written = 0;
    for value in values {
        push_u16(bytes, endpoint_index(plan, value)? as u16);
        written += 1;
    }
    while written < MAX_PROCESS_REFS {
        push_u16(bytes, u16::MAX);
        written += 1;
    }
    Ok(())
}

fn push_endpoint_requirement_list(
    bytes: &mut Vec<u8>,
    values: &[EndpointRequirement],
    plan: &BootPlan,
) -> Result<(), String> {
    if values.len() > MAX_PROCESS_REFS {
        return Err("too many endpoint requirement refs".to_owned());
    }
    push_u16(bytes, values.len() as u16);
    let mut written = 0;
    for value in values {
        push_u16(bytes, endpoint_index(plan, &value.endpoint)? as u16);
        push_u16(bytes, value.rights);
        written += 1;
    }
    while written < MAX_PROCESS_REFS {
        push_u16(bytes, u16::MAX);
        push_u16(bytes, 0);
        written += 1;
    }
    Ok(())
}

fn push_process_mount_list(bytes: &mut Vec<u8>, values: &[ProcessMount]) -> Result<(), String> {
    if values.len() > MAX_PROCESS_MOUNTS {
        return Err("too many process mounts".to_owned());
    }
    push_u16(bytes, values.len() as u16);
    let mut written = 0;
    for value in values {
        push_fixed_str(bytes, &value.path)?;
        push_fixed_str(bytes, &value.source)?;
        push_u16(bytes, value.flags);
        push_u16(bytes, 0);
        written += 1;
    }
    while written < MAX_PROCESS_MOUNTS {
        push_fixed_str(bytes, "")?;
        push_fixed_str(bytes, "")?;
        push_u16(bytes, 0);
        push_u16(bytes, 0);
        written += 1;
    }
    Ok(())
}

fn serialize_graph_records(plan: &BootPlan) -> Result<Vec<u8>, String> {
    if plan.graph_nodes.len() > MAX_GRAPH_NODES {
        return Err(format!(
            "native graph store exceeds {MAX_GRAPH_NODES} nodes"
        ));
    }
    if plan.graph_edges.len() > MAX_GRAPH_EDGES {
        return Err(format!(
            "native graph store exceeds {MAX_GRAPH_EDGES} edges"
        ));
    }

    let mut bytes = Vec::with_capacity(
        plan.graph_nodes.len() * GRAPH_NODE_RECORD_LEN
            + plan.graph_edges.len() * GRAPH_EDGE_RECORD_LEN,
    );

    for node in &plan.graph_nodes {
        push_u16(&mut bytes, node.kind);
        push_u16(&mut bytes, node.object_kind);
        push_fixed_str(&mut bytes, &node.id)?;
        push_fixed_str(&mut bytes, &node.label)?;
    }

    for edge in &plan.graph_edges {
        push_u16(&mut bytes, edge.kind);
        push_u16(
            &mut bytes,
            graph_node_index(&plan.graph_nodes, &edge.from)? as u16,
        );
        push_u16(
            &mut bytes,
            graph_node_index(&plan.graph_nodes, &edge.to)? as u16,
        );
        push_u16(&mut bytes, edge.rights);
        push_fixed_str(&mut bytes, &edge.id)?;
    }

    Ok(bytes)
}

fn serialize_policy_records(plan: &BootPlan) -> Result<Vec<u8>, String> {
    if plan.policy_capabilities.len() > MAX_POLICY_CAPABILITIES {
        return Err(format!(
            "native policy exceeds {MAX_POLICY_CAPABILITIES} capability facts"
        ));
    }
    if plan.policy_requirements.len() > MAX_POLICY_REQUIREMENTS {
        return Err(format!(
            "native policy exceeds {MAX_POLICY_REQUIREMENTS} requirement facts"
        ));
    }
    if plan.policy_provides.len() > MAX_POLICY_PROVIDES {
        return Err(format!(
            "native policy exceeds {MAX_POLICY_PROVIDES} provide facts"
        ));
    }
    if plan.policy_mounts.len() > MAX_POLICY_MOUNTS {
        return Err(format!(
            "native policy exceeds {MAX_POLICY_MOUNTS} mount facts"
        ));
    }
    if plan.policy_state_paths.len() > MAX_POLICY_STATE_PATHS {
        return Err(format!(
            "native policy exceeds {MAX_POLICY_STATE_PATHS} state path facts"
        ));
    }
    if plan.policy_bootstraps.len() > MAX_POLICY_BOOTSTRAPS {
        return Err(format!(
            "native policy exceeds {MAX_POLICY_BOOTSTRAPS} bootstrap facts"
        ));
    }

    let mut bytes = Vec::with_capacity(
        plan.policy_capabilities.len() * POLICY_CAPABILITY_RECORD_LEN
            + plan.policy_requirements.len() * POLICY_REQUIREMENT_RECORD_LEN
            + plan.policy_provides.len() * POLICY_PROVIDE_RECORD_LEN
            + plan.policy_mounts.len() * POLICY_MOUNT_RECORD_LEN
            + plan.policy_state_paths.len() * POLICY_STATE_PATH_RECORD_LEN
            + plan.policy_bootstraps.len() * POLICY_BOOTSTRAP_RECORD_LEN,
    );

    for capability in &plan.policy_capabilities {
        push_fixed_str(&mut bytes, &capability.id)?;
        push_fixed_str(&mut bytes, &capability.provider)?;
        push_u16(&mut bytes, capability.object_kind);
        push_u16(&mut bytes, capability.object_index as u16);
        push_u16(&mut bytes, capability.rights);
        push_u16(&mut bytes, 0);
    }

    for requirement in &plan.policy_requirements {
        push_fixed_str(&mut bytes, &requirement.service)?;
        push_fixed_str(&mut bytes, &requirement.capability)?;
        push_u16(&mut bytes, requirement.rights);
        push_u16(&mut bytes, 0);
    }

    for provide in &plan.policy_provides {
        push_fixed_str(&mut bytes, &provide.service)?;
        push_fixed_str(&mut bytes, &provide.capability)?;
    }

    for mount in &plan.policy_mounts {
        push_fixed_str(&mut bytes, &mount.service)?;
        push_fixed_str(&mut bytes, &mount.mount_root)?;
        push_fixed_str(&mut bytes, &mount.path)?;
        push_fixed_str(&mut bytes, &mount.source)?;
        push_u16(&mut bytes, mount.flags);
        push_u16(&mut bytes, 0);
    }

    for state_path in &plan.policy_state_paths {
        push_fixed_str(&mut bytes, &state_path.service)?;
        push_fixed_str(&mut bytes, &state_path.state)?;
        push_fixed_str(&mut bytes, &state_path.root)?;
        push_u16(&mut bytes, state_path.rights);
        push_u16(&mut bytes, 0);
    }

    for bootstrap in &plan.policy_bootstraps {
        push_fixed_str(&mut bytes, &bootstrap.service)?;
        push_fixed_str(&mut bytes, &bootstrap.authority)?;
        push_fixed_str(&mut bytes, &bootstrap.rule)?;
        push_u64(&mut bytes, bootstrap.rights);
    }

    Ok(bytes)
}

fn push_count(bytes: &mut Vec<u8>, count: usize, label: &str) -> Result<(), String> {
    let count =
        u16::try_from(count).map_err(|_| format!("KrustBoot {label} count does not fit in u16"))?;
    push_u16(bytes, count);
    Ok(())
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

struct BodySections {
    generation: (usize, usize),
    boot_modules: (usize, usize),
    processes: (usize, usize),
    endpoints: (usize, usize),
    grants: (usize, usize),
    store_objects: (usize, usize),
    state_volumes: (usize, usize),
    network_ports: (usize, usize),
    io_ports: (usize, usize),
    mmio_regions: (usize, usize),
    framebuffers: (usize, usize),
    interrupt_lines: (usize, usize),
    dma_regions: (usize, usize),
    pci_devices: (usize, usize),
    virtio_devices: (usize, usize),
    namespaces: (usize, usize),
    vfs_roots: (usize, usize),
}

fn wrap_v1(manifest: &GenerationManifest, plan: &BootPlan, body: &[u8]) -> Result<Vec<u8>, String> {
    let sections = BodySections::new(plan);
    let record_table_offset = V1_HEADER_SIZE;
    let payload_offset = V1_HEADER_SIZE + V1_RECORD_COUNT * V1_RECORD_SIZE;
    let total_size = payload_offset
        .checked_add(body.len())
        .ok_or_else(|| "KrustBoot v1 manifest size overflow".to_owned())?;
    let total_size_u32 = u32::try_from(total_size)
        .map_err(|_| "KrustBoot v1 manifest is too large for u32 total_size".to_owned())?;

    let mut bytes = Vec::with_capacity(total_size);
    bytes.extend_from_slice(V1_MAGIC);
    push_u16(&mut bytes, V1_VERSION);
    push_u16(&mut bytes, V1_HEADER_SIZE as u16);
    push_u32(&mut bytes, total_size_u32);
    push_u32(&mut bytes, record_table_offset as u32);
    push_u16(&mut bytes, V1_RECORD_COUNT as u16);
    push_u16(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_fixed_str(&mut bytes, &manifest.generation.id)?;
    push_fixed_str(
        &mut bytes,
        manifest.generation.parent.as_deref().unwrap_or_default(),
    )?;
    debug_assert_eq!(bytes.len(), V1_HEADER_SIZE);

    push_record(
        &mut bytes,
        RECORD_BOOT_MODULE,
        0,
        payload_offset + sections.boot_modules.0,
        sections.boot_modules.1,
    )?;
    push_record(
        &mut bytes,
        RECORD_PROCESS,
        0,
        payload_offset + sections.processes.0,
        sections.processes.1,
    )?;
    push_record(
        &mut bytes,
        RECORD_ENDPOINT,
        0,
        payload_offset + sections.endpoints.0,
        sections.endpoints.1,
    )?;
    push_record(
        &mut bytes,
        RECORD_GRANT,
        0,
        payload_offset + sections.grants.0,
        sections.grants.1,
    )?;
    push_record(
        &mut bytes,
        RECORD_STORE_OBJECT,
        0,
        payload_offset + sections.store_objects.0,
        sections.store_objects.1,
    )?;
    push_record(
        &mut bytes,
        RECORD_STATE_VOLUME,
        0,
        payload_offset + sections.state_volumes.0,
        sections.state_volumes.1,
    )?;
    push_record(&mut bytes, RECORD_TIMER, 0, payload_offset, 0)?;
    push_record(
        &mut bytes,
        RECORD_GENERATION,
        0,
        payload_offset + sections.generation.0,
        sections.generation.1,
    )?;
    push_record(
        &mut bytes,
        RECORD_POLICY,
        0,
        payload_offset + sections.network_ports.0,
        sections.network_ports.1
            + sections.io_ports.1
            + sections.mmio_regions.1
            + sections.framebuffers.1
            + sections.interrupt_lines.1
            + sections.dma_regions.1
            + sections.pci_devices.1
            + sections.virtio_devices.1
            + sections.namespaces.1
            + sections.vfs_roots.1,
    )?;
    debug_assert_eq!(bytes.len(), payload_offset);

    bytes.extend_from_slice(body);
    rewrite_v1_checksum(&mut bytes)?;
    Ok(bytes)
}

impl BodySections {
    fn new(plan: &BootPlan) -> Self {
        let generation = (48, STRING_LEN * 2);
        let boot_modules = (
            COMPACT_HEADER_SIZE,
            plan.boot_modules.len() * BOOT_MODULE_RECORD_LEN,
        );
        let processes = (
            boot_modules.0 + boot_modules.1,
            plan.processes.len() * PROCESS_RECORD_LEN,
        );
        let endpoints = (
            processes.0 + processes.1,
            plan.endpoints.len() * ENDPOINT_RECORD_LEN,
        );
        let grants = (
            endpoints.0 + endpoints.1,
            plan.grants.len() * GRANT_RECORD_LEN,
        );
        let store_objects = (
            grants.0 + grants.1,
            plan.store_objects.len() * STORE_OBJECT_RECORD_LEN,
        );
        let state_volumes = (
            store_objects.0 + store_objects.1,
            plan.state_volumes.len() * STATE_VOLUME_RECORD_LEN,
        );
        let network_ports = (
            state_volumes.0 + state_volumes.1,
            plan.network_ports.len() * NETWORK_PORT_RECORD_LEN,
        );
        let io_ports = (
            network_ports.0 + network_ports.1,
            plan.io_ports.len() * IO_PORT_RECORD_LEN,
        );
        let mmio_regions = (
            io_ports.0 + io_ports.1,
            plan.mmio_regions.len() * MMIO_REGION_RECORD_LEN,
        );
        let framebuffers = (
            mmio_regions.0 + mmio_regions.1,
            plan.framebuffers.len() * FRAMEBUFFER_RECORD_LEN,
        );
        let interrupt_lines = (
            framebuffers.0 + framebuffers.1,
            plan.interrupt_lines.len() * INTERRUPT_LINE_RECORD_LEN,
        );
        let dma_regions = (
            interrupt_lines.0 + interrupt_lines.1,
            plan.dma_regions.len() * DMA_REGION_RECORD_LEN,
        );
        let pci_devices = (
            dma_regions.0 + dma_regions.1,
            plan.pci_devices.len() * PCI_DEVICE_RECORD_LEN,
        );
        let virtio_devices = (
            pci_devices.0 + pci_devices.1,
            plan.virtio_devices.len() * VIRTIO_DEVICE_RECORD_LEN,
        );
        let namespace_len = plan
            .namespaces
            .iter()
            .map(|namespace| STRING_LEN + 2 + namespace.entries.len() * NAMESPACE_ENTRY_RECORD_LEN)
            .sum();
        let namespaces = (virtio_devices.0 + virtio_devices.1, namespace_len);
        let vfs_roots = (
            namespaces.0 + namespaces.1,
            plan.vfs_roots.len() * VFS_ROOT_RECORD_LEN,
        );

        Self {
            generation,
            boot_modules,
            processes,
            endpoints,
            grants,
            store_objects,
            state_volumes,
            network_ports,
            io_ports,
            mmio_regions,
            framebuffers,
            interrupt_lines,
            dma_regions,
            pci_devices,
            virtio_devices,
            namespaces,
            vfs_roots,
        }
    }
}

fn push_record(
    bytes: &mut Vec<u8>,
    kind: u16,
    id: u16,
    offset: usize,
    length: usize,
) -> Result<(), String> {
    let offset = u32::try_from(offset)
        .map_err(|_| "KrustBoot v1 record offset does not fit in u32".to_owned())?;
    let length = u32::try_from(length)
        .map_err(|_| "KrustBoot v1 record length does not fit in u32".to_owned())?;
    push_u16(bytes, kind);
    push_u16(bytes, id);
    push_u32(bytes, offset);
    push_u32(bytes, length);
    Ok(())
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn checksum32(bytes: &[u8]) -> u32 {
    let mut checksum = 0u32;
    for (index, byte) in bytes.iter().copied().enumerate() {
        checksum = checksum.wrapping_add((byte as u32).wrapping_mul(index as u32 + 1));
    }
    checksum
}

fn rewrite_v1_checksum(bytes: &mut [u8]) -> Result<(), String> {
    if bytes.len() < V1_CHECKSUM_OFFSET + 4 {
        return Err("KrustBoot v1 manifest is too short for checksum".to_owned());
    }
    bytes[V1_CHECKSUM_OFFSET..V1_CHECKSUM_OFFSET + 4].copy_from_slice(&0u32.to_le_bytes());
    let checksum = v1_checksum(bytes);
    bytes[V1_CHECKSUM_OFFSET..V1_CHECKSUM_OFFSET + 4].copy_from_slice(&checksum.to_le_bytes());
    Ok(())
}

fn corrupt_missing_provider(bytes: &mut [u8]) -> Result<(), String> {
    if bytes.len() < V1_PAYLOAD_OFFSET + COMPACT_HEADER_SIZE {
        return Err("KrustBoot manifest is too short to remove providers".to_owned());
    }
    let payload = V1_PAYLOAD_OFFSET;
    let boot_modules = read_u16_at(bytes, payload + 18)? as usize;
    let processes = read_u16_at(bytes, payload + 20)? as usize;
    let process_base = payload + COMPACT_HEADER_SIZE + boot_modules * BOOT_MODULE_RECORD_LEN;
    let mut index = 0;
    while index < processes {
        let offset = process_base + index * PROCESS_RECORD_LEN + PROCESS_PROVIDES_COUNT_OFFSET;
        if offset + 2 > bytes.len() {
            return Err("KrustBoot process record is out of bounds".to_owned());
        }
        bytes[offset..offset + 2].copy_from_slice(&0u16.to_le_bytes());
        index += 1;
    }
    Ok(())
}

fn corrupt_policy_version(bytes: &mut [u8]) -> Result<(), String> {
    let offset = compact_policy_header_offset(bytes)?;
    if offset + 2 > bytes.len() {
        return Err("KrustBoot policy header is out of bounds".to_owned());
    }
    bytes[offset..offset + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    Ok(())
}

fn corrupt_policy_hash(bytes: &mut [u8]) -> Result<(), String> {
    let offset = compact_policy_header_offset(bytes)?
        .checked_add(14)
        .ok_or_else(|| "KrustBoot policy hash offset overflow".to_owned())?;
    if offset + STRING_LEN > bytes.len() {
        return Err("KrustBoot policy hash is out of bounds".to_owned());
    }
    bytes[offset] ^= 0x01;
    Ok(())
}

fn corrupt_policy_mount_root(bytes: &mut [u8]) -> Result<(), String> {
    let payload = V1_PAYLOAD_OFFSET;
    if bytes.len() < payload + COMPACT_HEADER_SIZE {
        return Err("KrustBoot manifest is too short to corrupt mount root".to_owned());
    }
    let boot_modules = read_u16_at(bytes, payload + 18)? as usize;
    let processes = read_u16_at(bytes, payload + 20)? as usize;
    let process_base = payload
        .checked_add(COMPACT_HEADER_SIZE)
        .and_then(|offset| offset.checked_add(boot_modules.checked_mul(BOOT_MODULE_RECORD_LEN)?))
        .ok_or_else(|| "KrustBoot process table offset overflow".to_owned())?;
    let mut index = 0;
    while index < processes {
        let offset = process_base
            .checked_add(
                index
                    .checked_mul(PROCESS_RECORD_LEN)
                    .and_then(|offset| offset.checked_add(PROCESS_MOUNT_ROOT_OFFSET))
                    .ok_or_else(|| "KrustBoot process mount-root offset overflow".to_owned())?,
            )
            .ok_or_else(|| "KrustBoot process mount-root offset overflow".to_owned())?;
        if offset + STRING_LEN > bytes.len() {
            return Err("KrustBoot process mount-root is out of bounds".to_owned());
        }
        if fixed_str_equals(bytes, offset, "/fs/app") {
            let mut slot = [0u8; STRING_LEN];
            slot[0] = b'/';
            bytes[offset..offset + STRING_LEN].copy_from_slice(&slot);
            return Ok(());
        }
        index += 1;
    }
    Err("KrustBoot manifest has no /fs/app mount root to corrupt".to_owned())
}

fn corrupt_policy_state_root(bytes: &mut [u8]) -> Result<(), String> {
    let payload = V1_PAYLOAD_OFFSET;
    if bytes.len() < payload + COMPACT_HEADER_SIZE {
        return Err("KrustBoot manifest is too short to corrupt state root".to_owned());
    }
    let vfs_roots = read_u16_at(bytes, payload + 48)? as usize;
    let root_base = compact_vfs_roots_offset(bytes)?;
    let mut index = 0;
    while index < vfs_roots {
        let offset = root_base
            .checked_add(
                index
                    .checked_mul(VFS_ROOT_RECORD_LEN)
                    .ok_or_else(|| "KrustBoot VFS-root offset overflow".to_owned())?,
            )
            .ok_or_else(|| "KrustBoot VFS-root offset overflow".to_owned())?;
        let root_offset = offset
            .checked_add(STRING_LEN)
            .ok_or_else(|| "KrustBoot VFS-root path offset overflow".to_owned())?;
        if root_offset + STRING_LEN > bytes.len() {
            return Err("KrustBoot VFS-root record is out of bounds".to_owned());
        }
        if fixed_str_equals(bytes, root_offset, "/state/a") {
            let mut slot = [0u8; STRING_LEN];
            slot[.."/state/counter".len()].copy_from_slice(b"/state/counter");
            bytes[root_offset..root_offset + STRING_LEN].copy_from_slice(&slot);
            return Ok(());
        }
        index += 1;
    }
    Err("KrustBoot manifest has no /state/a VFS root to corrupt".to_owned())
}

fn corrupt_policy_excess_grant(bytes: &mut [u8]) -> Result<(), String> {
    let payload = V1_PAYLOAD_OFFSET;
    if bytes.len() < payload + COMPACT_HEADER_SIZE {
        return Err("KrustBoot manifest is too short to corrupt policy grant".to_owned());
    }
    let grants = read_u16_at(bytes, payload + 24)? as usize;
    let grant_base = compact_grants_offset(bytes)?;
    let mut grant_index = 0;
    while grant_index < grants {
        let offset = grant_base
            .checked_add(
                grant_index
                    .checked_mul(GRANT_RECORD_LEN)
                    .ok_or_else(|| "KrustBoot grant offset overflow".to_owned())?,
            )
            .ok_or_else(|| "KrustBoot grant offset overflow".to_owned())?;
        if offset + GRANT_RECORD_LEN > bytes.len() {
            return Err("KrustBoot grant record is out of bounds".to_owned());
        }
        let process_index = read_u16_at(bytes, offset)?;
        let object_kind = read_u16_at(bytes, offset + 2)?;
        let rights = read_u16_at(bytes, offset + 8)?;
        if process_index != 0 && object_kind == OBJECT_ENDPOINT && rights == RIGHT_RECEIVE {
            bytes[offset + 8..offset + 10].copy_from_slice(&RIGHT_SEND.to_le_bytes());
            rewrite_grant_graph_edge_rights(bytes, grant_index, RIGHT_SEND)?;
            return Ok(());
        }
        grant_index += 1;
    }
    Err("KrustBoot manifest has no endpoint receive grant to corrupt".to_owned())
}

fn rewrite_grant_graph_edge_rights(
    bytes: &mut [u8],
    grant_index: usize,
    rights: u16,
) -> Result<(), String> {
    let payload = V1_PAYLOAD_OFFSET;
    let graph_nodes = read_u16_at(bytes, payload + COMPACT_GRAPH_NODE_COUNT_OFFSET)? as usize;
    let graph_edges = read_u16_at(bytes, payload + COMPACT_GRAPH_EDGE_COUNT_OFFSET)? as usize;
    let graph_offset = compact_graph_records_offset(bytes)?;
    let graph_len = graph_nodes
        .checked_mul(GRAPH_NODE_RECORD_LEN)
        .and_then(|len| len.checked_add(graph_edges.checked_mul(GRAPH_EDGE_RECORD_LEN)?))
        .ok_or_else(|| "KrustBoot graph store length overflow".to_owned())?;
    let graph_end = graph_offset
        .checked_add(graph_len)
        .ok_or_else(|| "KrustBoot graph store end overflow".to_owned())?;
    if graph_end > bytes.len() {
        return Err("KrustBoot graph store is out of bounds".to_owned());
    }
    let edge_base = graph_offset
        .checked_add(
            graph_nodes
                .checked_mul(GRAPH_NODE_RECORD_LEN)
                .ok_or_else(|| "KrustBoot graph node length overflow".to_owned())?,
        )
        .ok_or_else(|| "KrustBoot graph edge offset overflow".to_owned())?;
    let edge_id = format!("grant:{grant_index}");
    let mut edge_index = 0;
    while edge_index < graph_edges {
        let offset = edge_base
            .checked_add(
                edge_index
                    .checked_mul(GRAPH_EDGE_RECORD_LEN)
                    .ok_or_else(|| "KrustBoot graph edge offset overflow".to_owned())?,
            )
            .ok_or_else(|| "KrustBoot graph edge offset overflow".to_owned())?;
        if offset + GRAPH_EDGE_RECORD_LEN > bytes.len() {
            return Err("KrustBoot graph edge is out of bounds".to_owned());
        }
        if fixed_str_equals(bytes, offset + 8, &edge_id) {
            bytes[offset + 6..offset + 8].copy_from_slice(&rights.to_le_bytes());
            let graph_checksum = checksum32(&bytes[graph_offset..graph_end]);
            let checksum_offset = payload + COMPACT_GRAPH_CHECKSUM_OFFSET;
            bytes[checksum_offset..checksum_offset + 4]
                .copy_from_slice(&graph_checksum.to_le_bytes());
            return Ok(());
        }
        edge_index += 1;
    }
    Err(format!(
        "KrustBoot graph store lacks edge grant:{grant_index}"
    ))
}

fn corrupt_graph_store_record(bytes: &mut [u8]) -> Result<(), String> {
    let payload = V1_PAYLOAD_OFFSET;
    if bytes.len() < payload + COMPACT_HEADER_SIZE {
        return Err("KrustBoot manifest is too short to corrupt graph store".to_owned());
    }
    let graph_nodes = read_u16_at(bytes, payload + COMPACT_GRAPH_NODE_COUNT_OFFSET)? as usize;
    let graph_edges = read_u16_at(bytes, payload + COMPACT_GRAPH_EDGE_COUNT_OFFSET)? as usize;
    if graph_nodes == 0 {
        return Err("KrustBoot graph store has no node record to corrupt".to_owned());
    }
    let graph_offset = compact_graph_records_offset(bytes)?;
    let graph_len = graph_nodes
        .checked_mul(GRAPH_NODE_RECORD_LEN)
        .and_then(|len| len.checked_add(graph_edges.checked_mul(GRAPH_EDGE_RECORD_LEN)?))
        .ok_or_else(|| "KrustBoot graph store length overflow".to_owned())?;
    let graph_end = graph_offset
        .checked_add(graph_len)
        .ok_or_else(|| "KrustBoot graph store end overflow".to_owned())?;
    if graph_end > bytes.len() || graph_offset + 2 > bytes.len() {
        return Err("KrustBoot graph store is out of bounds".to_owned());
    }
    bytes[graph_offset..graph_offset + 2].copy_from_slice(&0u16.to_le_bytes());
    let graph_checksum = checksum32(&bytes[graph_offset..graph_end]);
    let checksum_offset = payload + COMPACT_GRAPH_CHECKSUM_OFFSET;
    bytes[checksum_offset..checksum_offset + 4].copy_from_slice(&graph_checksum.to_le_bytes());
    Ok(())
}

fn compact_graph_records_offset(bytes: &[u8]) -> Result<usize, String> {
    let (offset, vfs_roots) = compact_vfs_roots_table(bytes)?;
    checked_advance(offset, vfs_roots, VFS_ROOT_RECORD_LEN)
}

fn compact_vfs_roots_offset(bytes: &[u8]) -> Result<usize, String> {
    compact_vfs_roots_table(bytes).map(|(offset, _)| offset)
}

fn compact_vfs_roots_table(bytes: &[u8]) -> Result<(usize, usize), String> {
    let payload = V1_PAYLOAD_OFFSET;
    if bytes.len() < payload + COMPACT_HEADER_SIZE {
        return Err("KrustBoot manifest is too short for compact header".to_owned());
    }
    let boot_modules = read_u16_at(bytes, payload + 18)? as usize;
    let processes = read_u16_at(bytes, payload + 20)? as usize;
    let endpoints = read_u16_at(bytes, payload + 22)? as usize;
    let grants = read_u16_at(bytes, payload + 24)? as usize;
    let store_objects = read_u16_at(bytes, payload + 26)? as usize;
    let state_volumes = read_u16_at(bytes, payload + 28)? as usize;
    let network_ports = read_u16_at(bytes, payload + 30)? as usize;
    let io_ports = read_u16_at(bytes, payload + 32)? as usize;
    let mmio_regions = read_u16_at(bytes, payload + 34)? as usize;
    let framebuffers = read_u16_at(bytes, payload + 36)? as usize;
    let interrupt_lines = read_u16_at(bytes, payload + 38)? as usize;
    let dma_regions = read_u16_at(bytes, payload + 40)? as usize;
    let pci_devices = read_u16_at(bytes, payload + 42)? as usize;
    let virtio_devices = read_u16_at(bytes, payload + 44)? as usize;
    let namespaces = read_u16_at(bytes, payload + 46)? as usize;
    let vfs_roots = read_u16_at(bytes, payload + 48)? as usize;

    let mut offset = payload + COMPACT_HEADER_SIZE;
    offset = checked_advance(offset, boot_modules, BOOT_MODULE_RECORD_LEN)?;
    offset = checked_advance(offset, processes, PROCESS_RECORD_LEN)?;
    offset = checked_advance(offset, endpoints, ENDPOINT_RECORD_LEN)?;
    offset = checked_advance(offset, grants, GRANT_RECORD_LEN)?;
    offset = checked_advance(offset, store_objects, STORE_OBJECT_RECORD_LEN)?;
    offset = checked_advance(offset, state_volumes, STATE_VOLUME_RECORD_LEN)?;
    offset = checked_advance(offset, network_ports, NETWORK_PORT_RECORD_LEN)?;
    offset = checked_advance(offset, io_ports, IO_PORT_RECORD_LEN)?;
    offset = checked_advance(offset, mmio_regions, MMIO_REGION_RECORD_LEN)?;
    offset = checked_advance(offset, framebuffers, FRAMEBUFFER_RECORD_LEN)?;
    offset = checked_advance(offset, interrupt_lines, INTERRUPT_LINE_RECORD_LEN)?;
    offset = checked_advance(offset, dma_regions, DMA_REGION_RECORD_LEN)?;
    offset = checked_advance(offset, pci_devices, PCI_DEVICE_RECORD_LEN)?;
    offset = checked_advance(offset, virtio_devices, VIRTIO_DEVICE_RECORD_LEN)?;
    let mut namespace_index = 0;
    while namespace_index < namespaces {
        if offset + STRING_LEN + 2 > bytes.len() {
            return Err("KrustBoot namespace records are out of bounds".to_owned());
        }
        let entry_count = read_u16_at(bytes, offset + STRING_LEN)? as usize;
        offset = offset
            .checked_add(STRING_LEN + 2)
            .and_then(|offset| {
                offset.checked_add(entry_count.checked_mul(NAMESPACE_ENTRY_RECORD_LEN)?)
            })
            .ok_or_else(|| "KrustBoot namespace record length overflow".to_owned())?;
        namespace_index += 1;
    }
    Ok((offset, vfs_roots))
}

fn compact_grants_offset(bytes: &[u8]) -> Result<usize, String> {
    let payload = V1_PAYLOAD_OFFSET;
    if bytes.len() < payload + COMPACT_HEADER_SIZE {
        return Err("KrustBoot manifest is too short for compact header".to_owned());
    }
    let boot_modules = read_u16_at(bytes, payload + 18)? as usize;
    let processes = read_u16_at(bytes, payload + 20)? as usize;
    let endpoints = read_u16_at(bytes, payload + 22)? as usize;

    let mut offset = payload + COMPACT_HEADER_SIZE;
    offset = checked_advance(offset, boot_modules, BOOT_MODULE_RECORD_LEN)?;
    offset = checked_advance(offset, processes, PROCESS_RECORD_LEN)?;
    checked_advance(offset, endpoints, ENDPOINT_RECORD_LEN)
}

fn compact_policy_header_offset(bytes: &[u8]) -> Result<usize, String> {
    let payload = V1_PAYLOAD_OFFSET;
    let graph_nodes = read_u16_at(bytes, payload + COMPACT_GRAPH_NODE_COUNT_OFFSET)? as usize;
    let graph_edges = read_u16_at(bytes, payload + COMPACT_GRAPH_EDGE_COUNT_OFFSET)? as usize;
    let graph_offset = compact_graph_records_offset(bytes)?;
    graph_offset
        .checked_add(
            graph_nodes
                .checked_mul(GRAPH_NODE_RECORD_LEN)
                .and_then(|len| len.checked_add(graph_edges.checked_mul(GRAPH_EDGE_RECORD_LEN)?))
                .ok_or_else(|| "KrustBoot graph store length overflow".to_owned())?,
        )
        .ok_or_else(|| "KrustBoot policy header offset overflow".to_owned())
}

fn checked_advance(offset: usize, count: usize, record_len: usize) -> Result<usize, String> {
    offset
        .checked_add(
            count
                .checked_mul(record_len)
                .ok_or_else(|| "KrustBoot compact section length overflow".to_owned())?,
        )
        .ok_or_else(|| "KrustBoot compact section offset overflow".to_owned())
}

fn fixed_str_equals(bytes: &[u8], offset: usize, value: &str) -> bool {
    if offset + STRING_LEN > bytes.len() || value.len() > STRING_LEN {
        return false;
    }
    let value = value.as_bytes();
    let mut index = 0;
    while index < value.len() {
        if bytes[offset + index] != value[index] {
            return false;
        }
        index += 1;
    }
    value.len() == STRING_LEN || bytes[offset + value.len()] == 0
}

fn read_u16_at(bytes: &[u8], offset: usize) -> Result<u16, String> {
    if offset + 2 > bytes.len() {
        return Err("KrustBoot manifest is too short for u16 read".to_owned());
    }
    Ok(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
}

fn read_u32_at(bytes: &[u8], offset: usize) -> Result<u32, String> {
    if offset + 4 > bytes.len() {
        return Err("KrustBoot manifest is too short for u32 read".to_owned());
    }
    Ok(u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}

fn v1_checksum(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for (index, byte) in bytes.iter().copied().enumerate() {
        let value = if (V1_CHECKSUM_OFFSET..V1_CHECKSUM_OFFSET + 4).contains(&index) {
            0
        } else {
            byte
        };
        hash ^= value as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

fn push_fixed_str(bytes: &mut Vec<u8>, value: &str) -> Result<(), String> {
    if value.len() > STRING_LEN {
        return Err(format!(
            "KrustBoot string is too long: {value} is {} bytes, max {}",
            value.len(),
            STRING_LEN
        ));
    }
    if !value.is_ascii() {
        return Err(format!("KrustBoot string must be ASCII: {value}"));
    }

    let mut slot = [0u8; STRING_LEN];
    slot[..value.len()].copy_from_slice(value.as_bytes());
    bytes.extend_from_slice(&slot);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_plan() -> BootPlan {
        BootPlan {
            boot_modules: vec![BootModule {
                name: "init".to_owned(),
                module_string: "vertex-init".to_owned(),
            }],
            processes: vec![NativeProcess {
                name: "vertex-init".to_owned(),
                module_string: "vertex-init".to_owned(),
                initial: true,
                service_id: "svc:vertex-supervisor".to_owned(),
                start_after: Vec::new(),
                requires_endpoints: Vec::new(),
                provides_endpoints: Vec::new(),
                health_kind: String::new(),
                mount_root: "/".to_owned(),
                mounts: Vec::new(),
                restart: RESTART_NEVER,
            }],
            endpoints: vec![Endpoint {
                name: "serial-log".to_owned(),
            }],
            grants: vec![
                Grant {
                    process: "vertex-init".to_owned(),
                    object_kind: OBJECT_ENDPOINT,
                    object_name: "serial-log".to_owned(),
                    cap_slot: 1,
                    rights: RIGHT_SEND,
                },
                Grant {
                    process: "vertex-init".to_owned(),
                    object_kind: OBJECT_ENDPOINT,
                    object_name: "serial-log".to_owned(),
                    cap_slot: 1,
                    rights: RIGHT_RECEIVE,
                },
            ],
            store_objects: Vec::new(),
            state_volumes: Vec::new(),
            network_ports: Vec::new(),
            io_ports: Vec::new(),
            mmio_regions: Vec::new(),
            framebuffers: Vec::new(),
            interrupt_lines: Vec::new(),
            dma_regions: Vec::new(),
            pci_devices: Vec::new(),
            virtio_devices: Vec::new(),
            namespaces: Vec::new(),
            vfs_roots: Vec::new(),
            graph_nodes: Vec::new(),
            graph_edges: Vec::new(),
            policy_capabilities: Vec::new(),
            policy_requirements: Vec::new(),
            policy_provides: Vec::new(),
            policy_mounts: Vec::new(),
            policy_state_paths: Vec::new(),
            policy_bootstraps: Vec::new(),
        }
    }

    #[test]
    fn validate_boot_plan_rejects_duplicate_process_cap_slots() {
        let mut plan = minimal_plan();
        plan.grants = vec![
            Grant {
                process: "vertex-init".to_owned(),
                object_kind: OBJECT_ENDPOINT,
                object_name: "serial-log".to_owned(),
                cap_slot: 1,
                rights: RIGHT_SEND,
            },
            Grant {
                process: "vertex-init".to_owned(),
                object_kind: OBJECT_ENDPOINT,
                object_name: "serial-log".to_owned(),
                cap_slot: 1,
                rights: RIGHT_RECEIVE,
            },
        ];

        let error = validate_plan(&plan).expect_err("duplicate cap slot should fail");
        assert!(error.contains("duplicate grant cap slot 1 for process vertex-init"));
    }

    #[test]
    fn validate_boot_plan_rejects_overlapping_dma_regions() {
        let mut plan = minimal_plan();
        plan.dma_regions = vec![
            DmaRegion {
                id: "cap:dma.a".to_owned(),
                base: 0x200000,
                length: PAGE_SIZE * 2,
            },
            DmaRegion {
                id: "cap:dma.b".to_owned(),
                base: 0x201000,
                length: PAGE_SIZE,
            },
        ];

        let error = validate_plan(&plan).expect_err("overlapping dma should fail");
        assert!(error.contains("dma-region capability cap:dma.b overlaps cap:dma.a"));
    }

    #[test]
    fn validate_boot_plan_rejects_duplicate_irq_lines() {
        let mut plan = minimal_plan();
        plan.interrupt_lines = vec![
            InterruptLine {
                id: "cap:irq.a".to_owned(),
                line: 11,
            },
            InterruptLine {
                id: "cap:irq.b".to_owned(),
                line: 11,
            },
        ];

        let error = validate_plan(&plan).expect_err("duplicate irq should fail");
        assert!(error.contains("duplicate interrupt-line ownership for legacy IRQ 11"));
    }

    #[test]
    fn endpoint_target_slots_do_not_overlap_provider_slot() {
        assert_eq!(endpoint_target_slot(0, 0), SERVICE_CAP_SLOT);
        assert_eq!(endpoint_target_slot(0, 1), READINESS_RESERVED_CAP_SLOT + 1);
        assert_eq!(endpoint_target_slot(1, 0), READINESS_RESERVED_CAP_SLOT + 1);
        assert_eq!(endpoint_target_slot(2, 0), READINESS_RESERVED_CAP_SLOT + 2);
        assert_eq!(
            first_non_endpoint_service_slot(1, 1),
            READINESS_RESERVED_CAP_SLOT + 2
        );
        assert_eq!(
            first_non_endpoint_service_slot(2, 1),
            READINESS_RESERVED_CAP_SLOT + 3
        );
    }

    #[test]
    fn missing_provider_corruption_zeros_current_provides_list() {
        let process_base = V1_PAYLOAD_OFFSET + COMPACT_HEADER_SIZE + BOOT_MODULE_RECORD_LEN;
        let current_offset = process_base + PROCESS_PROVIDES_COUNT_OFFSET;
        let stale_offset = process_base
            + STRING_LEN * 5
            + 4
            + PROCESS_REF_LIST_LEN
            + ENDPOINT_REQUIREMENT_LIST_LEN;
        let mut bytes = vec![0; process_base + PROCESS_RECORD_LEN];
        let payload = V1_PAYLOAD_OFFSET;
        bytes[payload + 18..payload + 20].copy_from_slice(&1u16.to_le_bytes());
        bytes[payload + 20..payload + 22].copy_from_slice(&1u16.to_le_bytes());
        bytes[current_offset..current_offset + 2].copy_from_slice(&1u16.to_le_bytes());
        bytes[stale_offset..stale_offset + 2].copy_from_slice(&0xaaaa_u16.to_le_bytes());

        corrupt_missing_provider(&mut bytes).expect("missing-provider corruption should apply");

        assert_eq!(
            &bytes[current_offset..current_offset + 2],
            &0u16.to_le_bytes()
        );
        assert_eq!(
            &bytes[stale_offset..stale_offset + 2],
            &0xaaaa_u16.to_le_bytes()
        );
    }
}
