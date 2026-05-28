use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use vertex_ir::{GenerationManifest, Service};

const COMPACT_MAGIC: &[u8; 16] = b"KRUSTBOOTM60\0\0\0\0";
const COMPACT_VERSION: u16 = 6;
const V1_MAGIC: &[u8; 16] = b"KRUSTBOOTV1\0\0\0\0\0";
const V1_VERSION: u16 = 1;
const V1_HEADER_SIZE: usize = 164;
const V1_CHECKSUM_OFFSET: usize = 32;
const V1_RECORD_SIZE: usize = 12;
const V1_RECORD_COUNT: usize = 9;
const V1_PAYLOAD_OFFSET: usize = V1_HEADER_SIZE + V1_RECORD_COUNT * V1_RECORD_SIZE;
const COMPACT_HEADER_SIZE: usize = 174;
const STRING_LEN: usize = 64;
const BOOT_MODULE_RECORD_LEN: usize = STRING_LEN * 2;
const PROCESS_REF_LIST_LEN: usize = 2 + MAX_PROCESS_REFS * 2;
const ENDPOINT_REQUIREMENT_LIST_LEN: usize = 2 + MAX_PROCESS_REFS * 4;
const PROCESS_RECORD_LEN: usize =
    STRING_LEN * 4 + 4 + PROCESS_REF_LIST_LEN * 2 + ENDPOINT_REQUIREMENT_LIST_LEN;
const ENDPOINT_RECORD_LEN: usize = STRING_LEN;
const GRANT_RECORD_LEN: usize = 12;
const STORE_OBJECT_RECORD_LEN: usize = STRING_LEN * 3 + 8;
const STATE_VOLUME_RECORD_LEN: usize = STRING_LEN;
const NETWORK_PORT_RECORD_LEN: usize = STRING_LEN;
const IO_PORT_RECORD_LEN: usize = STRING_LEN + 16;
const MMIO_REGION_RECORD_LEN: usize = STRING_LEN + 16;
const INTERRUPT_LINE_RECORD_LEN: usize = STRING_LEN + 8;
const DMA_REGION_RECORD_LEN: usize = STRING_LEN + 16;
const PCI_DEVICE_RECORD_LEN: usize = STRING_LEN * 2;
const VIRTIO_DEVICE_RECORD_LEN: usize = STRING_LEN * 2;
const NAMESPACE_ENTRY_RECORD_LEN: usize = STRING_LEN + 8;
const MAX_BOOT_MODULES: usize = 16;
const MAX_PROCESSES: usize = 16;
const MAX_ENDPOINTS: usize = 16;
const MAX_GRANTS: usize = 96;
const MAX_STORE_OBJECTS: usize = 32;
const MAX_STATE_VOLUMES: usize = 4;
const MAX_NETWORK_PORTS: usize = 4;
const MAX_IO_PORT_RANGES: usize = 4;
const MAX_MMIO_REGIONS: usize = 4;
const MAX_INTERRUPT_LINES: usize = 4;
const MAX_DMA_REGIONS: usize = 4;
const MAX_PCI_DEVICES: usize = 4;
const MAX_VIRTIO_DEVICES: usize = 4;
const MAX_NAMESPACES: usize = 4;
const MAX_NAMESPACE_ENTRIES: usize = 4;
const MAX_PROCESS_REFS: usize = 4;
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
const RECORD_BOOT_MODULE: u16 = 1;
const RECORD_PROCESS: u16 = 2;
const RECORD_ENDPOINT: u16 = 3;
const RECORD_GRANT: u16 = 4;
const RECORD_STORE_OBJECT: u16 = 5;
const RECORD_STATE_VOLUME: u16 = 6;
const RECORD_TIMER: u16 = 7;
const RECORD_GENERATION: u16 = 8;
const RECORD_POLICY: u16 = 9;
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
const LOGD_CONFIG_MODULE: &str = "config-logd-v0";
const LOGD_CONFIG_BYTES: &[u8] = b"{\"level\":\"info\",\"sink\":\"serial\"}\n";

pub fn compile(manifest: &GenerationManifest) -> Result<Vec<u8>, String> {
    let plan = derive_plan(manifest)?;
    validate_plan(&plan)?;

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
    push_count(&mut body, plan.interrupt_lines.len(), "interrupt_lines")?;
    push_count(&mut body, plan.dma_regions.len(), "dma_regions")?;
    push_count(&mut body, plan.pci_devices.len(), "pci_devices")?;
    push_count(&mut body, plan.virtio_devices.len(), "virtio_devices")?;
    push_count(&mut body, plan.namespaces.len(), "namespaces")?;
    push_fixed_str(&mut body, &manifest.generation.id)?;
    push_fixed_str(
        &mut body,
        manifest.generation.parent.as_deref().unwrap_or_default(),
    )?;

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
         interrupt_lines: {}\n\
         dma_regions: {}\n\
         pci_devices: {}\n\
         virtio_devices: {}\n\
         namespaces: {}\n\
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
        plan.interrupt_lines.len(),
        plan.dma_regions.len(),
        plan.pci_devices.len(),
        plan.virtio_devices.len(),
        plan.namespaces.len()
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
        "missing-provider" => {
            corrupt_missing_provider(&mut out)?;
            rewrite_v1_checksum(&mut out)?;
        }
        other => {
            return Err(format!(
                "unknown KrustBoot corruption mode {other}; expected truncated, bad-magic, unsupported-version, out-of-bounds-record, raw-compact, or missing-provider"
            ));
        }
    }
    Ok(out)
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
    interrupt_lines: Vec<InterruptLine>,
    dma_regions: Vec<DmaRegion>,
    pci_devices: Vec<PciDevice>,
    virtio_devices: Vec<VirtioDevice>,
    namespaces: Vec<Namespace>,
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
    restart: u16,
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
    let state_volumes = Vec::new();
    let mut network_ports = Vec::new();
    let mut io_ports = Vec::new();
    let mut mmio_regions = Vec::new();
    let mut interrupt_lines = Vec::new();
    let mut dma_regions = Vec::new();
    let mut pci_devices = Vec::new();
    let mut virtio_devices = Vec::new();
    let mut namespaces = Vec::new();
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
                    grants.push(Grant {
                        process: process_name.clone(),
                        object_kind: OBJECT_STORE,
                        object_name: store.id.clone(),
                        cap_slot: next_object_cap_slot(&mut next_object_slots, &process_name)?,
                        rights: rights_mask(
                            &requirement.rights,
                            &capability.rights,
                            &capability.id,
                        )?,
                    });
                }
                "state-volume" => {
                    return Err(format!(
                        "native KrustBoot uses vertexdisk-v0 state service IPC; legacy state backend capability {} required by {} is not supported",
                        capability.id, service.id
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

    Ok(BootPlan {
        boot_modules,
        processes,
        endpoints,
        grants,
        store_objects,
        state_volumes,
        network_ports,
        io_ports,
        mmio_regions,
        interrupt_lines,
        dma_regions,
        pci_devices,
        virtio_devices,
        namespaces,
    })
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

    let crate_dir = match module_string {
        "vertex-init" => "init",
        other => other,
    };
    paths.push(PathBuf::from(format!(
        "user/{crate_dir}/target/x86_64-unknown-none/debug/{module_string}"
    )));
    paths.push(PathBuf::from(format!(
        "kernel/krust/user/{crate_dir}/target/x86_64-unknown-none/debug/{module_string}"
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
        parsed.push(NamespaceEntry {
            path: path.to_owned(),
            object_kind: object_kind_for_capability(target)?,
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

fn object_kind_for_capability(capability: &vertex_ir::Capability) -> Result<u16, String> {
    match capability.kind.as_str() {
        "ipc-endpoint" => Ok(OBJECT_ENDPOINT),
        "store-object" => Ok(OBJECT_STORE),
        "timer" => Ok(OBJECT_TIMER),
        "network-port" => Ok(OBJECT_NETWORK_PORT),
        "io-port" => Ok(OBJECT_IO_PORT_RANGE),
        "mmio-region" => Ok(OBJECT_MMIO_REGION),
        "interrupt-line" => Ok(OBJECT_INTERRUPT_LINE),
        "dma-region" => Ok(OBJECT_DMA_REGION),
        "virtio-device" => Ok(OBJECT_VIRTIO_DEVICE),
        "namespace" => Ok(OBJECT_NAMESPACE),
        other => Err(format!(
            "namespace entries cannot resolve capability kind {other}"
        )),
    }
}

fn object_name_for_capability(capability: &vertex_ir::Capability) -> String {
    match capability.kind.as_str() {
        "timer" => "monotonic-timer".to_owned(),
        "store-object" | "io-port" | "mmio-region" | "interrupt-line" | "dma-region" => {
            capability.id.clone()
        }
        "ipc-endpoint" => endpoint_name(&capability.id),
        "network-port" | "virtio-device" | "namespace" => capability.id.clone(),
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
    let rights = if required.is_empty() {
        capability
    } else {
        required
    };

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
            other => return Err(format!("unsupported native right {other} for {context}")),
        }
    }

    if mask == 0 {
        return Err(format!("capability {context} has no native rights"));
    }

    Ok(mask)
}

fn service_label(service: &Service) -> String {
    if !service.name.is_empty() {
        service.name.clone()
    } else {
        module_basename(&service.id)
    }
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
        OBJECT_INTERRUPT_LINE => interrupt_line_index(plan, object_name),
        OBJECT_DMA_REGION => dma_region_index(plan, object_name),
        OBJECT_PCI_DEVICE => pci_device_index(plan, object_name),
        OBJECT_VIRTIO_DEVICE => virtio_device_index(plan, object_name),
        OBJECT_NAMESPACE => namespace_index(plan, object_name),
        other => Err(format!("unsupported native object kind {other}")),
    }
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
    interrupt_lines: (usize, usize),
    dma_regions: (usize, usize),
    pci_devices: (usize, usize),
    virtio_devices: (usize, usize),
    namespaces: (usize, usize),
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
            + sections.interrupt_lines.1
            + sections.dma_regions.1
            + sections.pci_devices.1
            + sections.virtio_devices.1
            + sections.namespaces.1,
    )?;
    debug_assert_eq!(bytes.len(), payload_offset);

    bytes.extend_from_slice(body);
    rewrite_v1_checksum(&mut bytes)?;
    Ok(bytes)
}

impl BodySections {
    fn new(plan: &BootPlan) -> Self {
        let generation = (46, STRING_LEN * 2);
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
        let interrupt_lines = (
            mmio_regions.0 + mmio_regions.1,
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
            interrupt_lines,
            dma_regions,
            pci_devices,
            virtio_devices,
            namespaces,
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
    let provides_count_offset =
        STRING_LEN * 4 + 4 + PROCESS_REF_LIST_LEN + ENDPOINT_REQUIREMENT_LIST_LEN;
    let mut index = 0;
    while index < processes {
        let offset = process_base + index * PROCESS_RECORD_LEN + provides_count_offset;
        if offset + 2 > bytes.len() {
            return Err("KrustBoot process record is out of bounds".to_owned());
        }
        bytes[offset..offset + 2].copy_from_slice(&0u16.to_le_bytes());
        index += 1;
    }
    Ok(())
}

fn read_u16_at(bytes: &[u8], offset: usize) -> Result<u16, String> {
    if offset + 2 > bytes.len() {
        return Err("KrustBoot manifest is too short for u16 read".to_owned());
    }
    Ok(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
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

    #[test]
    fn validate_boot_plan_rejects_duplicate_process_cap_slots() {
        let plan = BootPlan {
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
            interrupt_lines: Vec::new(),
            dma_regions: Vec::new(),
            pci_devices: Vec::new(),
            virtio_devices: Vec::new(),
        };

        let error = validate_plan(&plan).expect_err("duplicate cap slot should fail");
        assert!(error.contains("duplicate grant cap slot 1 for process vertex-init"));
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
}
