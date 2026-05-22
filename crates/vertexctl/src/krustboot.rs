use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use vertex_ir::{GenerationManifest, Service};

const COMPACT_MAGIC: &[u8; 16] = b"KRUSTBOOTV0\0\0\0\0\0";
const COMPACT_VERSION: u16 = 4;
const V1_MAGIC: &[u8; 16] = b"KRUSTBOOTV1\0\0\0\0\0";
const V1_VERSION: u16 = 1;
const V1_HEADER_SIZE: usize = 164;
const V1_CHECKSUM_OFFSET: usize = 32;
const V1_RECORD_SIZE: usize = 12;
const V1_RECORD_COUNT: usize = 9;
const V1_PAYLOAD_OFFSET: usize = V1_HEADER_SIZE + V1_RECORD_COUNT * V1_RECORD_SIZE;
const COMPACT_HEADER_SIZE: usize = 168;
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
const MAX_BOOT_MODULES: usize = 16;
const MAX_PROCESSES: usize = 16;
const MAX_ENDPOINTS: usize = 16;
const MAX_GRANTS: usize = 64;
const MAX_STORE_OBJECTS: usize = 4;
const MAX_STATE_VOLUMES: usize = 4;
const MAX_NETWORK_PORTS: usize = 4;
const MAX_IO_PORT_RANGES: usize = 4;
const MAX_MMIO_REGIONS: usize = 4;
const MAX_INTERRUPT_LINES: usize = 4;
const MAX_DMA_REGIONS: usize = 4;
const MAX_PROCESS_REFS: usize = 4;
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
const OBJECT_ENDPOINT: u16 = 1;
const OBJECT_STORE: u16 = 2;
const OBJECT_STATE: u16 = 3;
const OBJECT_TIMER: u16 = 4;
const OBJECT_NETWORK_PORT: u16 = 5;
const OBJECT_IO_PORT_RANGE: u16 = 6;
const OBJECT_MMIO_REGION: u16 = 7;
const OBJECT_INTERRUPT_LINE: u16 = 8;
const OBJECT_DMA_REGION: u16 = 9;
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
        plan.dma_regions.len()
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
    let init_name = module_basename(&init_executable.entrypoint);
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
        processes.push(NativeProcess {
            name: process_name,
            module_string: module_basename(&executable.entrypoint),
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
        let provider_rights = RIGHT_RECEIVE
            | (endpoint_rights_mask(&[], &capability.rights, &capability.id)? & RIGHT_SEND);

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
            cap_slot: SERVICE_CAP_SLOT,
            rights: provider_rights,
        });

        grants.push(Grant {
            process: init_name.clone(),
            object_kind: OBJECT_ENDPOINT,
            object_name: endpoint.clone(),
            cap_slot: init_endpoint_auth_slot(&endpoints, &endpoint)?,
            rights: RIGHT_SEND | RIGHT_RECEIVE,
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
                add_process_start_after(&mut processes, &service.id, &capability.provider)?;
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
            if manifest.service(&capability.provider).is_some() {
                add_process_start_after(&mut processes, &service.id, &capability.provider)?;
            }
        }
    }

    let mut store_objects = Vec::new();
    let mut state_volumes = Vec::new();
    let mut network_ports = Vec::new();
    let mut io_ports = Vec::new();
    let mut mmio_regions = Vec::new();
    let mut interrupt_lines = Vec::new();
    let mut dma_regions = Vec::new();
    let mut next_object_slots = initial_object_cap_slots(&processes);
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
                    push_unique_store_object(&mut store_objects, store);
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
                    let state = manifest.state_volume(&capability.provider).ok_or_else(|| {
                        format!(
                            "capability {} references unknown state volume {}",
                            capability.id, capability.provider
                        )
                    })?;
                    push_unique_state_volume(&mut state_volumes, state.id.clone());
                    grants.push(Grant {
                        process: process_name.clone(),
                        object_kind: OBJECT_STATE,
                        object_name: state.id.clone(),
                        cap_slot: next_object_cap_slot(&mut next_object_slots, &process_name)?,
                        rights: rights_mask(
                            &requirement.rights,
                            &capability.rights,
                            &capability.id,
                        )?,
                    });
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
    }

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

fn push_unique_store_object(objects: &mut Vec<StoreObject>, store: &vertex_ir::StoreObject) {
    if objects.iter().any(|object| object.id == store.id) {
        return;
    }

    objects.push(StoreObject {
        id: store.id.clone(),
        module_string: store.name.clone(),
        hash: store.hash.clone(),
        size: store.size_bytes,
    });
}

fn push_unique_state_volume(states: &mut Vec<StateVolume>, state_id: String) {
    if !states.iter().any(|state| state.id == state_id) {
        states.push(StateVolume { id: state_id });
    }
}

fn push_unique_network_port(ports: &mut Vec<NetworkPort>, port_id: String) {
    if !ports.iter().any(|port| port.id == port_id) {
        ports.push(NetworkPort { id: port_id });
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
    ports.push(IoPortRange {
        id: capability.id.clone(),
        base: value_u64(&capability.properties, "base")
            .or_else(|| value_u64(&capability.properties, "portBase"))
            .or_else(|| value_u64(&capability.properties, "port"))
            .or_else(|| value_u64(&device.selector, "base"))
            .or_else(|| value_u64(&device.selector, "portBase"))
            .or_else(|| value_u64(&device.selector, "port"))
            .ok_or_else(|| format!("io-port capability {} missing base port", capability.id))?,
        length: value_u64(&capability.properties, "length")
            .or_else(|| value_u64(&capability.properties, "ports"))
            .or_else(|| value_u64(&device.selector, "length"))
            .or_else(|| value_u64(&device.selector, "ports"))
            .unwrap_or(1),
    });
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
    regions.push(DmaRegion {
        id: capability.id.clone(),
        base: value_u64(&capability.properties, "base")
            .or_else(|| value_u64(&device.selector, "base"))
            .unwrap_or(0),
        length: value_u64(&capability.properties, "length")
            .or_else(|| value_u64(&device.selector, "length"))
            .unwrap_or(4096),
    });
    Ok(())
}

fn value_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(parse_u64_literal))
    })
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
                    !process.provides_endpoints.is_empty(),
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

fn first_non_endpoint_service_slot(has_provided_endpoint: bool, endpoint_count: usize) -> u16 {
    let mut slot = if endpoint_count == 0 && !has_provided_endpoint {
        SERVICE_CAP_SLOT
    } else {
        let last_required_slot = if endpoint_count == 0 {
            SERVICE_CAP_SLOT
        } else {
            endpoint_target_slot(has_provided_endpoint, endpoint_count - 1)
        };
        last_required_slot + 1
    };
    if slot <= READINESS_RESERVED_CAP_SLOT {
        slot = READINESS_RESERVED_CAP_SLOT + 1;
    }
    slot
}

fn next_object_cap_slot(slots: &mut BTreeMap<String, u16>, process: &str) -> Result<u16, String> {
    let slot = slots
        .get_mut(process)
        .ok_or_else(|| format!("unknown process {process} for cap slot allocation"))?;
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
            if requirement.rights == 0 || requirement.rights & !(RIGHT_SEND | RIGHT_RECEIVE) != 0 {
                return Err(format!(
                    "process {} has invalid endpoint requirement rights",
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

    for grant in &plan.grants {
        process_index(plan, &grant.process)?;
        object_index(plan, grant)?;
        if grant.rights == 0 {
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
    if mask & !(RIGHT_SEND | RIGHT_RECEIVE) != 0 {
        return Err(format!(
            "ipc endpoint {context} requires non-endpoint native rights"
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
            "sendrecv" => mask |= RIGHT_SEND | RIGHT_RECEIVE,
            "read" => mask |= RIGHT_READ,
            "write" => mask |= RIGHT_WRITE,
            "readwrite" | "read-write" => mask |= RIGHT_READ | RIGHT_WRITE,
            "snapshot" => mask |= RIGHT_SNAPSHOT,
            "restore" => mask |= RIGHT_RESTORE,
            "control" => mask |= RIGHT_CONTROL,
            "bind" => mask |= RIGHT_BIND,
            "listen" => mask |= RIGHT_LISTEN,
            "map" => mask |= RIGHT_MAP,
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

fn endpoint_target_slot(has_provided_endpoint: bool, requirement_index: usize) -> u16 {
    if !has_provided_endpoint && requirement_index == 0 {
        SERVICE_CAP_SLOT
    } else if has_provided_endpoint {
        READINESS_RESERVED_CAP_SLOT + 1 + requirement_index as u16
    } else {
        READINESS_RESERVED_CAP_SLOT + requirement_index as u16
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

fn object_index(plan: &BootPlan, grant: &Grant) -> Result<usize, String> {
    match grant.object_kind {
        OBJECT_ENDPOINT => endpoint_index(plan, &grant.object_name),
        OBJECT_STORE => store_index(plan, &grant.object_name),
        OBJECT_STATE => state_index(plan, &grant.object_name),
        OBJECT_TIMER if grant.object_name == "monotonic-timer" => Ok(0),
        OBJECT_NETWORK_PORT => network_port_index(plan, &grant.object_name),
        OBJECT_IO_PORT_RANGE => io_port_index(plan, &grant.object_name),
        OBJECT_MMIO_REGION => mmio_region_index(plan, &grant.object_name),
        OBJECT_INTERRUPT_LINE => interrupt_line_index(plan, &grant.object_name),
        OBJECT_DMA_REGION => dma_region_index(plan, &grant.object_name),
        other => Err(format!("unsupported native object kind {other}")),
    }
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
            + sections.dma_regions.1,
    )?;
    debug_assert_eq!(bytes.len(), payload_offset);

    bytes.extend_from_slice(body);
    rewrite_v1_checksum(&mut bytes)?;
    Ok(bytes)
}

impl BodySections {
    fn new(plan: &BootPlan) -> Self {
        let generation = (40, STRING_LEN * 2);
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
    if value.len() >= STRING_LEN {
        return Err(format!(
            "KrustBoot string is too long: {value} is {} bytes, max {}",
            value.len(),
            STRING_LEN - 1
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
        };

        let error = validate_plan(&plan).expect_err("duplicate cap slot should fail");
        assert!(error.contains("duplicate grant cap slot 1 for process vertex-init"));
    }

    #[test]
    fn endpoint_target_slots_do_not_overlap_provider_slot() {
        assert_eq!(endpoint_target_slot(false, 0), SERVICE_CAP_SLOT);
        assert_eq!(
            endpoint_target_slot(false, 1),
            READINESS_RESERVED_CAP_SLOT + 1
        );
        assert_eq!(
            endpoint_target_slot(true, 0),
            READINESS_RESERVED_CAP_SLOT + 1
        );
        assert_eq!(
            first_non_endpoint_service_slot(true, 1),
            READINESS_RESERVED_CAP_SLOT + 2
        );
    }
}
