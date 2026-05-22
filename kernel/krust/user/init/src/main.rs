#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const KRUSTBOOT_MAGIC: &[u8; 16] = b"KRUSTBOOTV0\0\0\0\0\0";
const KRUSTBOOT_VERSION: u16 = 3;
const MANIFEST_BUFFER_LEN: usize = 16 * 1024;
const OFFSET_VERSION: usize = 16;
const OFFSET_BOOT_MODULES: usize = 18;
const OFFSET_PROCESSES: usize = 20;
const OFFSET_ENDPOINTS: usize = 22;
const OFFSET_GRANTS: usize = 24;
const OFFSET_STORE_OBJECTS: usize = 26;
const OFFSET_STATE_VOLUMES: usize = 28;
const OFFSET_NETWORK_PORTS: usize = 30;
const OFFSET_GENERATION_ID: usize = 32;
const STRING_LEN: usize = 64;
const OFFSET_PARENT_GENERATION_ID: usize = OFFSET_GENERATION_ID + STRING_LEN;
const BOOT_MODULE_RECORD_LEN: usize = STRING_LEN * 2;
const PROCESS_REF_COUNT: usize = 4;
const REF_LIST_LEN: usize = 2 + PROCESS_REF_COUNT * 2;
const ENDPOINT_REQUIREMENT_LIST_LEN: usize = 2 + PROCESS_REF_COUNT * 4;
const PROCESS_RECORD_LEN: usize = STRING_LEN * 4 + 4 + REF_LIST_LEN * 2 + ENDPOINT_REQUIREMENT_LIST_LEN;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;
const MAX_PROCESSES: usize = 16;
const RESTART_ON_FAILURE: u16 = 1;
const RESTART_ALWAYS: u16 = 2;
const MAX_NATIVE_RESTARTS: u16 = 1;
const STATUS_RUNNING: u64 = u64::MAX - 8;
const READINESS_TIMEOUT_MS: u64 = 50;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"vertex-init started");

    let mut manifest = [0u8; MANIFEST_BUFFER_LEN];
    let manifest_len = sys::read_manifest(&mut manifest);
    if manifest_len == sys::STATUS_BAD_CAPABILITY
        || manifest_len == sys::STATUS_BAD_BUFFER
        || manifest_len == sys::STATUS_TOO_LARGE
    {
        log(b"vertex-init manifest read failed");
        sys::exit(1);
    }

    let Ok(manifest_len) = usize::try_from(manifest_len) else {
        log(b"vertex-init manifest length invalid");
        sys::exit(1);
    };

    if manifest_len < OFFSET_PARENT_GENERATION_ID + STRING_LEN
        || !valid_magic(&manifest[..manifest_len])
        || read_u16(&manifest, OFFSET_VERSION) != KRUSTBOOT_VERSION
    {
        log(b"vertex-init manifest invalid");
        sys::exit(1);
    }

    let generation =
        fixed_string(&manifest[OFFSET_GENERATION_ID..OFFSET_GENERATION_ID + STRING_LEN]);
    let parent_generation = fixed_string(
        &manifest[OFFSET_PARENT_GENERATION_ID..OFFSET_PARENT_GENERATION_ID + STRING_LEN],
    );

    log(b"vertex-init received cap[0]=manifest-read");
    log(b"vertex-init received cap[1]=serial-log");
    log(b"vertex-init received cap[2]=process-control");

    log_prefix(b"Boot generation: ", generation);
    log_prefix(b"vertex-init manifest generation: ", generation);

    let boot_modules = read_u16(&manifest, OFFSET_BOOT_MODULES);
    let processes = read_u16(&manifest, OFFSET_PROCESSES);
    let endpoints = read_u16(&manifest, OFFSET_ENDPOINTS);
    let grants = read_u16(&manifest, OFFSET_GRANTS);
    let store_objects = read_u16(&manifest, OFFSET_STORE_OBJECTS);
    let state_volumes = read_u16(&manifest, OFFSET_STATE_VOLUMES);
    let network_ports = read_u16(&manifest, OFFSET_NETWORK_PORTS);

    log_count(b"vertex-init boot modules: ", boot_modules);
    log_count(b"vertex-init processes: ", processes);
    log_count(b"vertex-init endpoints: ", endpoints);
    log_count(b"vertex-init grants: ", grants);
    log_count(b"vertex-init store objects: ", store_objects);
    log_count(b"vertex-init state volumes: ", state_volumes);
    log_count(b"vertex-init network ports: ", network_ports);

    let mut order = [0u16; MAX_PROCESSES];
    let Some(order_len) = activation_plan(
        &manifest[..manifest_len],
        boot_modules,
        processes,
        &mut order,
    ) else {
        activation_failed(parent_generation);
    };

    log(b"vertex-init activation plan:");
    let mut index = 0;
    while index < order_len {
        let process_index = order[index] as usize;
        let name = process_name(&manifest[..manifest_len], boot_modules, process_index);
        log_plan_entry(index + 1, name);
        index += 1;
    }

    index = 0;
    while index < order_len {
        let process_index = order[index] as usize;
        let name = process_name(&manifest[..manifest_len], boot_modules, process_index);
        if process_requires_endpoint(&manifest[..manifest_len], boot_modules, process_index) {
            transfer_endpoint_requirements(
                &manifest[..manifest_len],
                boot_modules,
                process_index,
                name,
                parent_generation,
            );
        }

        start_service(name, process_index as u64, parent_generation);
        if process_has_health(&manifest[..manifest_len], boot_modules, process_index) {
            wait_ready(name, parent_generation);
        }
        index += 1;
    }

    supervise_services(
        &manifest[..manifest_len],
        boot_modules,
        &order,
        order_len,
        parent_generation,
    );

    log(b"Native manifest-driven activation ok");
    log(b"Native readiness activation ok");
    log(b"Native service activation ok");
    sys::exit(0)
}

fn valid_magic(manifest: &[u8]) -> bool {
    manifest.len() >= KRUSTBOOT_MAGIC.len() && &manifest[..KRUSTBOOT_MAGIC.len()] == KRUSTBOOT_MAGIC
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn fixed_string(bytes: &[u8]) -> &[u8] {
    let mut len = 0;
    while len < bytes.len() && bytes[len] != 0 {
        len += 1;
    }
    &bytes[..len]
}

fn boot_modules_base() -> usize {
    OFFSET_PARENT_GENERATION_ID + STRING_LEN
}

fn process_base(boot_modules: u16) -> usize {
    boot_modules_base() + boot_modules as usize * BOOT_MODULE_RECORD_LEN
}

fn process_offset(boot_modules: u16, process_index: usize) -> usize {
    process_base(boot_modules) + process_index * PROCESS_RECORD_LEN
}

fn process_name(manifest: &[u8], boot_modules: u16, process_index: usize) -> &[u8] {
    let offset = process_offset(boot_modules, process_index);
    fixed_string(&manifest[offset..offset + STRING_LEN])
}

fn process_initial(manifest: &[u8], boot_modules: u16, process_index: usize) -> bool {
    let offset = process_offset(boot_modules, process_index) + STRING_LEN * 2;
    read_u16(manifest, offset) & 1 != 0
}

fn process_restart_policy(manifest: &[u8], boot_modules: u16, process_index: usize) -> u16 {
    let offset = process_offset(boot_modules, process_index) + STRING_LEN * 2 + 2;
    read_u16(manifest, offset)
}

fn process_health(manifest: &[u8], boot_modules: u16, process_index: usize) -> &[u8] {
    let offset = process_offset(boot_modules, process_index) + STRING_LEN * 3 + 4;
    fixed_string(&manifest[offset..offset + STRING_LEN])
}

fn process_has_health(manifest: &[u8], boot_modules: u16, process_index: usize) -> bool {
    !process_health(manifest, boot_modules, process_index).is_empty()
}

fn start_after_offset(boot_modules: u16, process_index: usize) -> usize {
    process_offset(boot_modules, process_index) + STRING_LEN * 4 + 4
}

fn requires_offset(boot_modules: u16, process_index: usize) -> usize {
    start_after_offset(boot_modules, process_index) + REF_LIST_LEN
}

fn provides_offset(boot_modules: u16, process_index: usize) -> usize {
    requires_offset(boot_modules, process_index) + ENDPOINT_REQUIREMENT_LIST_LEN
}

fn ref_count(manifest: &[u8], offset: usize) -> usize {
    read_u16(manifest, offset) as usize
}

fn ref_value(manifest: &[u8], offset: usize, index: usize) -> u16 {
    read_u16(manifest, offset + 2 + index * 2)
}

fn endpoint_requirement_value(manifest: &[u8], offset: usize, index: usize) -> u16 {
    read_u16(manifest, offset + 2 + index * 4)
}

fn endpoint_requirement_rights(manifest: &[u8], offset: usize, index: usize) -> u16 {
    read_u16(manifest, offset + 2 + index * 4 + 2)
}

fn process_requires_endpoint(manifest: &[u8], boot_modules: u16, process_index: usize) -> bool {
    ref_count(manifest, requires_offset(boot_modules, process_index)) > 0
}

fn activation_plan(
    manifest: &[u8],
    boot_modules: u16,
    processes: u16,
    order: &mut [u16; MAX_PROCESSES],
) -> Option<usize> {
    if missing_provider(manifest, boot_modules, processes) {
        log(b"vertex-init activation failed: missing provider");
        return None;
    }

    let mut planned = [false; MAX_PROCESSES];
    let mut service_count = 0;
    let mut index = 0;
    while index < processes as usize {
        if process_initial(manifest, boot_modules, index) {
            planned[index] = true;
        } else {
            service_count += 1;
        }
        index += 1;
    }

    let mut out = 0;
    while out < service_count {
        let mut progress = false;
        index = 0;
        while index < processes as usize {
            if !planned[index]
                && !process_initial(manifest, boot_modules, index)
                && dependencies_ready(manifest, boot_modules, index, &planned)
            {
                planned[index] = true;
                order[out] = index as u16;
                out += 1;
                progress = true;
            }
            index += 1;
        }

        if !progress {
            log(b"vertex-init activation failed: dependency cycle");
            return None;
        }
    }

    Some(out)
}

fn dependencies_ready(
    manifest: &[u8],
    boot_modules: u16,
    process_index: usize,
    planned: &[bool; MAX_PROCESSES],
) -> bool {
    let offset = start_after_offset(boot_modules, process_index);
    let count = ref_count(manifest, offset);
    let mut index = 0;
    while index < count {
        let dependency = ref_value(manifest, offset, index) as usize;
        if dependency >= planned.len() || !planned[dependency] {
            return false;
        }
        index += 1;
    }
    true
}

fn missing_provider(manifest: &[u8], boot_modules: u16, processes: u16) -> bool {
    let mut process_index = 0;
    while process_index < processes as usize {
        let requires = requires_offset(boot_modules, process_index);
        let require_count = ref_count(manifest, requires);
        let mut requirement_index = 0;
        while requirement_index < require_count {
            let endpoint = endpoint_requirement_value(manifest, requires, requirement_index);
            if !endpoint_has_provider(manifest, boot_modules, processes, endpoint) {
                return true;
            }
            requirement_index += 1;
        }
        process_index += 1;
    }
    false
}

fn endpoint_has_provider(
    manifest: &[u8],
    boot_modules: u16,
    processes: u16,
    endpoint: u16,
) -> bool {
    let mut process_index = 0;
    while process_index < processes as usize {
        let provides = provides_offset(boot_modules, process_index);
        let provide_count = ref_count(manifest, provides);
        let mut provide_index = 0;
        while provide_index < provide_count {
            if ref_value(manifest, provides, provide_index) == endpoint {
                return true;
            }
            provide_index += 1;
        }
        process_index += 1;
    }
    false
}

fn wait_ready(expected_name: &[u8], parent_generation: &[u8]) {
    let mut buffer = [0u8; 64];
    let received = sys::readiness_recv_timeout(&mut buffer, READINESS_TIMEOUT_MS);
    if received == sys::STATUS_BAD_CAPABILITY || received == sys::STATUS_BAD_BUFFER {
        log(b"vertex-init readiness wait failed");
        activation_failed(parent_generation);
    }
    if received == sys::STATUS_TIMEOUT {
        log(b"vertex-init readiness timeout");
        activation_failed(parent_generation);
    }

    let Ok(received) = usize::try_from(received) else {
        log(b"vertex-init readiness length invalid");
        activation_failed(parent_generation);
    };

    if received < ENVELOPE_LEN
        || read_u16(&buffer, 0) != PROTOCOL_HEALTH_V0
        || read_u16(&buffer, 2) != MESSAGE_READY
    {
        log(b"vertex-init readiness protocol invalid");
        activation_failed(parent_generation);
    }

    let payload_len = read_u32(&buffer, 4) as usize;
    if payload_len > received - ENVELOPE_LEN {
        log(b"vertex-init readiness payload invalid");
        activation_failed(parent_generation);
    }

    let service_name = &buffer[ENVELOPE_LEN..ENVELOPE_LEN + payload_len];
    if !bytes_eq(service_name, expected_name) {
        log(b"vertex-init readiness service mismatch");
        activation_failed(parent_generation);
    }

    log_prefix(b"vertex-init observed ready: ", service_name);
}

fn transfer_endpoint_requirements(
    manifest: &[u8],
    boot_modules: u16,
    process_index: usize,
    name: &[u8],
    parent_generation: &[u8],
) {
    let offset = requires_offset(boot_modules, process_index);
    let count = ref_count(manifest, offset);
    let has_provided_endpoint = ref_count(manifest, provides_offset(boot_modules, process_index)) > 0;
    let mut requirement_index = 0;
    while requirement_index < count {
        let endpoint_index = endpoint_requirement_value(manifest, offset, requirement_index);
        let manifest_rights = endpoint_requirement_rights(manifest, offset, requirement_index);
        let Some(sys_rights) = endpoint_rights_to_sys(manifest_rights) else {
            log(b"vertex-init endpoint rights invalid");
            activation_failed(parent_generation);
        };
        let auth_slot = endpoint_auth_slot(endpoint_index);
        let target_slot = endpoint_target_slot(has_provided_endpoint, requirement_index);
        log_derive(name, endpoint_index, manifest_rights);
        if sys::cap_derive(auth_slot, sys::CAP_DERIVED, sys_rights) != sys::STATUS_OK {
            log(b"vertex-init cap derive failed");
            activation_failed(parent_generation);
        }
        if sys::cap_transfer(process_index as u64, sys::CAP_DERIVED, target_slot, sys_rights)
            != sys::STATUS_OK
        {
            log(b"vertex-init cap transfer failed");
            activation_failed(parent_generation);
        }
        if sys::cap_drop(sys::CAP_DERIVED) != sys::STATUS_OK {
            log(b"vertex-init cap scratch drop failed");
            activation_failed(parent_generation);
        }
        requirement_index += 1;
    }
}

fn endpoint_auth_slot(endpoint_index: u16) -> u64 {
    if endpoint_index < 2 {
        return u64::MAX;
    }
    sys::CAP_ENDPOINT_AUTH_BASE + (endpoint_index as u64 - 2)
}

fn endpoint_target_slot(has_provided_endpoint: bool, requirement_index: usize) -> u64 {
    if !has_provided_endpoint && requirement_index == 0 {
        0
    } else if has_provided_endpoint {
        3 + requirement_index as u64
    } else {
        2 + requirement_index as u64
    }
}

fn endpoint_rights_to_sys(rights: u16) -> Option<u64> {
    let mut out = 0;
    if rights & 1 != 0 {
        out |= sys::RIGHT_SEND;
    }
    if rights & 2 != 0 {
        out |= sys::RIGHT_RECEIVE;
    }
    if out == 0 || rights & !3 != 0 {
        return None;
    }
    Some(out)
}

fn start_service(name: &[u8], process_index: u64, parent_generation: &[u8]) {
    log_prefix(b"vertex-init starting service: ", name);
    if sys::process_start(process_index) != sys::STATUS_OK {
        log_prefix(b"vertex-init service start failed: ", name);
        activation_failed(parent_generation);
    }
}

fn supervise_services(
    manifest: &[u8],
    boot_modules: u16,
    order: &[u16; MAX_PROCESSES],
    order_len: usize,
    parent_generation: &[u8],
) {
    let mut complete = [false; MAX_PROCESSES];
    let mut restart_counts = [0u16; MAX_PROCESSES];
    let mut complete_count = 0;
    let mut restart_observed = false;

    while complete_count < order_len {
        let mut made_progress = false;
        let mut index = 0;
        while index < order_len {
            let process_index = order[index] as usize;
            if complete[process_index] {
                index += 1;
                continue;
            }

            let status = sys::process_status(process_index as u64);
            if status == sys::STATUS_BAD_CAPABILITY {
                log(b"vertex-init process status failed");
                activation_failed(parent_generation);
            }
            if status == STATUS_RUNNING {
                index += 1;
                continue;
            }

            let name = process_name(manifest, boot_modules, process_index);
            let restart_policy = process_restart_policy(manifest, boot_modules, process_index);
            let restart_count = restart_counts[process_index];
            let should_restart = restart_count < MAX_NATIVE_RESTARTS
                && (restart_policy == RESTART_ALWAYS
                    || (restart_policy == RESTART_ON_FAILURE && status != 0));
            if should_restart {
                if status == 0 {
                    log(b"vertex-init observes exit");
                    log(b"restart policy = always");
                } else {
                    log(b"vertex-init observes failure");
                    if restart_policy == RESTART_ALWAYS {
                        log(b"restart policy = always");
                    } else {
                        log(b"restart policy = on-failure");
                    }
                }
                log_restart_once(name);
                if sys::process_start(process_index as u64) != sys::STATUS_OK {
                    log_prefix(b"vertex-init service restart failed: ", name);
                    activation_failed(parent_generation);
                }
                restart_counts[process_index] = restart_count.saturating_add(1);
                restart_observed = true;
                made_progress = true;
                index += 1;
                continue;
            }

            if status == 0 {
                complete[process_index] = true;
                complete_count += 1;
                made_progress = true;
                index += 1;
                continue;
            }

            log_prefix(b"vertex-init service failed: ", name);
            activation_failed(parent_generation);
        }

        if complete_count < order_len && !made_progress {
            sys::yield_now();
        }
    }

    if restart_observed {
        log(b"Native restart policy ok");
    }
}

fn activation_failed(parent_generation: &[u8]) -> ! {
    log(b"activation failed");
    if !parent_generation.is_empty() {
        log_prefix(b"falling back to generation: ", parent_generation);
        if sys::rollback_generation(parent_generation) != sys::STATUS_OK {
            log(b"rollback activation failed");
            sys::exit(1);
        }
        loop {
            sys::pause();
        }
    }
    sys::exit(1);
}

fn log_restart_once(value: &[u8]) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, b"vertex-init restarts ");
    let len = append(&mut buffer, len, value);
    let len = append(&mut buffer, len, b" once");
    log(&buffer[..len]);
}

fn log(message: &[u8]) {
    if sys::log(message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn log_prefix(prefix: &[u8], value: &[u8]) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, prefix);
    let len = append(&mut buffer, len, value);
    log(&buffer[..len]);
}

fn log_plan_entry(index: usize, value: &[u8]) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, b"  ");
    let len = append_decimal(&mut buffer, len, index as u64);
    let len = append(&mut buffer, len, b". ");
    let len = append(&mut buffer, len, value);
    log(&buffer[..len]);
}

fn log_derive(value: &[u8], endpoint_index: u16, rights: u16) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, b"vertex-init derives endpoint cap for ");
    let len = append(&mut buffer, len, value);
    let len = append(&mut buffer, len, b" from endpoint[");
    let len = append_decimal(&mut buffer, len, endpoint_index as u64);
    let len = append(&mut buffer, len, b"] rights=");
    let len = append_manifest_endpoint_rights(&mut buffer, len, rights);
    log(&buffer[..len]);
}

fn append_manifest_endpoint_rights(buffer: &mut [u8], offset: usize, rights: u16) -> usize {
    let mut out = offset;
    let mut wrote = false;
    if rights & 1 != 0 {
        out = append(buffer, out, b"send");
        wrote = true;
    }
    if rights & 2 != 0 {
        if wrote {
            out = append(buffer, out, b"|");
        }
        out = append(buffer, out, b"receive");
        wrote = true;
    }
    if !wrote {
        out = append(buffer, out, b"none");
    }
    out
}

fn log_count(prefix: &[u8], value: u16) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, prefix);
    let len = append_decimal(&mut buffer, len, value as u64);
    log(&buffer[..len]);
}

fn append(buffer: &mut [u8], mut offset: usize, value: &[u8]) -> usize {
    let mut index = 0;
    while offset < buffer.len() && index < value.len() {
        buffer[offset] = value[index];
        offset += 1;
        index += 1;
    }
    offset
}

fn append_decimal(buffer: &mut [u8], offset: usize, mut value: u64) -> usize {
    if value == 0 {
        return append(buffer, offset, b"0");
    }

    let mut digits = [0u8; 20];
    let mut len = 0;
    while value > 0 {
        digits[len] = b'0' + (value % 10) as u8;
        value /= 10;
        len += 1;
    }

    let mut out = offset;
    while len > 0 {
        len -= 1;
        out = append(buffer, out, &digits[len..len + 1]);
    }
    out
}

fn bytes_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
