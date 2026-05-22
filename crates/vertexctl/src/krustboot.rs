use std::collections::BTreeSet;
use vertex_ir::{GenerationManifest, Service};

const MAGIC: &[u8; 16] = b"KRUSTBOOTV0\0\0\0\0\0";
const VERSION: u16 = 2;
const STRING_LEN: usize = 64;
const MAX_BOOT_MODULES: usize = 16;
const MAX_PROCESSES: usize = 16;
const MAX_ENDPOINTS: usize = 16;
const MAX_GRANTS: usize = 64;
const MAX_STORE_OBJECTS: usize = 4;
const MAX_STATE_VOLUMES: usize = 4;
const MAX_PROCESS_REFS: usize = 4;
const RIGHT_SEND: u16 = 1 << 0;
const RIGHT_RECEIVE: u16 = 1 << 1;
const RIGHT_READ: u16 = 1 << 2;
const RIGHT_WRITE: u16 = 1 << 3;
const RIGHT_SNAPSHOT: u16 = 1 << 4;
const RIGHT_RESTORE: u16 = 1 << 5;
const RIGHT_CONTROL: u16 = 1 << 6;
const OBJECT_ENDPOINT: u16 = 1;
const OBJECT_STORE: u16 = 2;
const OBJECT_STATE: u16 = 3;
const OBJECT_TIMER: u16 = 4;
const RESTART_NEVER: u16 = 0;
const RESTART_ON_FAILURE: u16 = 1;
const RESTART_ALWAYS: u16 = 2;
const SERVICE_CAP_SLOT: u16 = 0;
const SERIAL_CAP_SLOT: u16 = 1;
const READINESS_CAP_SLOT: u16 = 2;
const INIT_SERIAL_CAP_SLOT: u16 = 1;
const INIT_READINESS_CAP_SLOT: u16 = 3;
const INIT_ENDPOINT_AUTH_CAP_SLOT: u16 = 4;

pub fn compile(manifest: &GenerationManifest) -> Result<Vec<u8>, String> {
    let plan = derive_plan(manifest)?;
    validate_plan(&plan)?;

    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    push_u16(&mut bytes, VERSION);
    push_count(&mut bytes, plan.boot_modules.len(), "boot_modules")?;
    push_count(&mut bytes, plan.processes.len(), "processes")?;
    push_count(&mut bytes, plan.endpoints.len(), "endpoints")?;
    push_count(&mut bytes, plan.grants.len(), "grants")?;
    push_count(&mut bytes, plan.store_objects.len(), "store_objects")?;
    push_count(&mut bytes, plan.state_volumes.len(), "state_volumes")?;
    push_fixed_str(&mut bytes, &manifest.generation.id)?;
    push_fixed_str(
        &mut bytes,
        manifest.generation.parent.as_deref().unwrap_or_default(),
    )?;

    for module in &plan.boot_modules {
        push_fixed_str(&mut bytes, &module.name)?;
        push_fixed_str(&mut bytes, &module.module_string)?;
    }

    for process in &plan.processes {
        push_fixed_str(&mut bytes, &process.name)?;
        push_fixed_str(&mut bytes, &process.module_string)?;
        push_u16(&mut bytes, u16::from(process.initial));
        push_u16(&mut bytes, process.restart);
        push_fixed_str(&mut bytes, &process.service_id)?;
        push_fixed_str(&mut bytes, &process.health_kind)?;
        push_process_ref_list(&mut bytes, &process.start_after, &plan)?;
        push_endpoint_ref_list(&mut bytes, &process.requires_endpoints, &plan)?;
        push_endpoint_ref_list(&mut bytes, &process.provides_endpoints, &plan)?;
    }

    for endpoint in &plan.endpoints {
        push_fixed_str(&mut bytes, &endpoint.name)?;
    }

    for grant in &plan.grants {
        push_u16(&mut bytes, process_index(&plan, &grant.process)? as u16);
        push_u16(&mut bytes, grant.object_kind);
        push_u16(&mut bytes, object_index(&plan, grant)? as u16);
        push_u16(&mut bytes, grant.cap_slot);
        push_u16(&mut bytes, grant.rights);
        push_u16(&mut bytes, 0);
    }

    for object in &plan.store_objects {
        push_fixed_str(&mut bytes, &object.id)?;
        push_fixed_str(&mut bytes, &object.module_string)?;
        push_fixed_str(&mut bytes, &object.hash)?;
        push_u64(&mut bytes, object.size);
    }

    for state in &plan.state_volumes {
        push_fixed_str(&mut bytes, &state.id)?;
    }

    Ok(bytes)
}

pub fn summary(manifest: &GenerationManifest, output_path: &str, byte_len: usize) -> String {
    let plan = derive_plan(manifest).expect("summary is only called after compile succeeds");

    format!(
        "wrote {output_path}\n\
         format: KrustBootManifest v2\n\
         generation: {}\n\
         parent_generation: {}\n\
         boot_modules: {}\n\
         processes: {}\n\
         endpoints: {}\n\
         grants: {}\n\
         store_objects: {}\n\
         state_volumes: {}\n\
         bytes: {byte_len}",
        manifest.generation.id,
        manifest.generation.parent.as_deref().unwrap_or("<none>"),
        plan.boot_modules.len(),
        plan.processes.len(),
        plan.endpoints.len(),
        plan.grants.len(),
        plan.store_objects.len(),
        plan.state_volumes.len()
    )
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
                "{} receives send authority to endpoint {endpoint}\n\
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
    requires_endpoints: Vec<String>,
    provides_endpoints: Vec<String>,
    health_kind: String,
    restart: u16,
}

#[derive(Debug, Clone)]
struct Endpoint {
    name: String,
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
            rights: RIGHT_RECEIVE,
        });

        grants.push(Grant {
            process: init_name.clone(),
            object_kind: OBJECT_ENDPOINT,
            object_name: endpoint.clone(),
            cap_slot: INIT_ENDPOINT_AUTH_CAP_SLOT,
            rights: RIGHT_SEND | RIGHT_RECEIVE,
        });

        for service in &manifest.services {
            if service.id == root_service.id {
                continue;
            }
            if service
                .requires
                .iter()
                .any(|requirement| requirement.capability == capability.id)
            {
                if native_process_for_service(&processes, &service.id).is_none() {
                    continue;
                }
                add_process_endpoint_ref(
                    &mut processes,
                    &service.id,
                    endpoint.clone(),
                    EndpointRefKind::Requires,
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
                        cap_slot: SERVICE_CAP_SLOT,
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
                        cap_slot: SERVICE_CAP_SLOT,
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
                        cap_slot: SERVICE_CAP_SLOT,
                        rights: rights_mask(
                            &requirement.rights,
                            &capability.rights,
                            &capability.id,
                        )?,
                    });
                }
                _ => {}
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
        for endpoint in &process.requires_endpoints {
            endpoint_index(plan, endpoint)?;
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

    Ok(())
}

enum EndpointRefKind {
    Requires,
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
        EndpointRefKind::Requires => &mut process.requires_endpoints,
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

fn object_index(plan: &BootPlan, grant: &Grant) -> Result<usize, String> {
    match grant.object_kind {
        OBJECT_ENDPOINT => endpoint_index(plan, &grant.object_name),
        OBJECT_STORE => store_index(plan, &grant.object_name),
        OBJECT_STATE => state_index(plan, &grant.object_name),
        OBJECT_TIMER if grant.object_name == "monotonic-timer" => Ok(0),
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
