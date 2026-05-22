use std::collections::BTreeSet;
use vertex_ir::{GenerationManifest, KrustBoot};

const MAGIC: &[u8; 16] = b"KRUSTBOOTV0\0\0\0\0\0";
const VERSION: u16 = 0;
const STRING_LEN: usize = 64;
const MAX_BOOT_MODULES: usize = 4;
const MAX_PROCESSES: usize = 4;
const MAX_ENDPOINTS: usize = 4;
const MAX_GRANTS: usize = 8;
const RIGHT_SEND: u16 = 1 << 0;
const RIGHT_RECEIVE: u16 = 1 << 1;

pub fn compile(manifest: &GenerationManifest) -> Result<Vec<u8>, String> {
    let plan = boot_plan(manifest)?;
    validate_plan(plan)?;

    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    push_u16(&mut bytes, VERSION);
    push_count(&mut bytes, plan.boot_modules.len(), "boot_modules")?;
    push_count(&mut bytes, plan.processes.len(), "processes")?;
    push_count(&mut bytes, plan.endpoints.len(), "endpoints")?;
    push_count(&mut bytes, plan.grants.len(), "grants")?;
    push_fixed_str(&mut bytes, &manifest.generation.id)?;

    for module in &plan.boot_modules {
        push_fixed_str(&mut bytes, &module.name)?;
        push_fixed_str(&mut bytes, &module.module_string)?;
    }

    for process in &plan.processes {
        push_fixed_str(&mut bytes, &process.name)?;
        push_fixed_str(&mut bytes, &process.module_string)?;
        push_u16(&mut bytes, u16::from(process.initial));
        push_u16(&mut bytes, 0);
    }

    for endpoint in &plan.endpoints {
        push_fixed_str(&mut bytes, &endpoint.name)?;
    }

    for grant in &plan.grants {
        push_u16(&mut bytes, index_of_process(plan, &grant.process)? as u16);
        push_u16(&mut bytes, index_of_endpoint(plan, &grant.endpoint)? as u16);
        push_u16(&mut bytes, grant.cap_slot);
        push_u16(&mut bytes, rights_bits(&grant.rights)?);
    }

    Ok(bytes)
}

pub fn summary(manifest: &GenerationManifest, output_path: &str, byte_len: usize) -> String {
    let plan = manifest
        .krust_boot
        .as_ref()
        .expect("summary is only called after compile succeeds");

    format!(
        "wrote {output_path}\n\
         format: KrustBootManifest v0\n\
         generation: {}\n\
         boot_modules: {}\n\
         processes: {}\n\
         endpoints: {}\n\
         grants: {}\n\
         bytes: {byte_len}",
        manifest.generation.id,
        plan.boot_modules.len(),
        plan.processes.len(),
        plan.endpoints.len(),
        plan.grants.len()
    )
}

fn boot_plan(manifest: &GenerationManifest) -> Result<&KrustBoot, String> {
    manifest
        .krust_boot
        .as_ref()
        .ok_or_else(|| "manifest has no krustBoot section".to_owned())
}

fn validate_plan(plan: &KrustBoot) -> Result<(), String> {
    if plan.format != "krustboot.v0" {
        return Err(format!(
            "krustBoot.format must be krustboot.v0, found {}",
            plan.format
        ));
    }
    if plan.boot_modules.is_empty() {
        return Err("krustBoot.bootModules must not be empty".to_owned());
    }
    if plan.processes.is_empty() {
        return Err("krustBoot.processes must not be empty".to_owned());
    }
    if plan.endpoints.is_empty() {
        return Err("krustBoot.endpoints must not be empty".to_owned());
    }
    if plan.grants.is_empty() {
        return Err("krustBoot.grants must not be empty".to_owned());
    }
    if plan.boot_modules.len() > MAX_BOOT_MODULES {
        return Err(format!(
            "krustBoot.bootModules exceeds max {MAX_BOOT_MODULES}"
        ));
    }
    if plan.processes.len() > MAX_PROCESSES {
        return Err(format!("krustBoot.processes exceeds max {MAX_PROCESSES}"));
    }
    if plan.endpoints.len() > MAX_ENDPOINTS {
        return Err(format!("krustBoot.endpoints exceeds max {MAX_ENDPOINTS}"));
    }
    if plan.grants.len() > MAX_GRANTS {
        return Err(format!("krustBoot.grants exceeds max {MAX_GRANTS}"));
    }

    let initial_count = plan
        .processes
        .iter()
        .filter(|process| process.initial)
        .count();
    if initial_count != 1 {
        return Err(format!(
            "krustBoot.processes must contain exactly one initial process, found {initial_count}"
        ));
    }

    let mut module_names = BTreeSet::new();
    let mut module_strings = BTreeSet::new();
    for module in &plan.boot_modules {
        if !module_names.insert(module.name.as_str()) {
            return Err(format!(
                "duplicate krustBoot boot module name {}",
                module.name
            ));
        }
        if !module_strings.insert(module.module_string.as_str()) {
            return Err(format!(
                "duplicate krustBoot module string {}",
                module.module_string
            ));
        }
    }

    let mut process_names = BTreeSet::new();
    for process in &plan.processes {
        if !process_names.insert(process.name.as_str()) {
            return Err(format!("duplicate krustBoot process {}", process.name));
        }
        if plan
            .boot_modules
            .iter()
            .all(|module| module.module_string != process.module_string)
        {
            return Err(format!(
                "krustBoot process {} references unknown module string {}",
                process.name, process.module_string
            ));
        }
    }

    let mut endpoint_names = BTreeSet::new();
    for endpoint in &plan.endpoints {
        if !endpoint_names.insert(endpoint.name.as_str()) {
            return Err(format!("duplicate krustBoot endpoint {}", endpoint.name));
        }
    }

    for grant in &plan.grants {
        index_of_process(plan, &grant.process)?;
        index_of_endpoint(plan, &grant.endpoint)?;
        rights_bits(&grant.rights)?;
    }

    Ok(())
}

fn index_of_process(plan: &KrustBoot, name: &str) -> Result<usize, String> {
    plan.processes
        .iter()
        .position(|process| process.name == name)
        .ok_or_else(|| format!("krustBoot grant references unknown process {name}"))
}

fn index_of_endpoint(plan: &KrustBoot, name: &str) -> Result<usize, String> {
    plan.endpoints
        .iter()
        .position(|endpoint| endpoint.name == name)
        .ok_or_else(|| format!("krustBoot grant references unknown endpoint {name}"))
}

fn rights_bits(rights: &[String]) -> Result<u16, String> {
    let mut bits = 0;
    for right in rights {
        match right.as_str() {
            "send" => bits |= RIGHT_SEND,
            "receive" => bits |= RIGHT_RECEIVE,
            other => return Err(format!("unsupported krustBoot grant right {other}")),
        }
    }
    if bits == 0 {
        return Err("krustBoot grant has no rights".to_owned());
    }
    Ok(bits)
}

fn push_count(bytes: &mut Vec<u8>, count: usize, label: &str) -> Result<(), String> {
    let count =
        u16::try_from(count).map_err(|_| format!("krustBoot {label} count does not fit in u16"))?;
    push_u16(bytes, count);
    Ok(())
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_fixed_str(bytes: &mut Vec<u8>, value: &str) -> Result<(), String> {
    if value.len() >= STRING_LEN {
        return Err(format!(
            "krustboot string is too long: {value} is {} bytes, max {}",
            value.len(),
            STRING_LEN - 1
        ));
    }
    if !value.is_ascii() {
        return Err(format!("krustboot string must be ASCII: {value}"));
    }

    let mut slot = [0u8; STRING_LEN];
    slot[..value.len()].copy_from_slice(value.as_bytes());
    bytes.extend_from_slice(&slot);
    Ok(())
}
