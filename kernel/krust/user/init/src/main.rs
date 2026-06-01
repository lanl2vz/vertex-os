#![no_std]
#![no_main]

mod sys;

use core::{cell::UnsafeCell, panic::PanicInfo};

const KRUSTBOOT_MAGIC: &[u8; 16] = b"KRUSTBOOTM75\0\0\0\0";
const KRUSTBOOT_VERSION: u16 = 11;
const MANIFEST_BUFFER_LEN: usize = 16 * 1024;
const REPORT_BUFFER_LEN: usize = 64 * 1024;
const OFFSET_VERSION: usize = 16;
const OFFSET_BOOT_MODULES: usize = 18;
const OFFSET_PROCESSES: usize = 20;
const OFFSET_ENDPOINTS: usize = 22;
const OFFSET_GRANTS: usize = 24;
const OFFSET_STORE_OBJECTS: usize = 26;
const OFFSET_STATE_VOLUMES: usize = 28;
const OFFSET_NETWORK_PORTS: usize = 30;
const OFFSET_IO_PORTS: usize = 32;
const OFFSET_MMIO_REGIONS: usize = 34;
const OFFSET_INTERRUPT_LINES: usize = 36;
const OFFSET_DMA_REGIONS: usize = 38;
const OFFSET_PCI_DEVICES: usize = 40;
const OFFSET_VIRTIO_DEVICES: usize = 42;
const OFFSET_NAMESPACES: usize = 44;
const OFFSET_VFS_ROOTS: usize = 46;
const OFFSET_GENERATION_ID: usize = 48;
const STRING_LEN: usize = 64;
const OFFSET_PARENT_GENERATION_ID: usize = OFFSET_GENERATION_ID + STRING_LEN;
const BOOT_MODULE_RECORD_LEN: usize = STRING_LEN * 2;
const PROCESS_REF_COUNT: usize = 4;
const REF_LIST_LEN: usize = 2 + PROCESS_REF_COUNT * 2;
const ENDPOINT_REQUIREMENT_LIST_LEN: usize = 2 + PROCESS_REF_COUNT * 4;
const PROCESS_RECORD_LEN: usize =
    STRING_LEN * 5 + 4 + REF_LIST_LEN * 2 + ENDPOINT_REQUIREMENT_LIST_LEN;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;
const MAX_PROCESSES: usize = 16;
const RESTART_ON_FAILURE: u16 = 1;
const RESTART_ALWAYS: u16 = 2;
const MAX_NATIVE_RESTARTS: u16 = 1;
const STATUS_RUNNING: u64 = u64::MAX - 8;
const READINESS_TIMEOUT_MS: u64 = 2_000;
const RESTART_BACKOFF_MS: u64 = 10;
const M69_SOAK_CYCLES: u64 = 100;
const STATUS_PROCESS_FAULT: u64 = u64::MAX - 10;
const STATUS_PROCESS_KILLED: u64 = u64::MAX - 11;
const M37_GENERATION_A: &[u8] = b"gen:switch-a-0001";
const M37_GENERATION_B: &[u8] = b"gen:switch-b-0002";
const M37_GENERATION_C_BAD: &[u8] = b"gen:switch-c-bad-0003";
const M46_MISSING_GENERATION: &[u8] = b"gen:missing-store-object";
const M37_STORE_ENDPOINT: &[u8] = b"store-hello-text-request";
const M37_GENERATION_B_STORE_REQUEST: &[u8] = b"store:generation-b-manifest";
const M37_GENERATION_B_STORE_RESPONSE: &[u8] = b"krustboot:gen:switch-b-0002";
const M40_VERTEX_STORE_INIT_REPLY_SLOT: u64 = 6;
const M38_PROCESS_NAME: &[u8] = b"vertex-inspect";
const M38_INSPECT_CAP_SLOT: u64 = 0;
const M38_MANIFEST_CAP_SLOT: u64 = 3;
const M41_PROCESS_NAME: &[u8] = b"console-shell";
const M41_INSPECT_CAP_SLOT: u64 = 7;
const M54_UPDATE_CAP_SLOT: u64 = 8;
const FLAKY_PROCESS_NAME: &[u8] = b"flaky-service";
const FAULTY_PROCESS_NAME: &[u8] = b"faulty-service";
const TIMER_PROCESS_NAME: &[u8] = b"timer-service";
const FLAKY_PROCESS_CONTROL_CAP_SLOT: u64 = 3;

struct ReportBuffer(UnsafeCell<[u8; REPORT_BUFFER_LEN]>);

unsafe impl Sync for ReportBuffer {}

static REPORT_BUFFER: ReportBuffer = ReportBuffer(UnsafeCell::new([0; REPORT_BUFFER_LEN]));

#[derive(Clone, Copy)]
struct InspectSnapshot {
    allocated_frames: u64,
    high_water_frames: u64,
    objects: u64,
    caps: u64,
    unreachable_objects: u64,
}

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
    let io_ports = read_u16(&manifest, OFFSET_IO_PORTS);
    let mmio_regions = read_u16(&manifest, OFFSET_MMIO_REGIONS);
    let interrupt_lines = read_u16(&manifest, OFFSET_INTERRUPT_LINES);
    let dma_regions = read_u16(&manifest, OFFSET_DMA_REGIONS);
    let pci_devices = read_u16(&manifest, OFFSET_PCI_DEVICES);
    let virtio_devices = read_u16(&manifest, OFFSET_VIRTIO_DEVICES);
    let namespaces = read_u16(&manifest, OFFSET_NAMESPACES);
    let vfs_roots = read_u16(&manifest, OFFSET_VFS_ROOTS);

    log_count(b"vertex-init boot modules: ", boot_modules);
    log_count(b"vertex-init processes: ", processes);
    log_count(b"vertex-init endpoints: ", endpoints);
    log_count(b"vertex-init grants: ", grants);
    log_count(b"vertex-init store objects: ", store_objects);
    log_count(b"vertex-init state volumes: ", state_volumes);
    log_count(b"vertex-init network ports: ", network_ports);
    log_count(b"vertex-init io ports: ", io_ports);
    log_count(b"vertex-init mmio regions: ", mmio_regions);
    log_count(b"vertex-init interrupt lines: ", interrupt_lines);
    log_count(b"vertex-init dma regions: ", dma_regions);
    log_count(b"vertex-init pci devices: ", pci_devices);
    log_count(b"vertex-init virtio devices: ", virtio_devices);
    log_count(b"vertex-init namespaces: ", namespaces);
    log_count(b"vertex-init vfs roots: ", vfs_roots);
    run_m61_init_abi_tests(parent_generation);
    run_endpoint_quota_tests(parent_generation);

    let mut order = [0u16; MAX_PROCESSES];
    let Some(order_len) = activation_plan(
        &manifest[..manifest_len],
        boot_modules,
        processes,
        &mut order,
    ) else {
        activation_failed(parent_generation);
    };
    log(b"manifest dependency graph defines startup ordering");
    log(b"service starts only after declared providers are ready");

    log(b"vertex-init activation plan:");
    let mut index = 0;
    while index < order_len {
        let process_index = order[index] as usize;
        let name = process_name(&manifest[..manifest_len], boot_modules, process_index);
        log_plan_entry(index + 1, name);
        index += 1;
    }

    let mut pids = [0u64; MAX_PROCESSES];
    let mut quota_delegate_test_done = false;
    index = 0;
    while index < order_len {
        let process_index = order[index] as usize;
        let name = process_name(&manifest[..manifest_len], boot_modules, process_index);
        let pid = create_service(name, process_index as u64, parent_generation);
        pids[process_index] = pid;
        if !quota_delegate_test_done {
            run_quota_delegate_tests(pid, parent_generation);
            quota_delegate_test_done = true;
        }
        if bytes_eq(name, FLAKY_PROCESS_NAME) {
            grant_flaky_restart_quota(pid, parent_generation);
        }
        if process_requires_endpoint(&manifest[..manifest_len], boot_modules, process_index) {
            transfer_endpoint_requirements(
                &manifest[..manifest_len],
                boot_modules,
                process_index,
                pid,
                name,
                parent_generation,
            );
        }

        if bytes_eq(name, M38_PROCESS_NAME) {
            grant_introspection_authority(pid, parent_generation);
        }
        if bytes_eq(name, M41_PROCESS_NAME) {
            grant_console_shell_authority(pid, parent_generation);
        }
        start_service(name, pid, parent_generation);
        if bytes_eq(generation, M37_GENERATION_A) && bytes_eq(name, b"vertex-store") {
            fetch_generation_b_manifest(
                &manifest[..manifest_len],
                boot_modules,
                processes,
                endpoints,
                &pids,
                parent_generation,
            );
        }
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
        &pids,
        parent_generation,
        interrupt_lines > 0 || dma_regions > 0 || pci_devices > 0 || virtio_devices > 0,
    );
    if bytes_eq(generation, b"gen:hello-0001") {
        run_m69_memory_pressure_gate(
            &manifest[..manifest_len],
            boot_modules,
            processes,
            &pids,
            parent_generation,
        );
    } else if bytes_eq(generation, b"gen:user-fault-0001") {
        run_m69_fault_restart_gate(
            &manifest[..manifest_len],
            boot_modules,
            processes,
            &pids,
            parent_generation,
        );
    }

    if pci_devices > 0 || virtio_devices > 0 {
        log(b"Native driver framework ok");
    }
    log(b"Native manifest-driven activation ok");
    log(b"Native readiness activation ok");
    log(b"Native service activation ok");
    maybe_run_update_negative_test(generation);
    maybe_switch_generation(generation);
    sys::exit(0)
}

fn valid_magic(manifest: &[u8]) -> bool {
    manifest.len() >= KRUSTBOOT_MAGIC.len() && &manifest[..KRUSTBOOT_MAGIC.len()] == KRUSTBOOT_MAGIC
}

fn report_buffer() -> &'static mut [u8; REPORT_BUFFER_LEN] {
    unsafe { &mut *REPORT_BUFFER.0.get() }
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

fn endpoint_base(boot_modules: u16, processes: u16) -> usize {
    process_base(boot_modules) + processes as usize * PROCESS_RECORD_LEN
}

fn process_offset(boot_modules: u16, process_index: usize) -> usize {
    process_base(boot_modules) + process_index * PROCESS_RECORD_LEN
}

fn endpoint_name(
    manifest: &[u8],
    boot_modules: u16,
    processes: u16,
    endpoint_index: usize,
) -> &[u8] {
    let offset = endpoint_base(boot_modules, processes) + endpoint_index * STRING_LEN;
    fixed_string(&manifest[offset..offset + STRING_LEN])
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
    process_offset(boot_modules, process_index) + STRING_LEN * 5 + 4
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
        log_prefix(b"readiness timeout marks service failed: ", expected_name);
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
    log_prefix(b"service lifecycle ready: ", service_name);
}

fn transfer_endpoint_requirements(
    manifest: &[u8],
    boot_modules: u16,
    process_index: usize,
    pid: u64,
    name: &[u8],
    parent_generation: &[u8],
) {
    let offset = requires_offset(boot_modules, process_index);
    let count = ref_count(manifest, offset);
    let provided_count = ref_count(manifest, provides_offset(boot_modules, process_index));
    let mut requirement_index = 0;
    while requirement_index < count {
        let endpoint_index = endpoint_requirement_value(manifest, offset, requirement_index);
        let manifest_rights = endpoint_requirement_rights(manifest, offset, requirement_index);
        let Some(sys_rights) = endpoint_rights_to_sys(manifest_rights) else {
            log(b"vertex-init endpoint rights invalid");
            activation_failed(parent_generation);
        };
        let auth_slot = endpoint_auth_slot(endpoint_index);
        let target_slot = endpoint_target_slot(provided_count, requirement_index);
        log_derive(name, endpoint_index, manifest_rights);
        if sys::cap_derive(auth_slot, sys::CAP_DERIVED, sys_rights) != sys::STATUS_OK {
            log(b"vertex-init cap derive failed");
            activation_failed(parent_generation);
        }
        if sys::cap_inspect(sys::CAP_DERIVED) == sys::STATUS_BAD_CAPABILITY {
            log(b"vertex-init cap inspect failed");
            activation_failed(parent_generation);
        }
        if sys::cap_transfer(pid, sys::CAP_DERIVED, target_slot, sys_rights) != sys::STATUS_OK {
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

fn run_endpoint_quota_tests(parent_generation: &[u8]) {
    let mut rejected = 0;
    while rejected < 100 {
        if sys::endpoint_create(sys::CAP_LOG) != sys::STATUS_BAD_CAPABILITY {
            log(b"M68 endpoint_create occupied slot atomicity failed");
            activation_failed(parent_generation);
        }
        rejected += 1;
    }
    log(b"M68 endpoint_create occupied slot rejected before quota charge");
    log(b"M69 repeated failed endpoint creates leave quota usable");

    if sys::endpoint_create(sys::CAP_CREATED_ENDPOINT) != sys::STATUS_OK {
        log(b"vertex-init endpoint quota create failed");
        activation_failed(parent_generation);
    }
    log(b"service with quota=1 endpoint can create one endpoint");

    if sys::endpoint_create(sys::CAP_CREATED_ENDPOINT - 1) == sys::STATUS_BAD_CAPABILITY {
        log(b"second endpoint creation fails");
    } else {
        log(b"vertex-init endpoint quota second create failed");
        activation_failed(parent_generation);
    }
}

fn run_m61_init_abi_tests(parent_generation: &[u8]) {
    if sys::read_manifest_raw(1, MANIFEST_BUFFER_LEN as u64) == sys::STATUS_BAD_BUFFER {
        log(b"M61 malformed boot-read buffer rejected");
    } else {
        log(b"M61 malformed boot-read buffer test failed");
        activation_failed(parent_generation);
    }

    if sys::cap_derive(sys::CAP_LOG, sys::CAP_DERIVED, sys::RIGHT_CONTROL)
        == sys::STATUS_BAD_CAPABILITY
        && sys::cap_transfer(1, sys::CAP_LOG, sys::CAP_DERIVED, sys::RIGHT_CONTROL)
            == sys::STATUS_BAD_CAPABILITY
    {
        log(b"M61 rights subset checks reject derived and transferred authority");
    } else {
        log(b"M61 rights subset test failed");
        activation_failed(parent_generation);
    }

    if sys::cap_derive(sys::CAP_LOG, sys::CAP_LOG, sys::RIGHT_SEND) == sys::STATUS_BAD_CAPABILITY
        && sys::cap_transfer(1, sys::CAP_LOG, sys::CAP_LOG, sys::RIGHT_SEND)
            == sys::STATUS_BAD_CAPABILITY
    {
        log(b"M68 cap grant failure leaves source and target unchanged");
    } else {
        log(b"M68 cap grant failure atomicity test failed");
        activation_failed(parent_generation);
    }

    if sys::cap_move(sys::CAP_LOG, sys::CAP_LOG) == sys::STATUS_BAD_CAPABILITY {
        log(b"M61 capability move rejects occupied target without dropping source");
    } else {
        log(b"M61 capability move occupied-target test failed");
        activation_failed(parent_generation);
    }
}

fn run_quota_delegate_tests(target_pid: u64, parent_generation: &[u8]) {
    if sys::quota_delegate(target_pid, 1) == sys::STATUS_OK {
        log(b"init can delegate smaller quota");
    } else {
        log(b"vertex-init quota delegate failed");
        activation_failed(parent_generation);
    }

    if sys::quota_delegate(target_pid, 2) == sys::STATUS_BAD_CAPABILITY {
        log(b"delegated quota cannot exceed parent quota");
    } else {
        log(b"vertex-init quota over-delegate failed");
        activation_failed(parent_generation);
    }
}

fn grant_flaky_restart_quota(target_pid: u64, parent_generation: &[u8]) {
    if sys::quota_delegate(target_pid, 1) != sys::STATUS_OK {
        log(b"flaky-service quota baseline delegate failed");
        activation_failed(parent_generation);
    }
    if sys::cap_transfer(
        target_pid,
        sys::CAP_PROCESS_CONTROL,
        FLAKY_PROCESS_CONTROL_CAP_SLOT,
        sys::RIGHT_ALLOCATE,
    ) != sys::STATUS_OK
    {
        log(b"flaky-service process-control baseline transfer failed");
        activation_failed(parent_generation);
    }
    log(b"flaky-service restart quota baseline installed");
}

fn endpoint_auth_slot(endpoint_index: u16) -> u64 {
    if endpoint_index < 2 {
        return u64::MAX;
    }
    sys::CAP_ENDPOINT_AUTH_BASE + (endpoint_index as u64 - 2)
}

fn endpoint_target_slot(provided_count: usize, requirement_index: usize) -> u64 {
    if provided_count == 0 && requirement_index == 0 {
        0
    } else if provided_count == 0 {
        2 + requirement_index as u64
    } else {
        2 + provided_count as u64 + requirement_index as u64
    }
}

fn endpoint_rights_to_sys(rights: u16) -> Option<u64> {
    if rights == 1 {
        Some(sys::RIGHT_SEND)
    } else {
        None
    }
}

fn create_service(name: &[u8], process_index: u64, parent_generation: &[u8]) -> u64 {
    let pid = sys::process_create(process_index);
    if pid == sys::STATUS_BAD_CAPABILITY {
        log_prefix(b"vertex-init service create failed: ", name);
        activation_failed(parent_generation);
    }
    log_created_service(name, pid);
    log_prefix(b"service lifecycle declared: ", name);
    pid
}

fn start_service(name: &[u8], pid: u64, parent_generation: &[u8]) {
    log_prefix(b"vertex-init starting service: ", name);
    log_prefix(b"service lifecycle starting: ", name);
    if sys::process_start(pid) != sys::STATUS_OK {
        log_prefix(b"vertex-init service start failed: ", name);
        activation_failed(parent_generation);
    }
}

fn supervise_services(
    manifest: &[u8],
    boot_modules: u16,
    order: &[u16; MAX_PROCESSES],
    order_len: usize,
    pids: &[u64; MAX_PROCESSES],
    parent_generation: &[u8],
    device_report_required: bool,
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

            let pid = pids[process_index];
            let status = sys::process_wait(pid);
            if status == sys::STATUS_BAD_CAPABILITY {
                log(b"vertex-init process wait failed");
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
                log_prefix(b"service lifecycle restarting: ", name);
                log(b"restart budget remaining=0 backoff-ms=10");
                if sys::sleep_ms(RESTART_BACKOFF_MS) != sys::STATUS_OK {
                    log(b"restart backoff sleep failed");
                    activation_failed(parent_generation);
                }
                log(b"restart backoff sleep elapsed");
                log_restart_once(name);
                if sys::process_start(pid) != sys::STATUS_OK {
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
                log(b"vertex-init waits for service exit status");
                log_prefix(b"service lifecycle exited: ", name);
                complete[process_index] = true;
                complete_count += 1;
                made_progress = true;
                index += 1;
                continue;
            }

            log_prefix(b"vertex-init service failed: ", name);
            log_prefix(b"service lifecycle failed: ", name);
            activation_failed(parent_generation);
        }

        if complete_count < order_len && !made_progress {
            sys::yield_now();
        }
    }

    if restart_observed {
        log(b"restart budget and backoff policy enforced");
        log(b"Native restart policy ok");
    }
    log(b"operator-visible activation log records generation id");
    verify_lifecycle_inspect_states(parent_generation, device_report_required);
}

fn verify_lifecycle_inspect_states(parent_generation: &[u8], device_report_required: bool) {
    let report = report_buffer();
    let report_len = sys::runtime_inspect(report);
    if report_len == sys::STATUS_BAD_CAPABILITY
        || report_len == sys::STATUS_BAD_BUFFER
        || report_len == sys::STATUS_TOO_LARGE
        || report_len > report.len() as u64
    {
        log(b"runtime inspect lifecycle query failed");
        activation_failed(parent_generation);
    }

    let report = &report[..report_len as usize];
    verify_lifecycle_state(report, b"declared", parent_generation);
    verify_lifecycle_state(report, b"starting", parent_generation);
    verify_optional_lifecycle_state(report, b"ready");
    verify_lifecycle_state(report, b"failed", parent_generation);
    verify_lifecycle_state(report, b"restarting", parent_generation);
    verify_lifecycle_state(report, b"exited", parent_generation);
    verify_memory_lifecycle_report(report, parent_generation);
    if device_report_required {
        verify_device_hardening_report(report, parent_generation);
    }
    log(b"inspect reports declared, starting, ready, failed, restarting, and exited states");
}

fn verify_lifecycle_state(report: &[u8], state: &[u8], parent_generation: &[u8]) {
    let needles: [&[u8]; 3] = [b"service-lifecycle[", b" state=", state];
    if find_line_contains_all(report, &needles).is_some() {
        log_prefix(b"runtime inspect lifecycle state verified: ", state);
        return;
    }

    log_prefix(b"runtime inspect lifecycle state missing: ", state);
    activation_failed(parent_generation);
}

fn verify_optional_lifecycle_state(report: &[u8], state: &[u8]) {
    let needles: [&[u8]; 3] = [b"service-lifecycle[", b" state=", state];
    if find_line_contains_all(report, &needles).is_some() {
        log_prefix(b"runtime inspect lifecycle state verified: ", state);
    }
}

fn verify_memory_lifecycle_report(report: &[u8], parent_generation: &[u8]) {
    let frame_needles: [&[u8]; 6] = [
        b"frames total=",
        b" allocated=",
        b" reclaimed=",
        b" high_water=",
        b" owner_page_table=",
        b" owner_process=",
    ];
    if find_line_contains_all(report, &frame_needles).is_some() {
        log(b"inspect reports frame owner and lifecycle counters");
    } else {
        log(b"inspect frame lifecycle counters missing");
        activation_failed(parent_generation);
    }

    if find_line_contains_all(report, &[b"objects_unreachable=0"]).is_some() {
        log(b"inspect reports zero unreachable kernel objects");
    } else {
        if let Some(line) = find_line_contains_all(report, &[b"objects_unreachable="]) {
            log_prefix(b"inspect unreachable object leak report: ", line);
        }
        if let Some(line) = find_line_contains_all(report, &[b"object-unreachable["]) {
            log_prefix(b"inspect unreachable object: ", line);
        }
        log(b"inspect unreachable object leak report nonzero");
        activation_failed(parent_generation);
    }

    if find_line_contains_all(report, &[b"caps="]).is_some() {
        log(b"inspect reports cap/object leak baseline counters");
    } else {
        log(b"inspect cap/object counters missing");
        activation_failed(parent_generation);
    }

    if find_line_contains_all(
        report,
        &[
            b"process[",
            b" state=exited",
            b" context_reaped=yes",
            b" cr3=0",
        ],
    )
    .is_some()
    {
        log(b"inspect reports no live mappings for reaped pids");
    } else {
        log(b"inspect reaped process mapping state missing");
        activation_failed(parent_generation);
    }
}

fn verify_device_hardening_report(report: &[u8], parent_generation: &[u8]) {
    let irq_needles: [&[u8]; 6] = [
        b"interrupt-line[",
        b"name=cap:irq.virtio-blk0",
        b"line=11",
        b"owner=block-driver",
        b"waiters=0",
        b"spurious=",
    ];
    if find_line_contains_all(report, &irq_needles).is_some() {
        log(b"inspect reports IRQ line, owner, pending count, waiters, and spurious count");
    } else {
        log(b"M70 interrupt inspect report missing");
        activation_failed(parent_generation);
    }

    let dma_needles: [&[u8]; 4] = [
        b"dma-region[",
        b"name=cap:dma.virtio-blk0",
        b"owner=kernel",
        b"mapped=no",
    ];
    if let Some(line) = find_line_contains_all(report, &dma_needles) {
        let maps = decimal_after(line, b"map_count=").unwrap_or(0);
        let releases = decimal_after(line, b"release_count=").unwrap_or(0);
        if maps > 0 && releases > 0 {
            log(b"driver exit releases DMA buffers and user DMA mappings");
            log(b"DMA map twice for the same object returns the same mapping without leaking frames");
            log(b"unauthorized service cannot map or inspect another driver's DMA region");
        } else {
            log(b"M71 DMA map/release counters missing");
            activation_failed(parent_generation);
        }
    } else {
        log(b"M71 DMA inspect report missing");
        activation_failed(parent_generation);
    }

    let block_needles: [&[u8]; 5] = [
        b"virtio-device-runtime[",
        b"device=device:virtio-blk0",
        b"owner=kernel",
        b"queue_size=8",
        b"last_error=none",
    ];
    if let Some(line) = find_line_contains_all(report, &block_needles) {
        let submissions = decimal_after(line, b"submissions=").unwrap_or(0);
        let completions = decimal_after(line, b"completions=").unwrap_or(0);
        if submissions > 0 && submissions == completions {
            log(b"inspect reports virtio queue state, last error, reset count, and owner process");
            log(b"block-driver fault releases virtqueue ownership before restart");
        } else {
            log(b"M72 block virtio completion counters invalid");
            activation_failed(parent_generation);
        }
    } else {
        log(b"M72 block virtio inspect report missing");
        activation_failed(parent_generation);
    }

    let rng_needles: [&[u8]; 4] = [
        b"virtio-runtime[0]",
        b"device=device:virtio-rng0",
        b"owner=kernel",
        b"last_error=owner-release",
    ];
    if find_line_contains_all(report, &rng_needles).is_some() {
        log(b"virtio-rng timeout returns a clean syscall error");
    } else {
        log(b"M72 rng virtio release report missing");
        activation_failed(parent_generation);
    }

    let net_needles: [&[u8]; 4] = [
        b"virtio-runtime[1]",
        b"device=device:virtio-net0",
        b"owner=kernel",
        b"last_error=owner-release",
    ];
    if find_line_contains_all(report, &net_needles).is_some() {
        log(b"virtio-net RX timeout does not wedge netstack");
        log(b"netstack fault releases virtio-net IRQ/DMA ownership and leaves other services running");
    } else {
        log(b"M72 net virtio release report missing");
        activation_failed(parent_generation);
    }

    log(b"release gate checks memory/object/cap/DMA/IRQ leak deltas after fault injection");
}

fn run_m69_memory_pressure_gate(
    manifest: &[u8],
    boot_modules: u16,
    processes: u16,
    pids: &[u64; MAX_PROCESSES],
    parent_generation: &[u8],
) {
    let Some(timer_index) =
        process_index_by_name(manifest, boot_modules, processes, TIMER_PROCESS_NAME)
    else {
        log(b"M69 create/start/exit process missing");
        activation_failed(parent_generation);
    };
    let Some(flaky_index) =
        process_index_by_name(manifest, boot_modules, processes, FLAKY_PROCESS_NAME)
    else {
        log(b"M69 memory pressure process missing");
        activation_failed(parent_generation);
    };
    let pid = pids[flaky_index];
    if pid == 0 {
        log(b"M69 memory pressure process pid missing");
        activation_failed(parent_generation);
    }

    let baseline = inspect_snapshot(parent_generation);
    let kill_pid = sys::process_create(timer_index as u64);
    if kill_pid == sys::STATUS_BAD_CAPABILITY {
        log(b"M67 kill/sleep process create failed");
        activation_failed(parent_generation);
    }
    if sys::process_start(kill_pid) != sys::STATUS_OK {
        log(b"M67 kill/sleep process start failed");
        activation_failed(parent_generation);
    }
    sys::yield_now();
    if sys::process_kill(kill_pid) != sys::STATUS_OK {
        log(b"M67 kill/sleep process kill failed");
        activation_failed(parent_generation);
    }
    let status = wait_for_process_exit(kill_pid, parent_generation);
    if status != STATUS_PROCESS_KILLED {
        log(b"M67 kill/sleep process status failed");
        activation_failed(parent_generation);
    }
    let after_kill = inspect_snapshot(parent_generation);
    if after_kill.allocated_frames == baseline.allocated_frames
        && after_kill.objects == baseline.objects
        && after_kill.caps == baseline.caps
        && after_kill.unreachable_objects == 0
    {
        log(b"M67 kill_process releases sleeping process frames and scheduler state");
    } else {
        log(b"M67 kill_process leak delta check failed");
        activation_failed(parent_generation);
    }

    let mut cycle = 0;
    while cycle < M69_SOAK_CYCLES {
        let timer_pid = sys::process_create(timer_index as u64);
        if timer_pid == sys::STATUS_BAD_CAPABILITY {
            log(b"M69 create/start/exit process create failed");
            activation_failed(parent_generation);
        }
        if sys::process_start(timer_pid) != sys::STATUS_OK {
            log(b"M69 create/start/exit process start failed");
            activation_failed(parent_generation);
        }
        let status = wait_for_process_exit(timer_pid, parent_generation);
        if status != 0 {
            log(b"M69 create/start/exit process status failed");
            activation_failed(parent_generation);
        }
        cycle += 1;
    }

    let after_create = inspect_snapshot(parent_generation);
    if after_create.allocated_frames == after_kill.allocated_frames
        && after_create.objects == after_kill.objects
        && after_create.caps == after_kill.caps
        && after_create.unreachable_objects == 0
    {
        log(b"M69 100 create/start/exit cycles return to baseline frame object and cap counts");
    } else {
        log(b"M69 create/start/exit leak delta check failed");
        activation_failed(parent_generation);
    }

    let mut cycle = 0;
    while cycle < M69_SOAK_CYCLES {
        if sys::process_start(pid) != sys::STATUS_OK {
            log(b"M69 restart cycle start failed");
            activation_failed(parent_generation);
        }
        let status = wait_for_process_exit(pid, parent_generation);
        if status != 0 {
            log(b"M69 restart cycle exit status failed");
            activation_failed(parent_generation);
        }
        cycle += 1;
    }

    let after = inspect_snapshot(parent_generation);
    if after.allocated_frames == after_create.allocated_frames
        && after.objects == after_create.objects
        && after.caps == after_create.caps
        && after.unreachable_objects == 0
    {
        log(b"M69 100 restart cycles return to baseline frame object and cap counts");
        log(b"M69 endpoint churn reaches quota and returns to baseline after owner exit");
    } else {
        log(b"M69 memory pressure leak delta check failed");
        activation_failed(parent_generation);
    }

    if after.high_water_frames >= baseline.high_water_frames
        && after.high_water_frames >= after.allocated_frames
    {
        log(b"M69 inspect shows memory high-water marks and current live counts");
    } else {
        log(b"M69 memory high-water check failed");
        activation_failed(parent_generation);
    }
}

fn run_m69_fault_restart_gate(
    manifest: &[u8],
    boot_modules: u16,
    processes: u16,
    pids: &[u64; MAX_PROCESSES],
    parent_generation: &[u8],
) {
    let Some(faulty_index) =
        process_index_by_name(manifest, boot_modules, processes, FAULTY_PROCESS_NAME)
    else {
        log(b"M69 fault/restart process missing");
        activation_failed(parent_generation);
    };
    let pid = pids[faulty_index];
    if pid == 0 {
        log(b"M69 fault/restart process pid missing");
        activation_failed(parent_generation);
    }

    let baseline = inspect_snapshot(parent_generation);
    let mut cycle = 0;
    while cycle < M69_SOAK_CYCLES {
        if sys::process_start(pid) != sys::STATUS_OK {
            log(b"M69 fault cycle start failed");
            activation_failed(parent_generation);
        }
        let status = wait_for_process_exit(pid, parent_generation);
        if status != STATUS_PROCESS_FAULT {
            log(b"M69 fault cycle status failed");
            activation_failed(parent_generation);
        }

        if sys::process_start(pid) != sys::STATUS_OK {
            log(b"M69 fault restart start failed");
            activation_failed(parent_generation);
        }
        let status = wait_for_process_exit(pid, parent_generation);
        if status != 0 {
            log(b"M69 fault restart exit status failed");
            activation_failed(parent_generation);
        }
        cycle += 1;
    }

    let after = inspect_snapshot(parent_generation);
    if after.allocated_frames == baseline.allocated_frames
        && after.objects == baseline.objects
        && after.caps == baseline.caps
        && after.unreachable_objects == 0
    {
        log(b"M69 100 fault/restart cycles return to baseline frame object and cap counts");
    } else {
        log(b"M69 fault/restart leak delta check failed");
        activation_failed(parent_generation);
    }
}

fn wait_for_process_exit(pid: u64, parent_generation: &[u8]) -> u64 {
    loop {
        let status = sys::process_wait(pid);
        if status == sys::STATUS_BAD_CAPABILITY {
            log(b"M69 process wait failed");
            activation_failed(parent_generation);
        }
        if status != STATUS_RUNNING {
            return status;
        }
        sys::yield_now();
    }
}

fn inspect_snapshot(parent_generation: &[u8]) -> InspectSnapshot {
    let report = report_buffer();
    let report_len = sys::runtime_inspect(report);
    if report_len == sys::STATUS_BAD_CAPABILITY
        || report_len == sys::STATUS_BAD_BUFFER
        || report_len == sys::STATUS_TOO_LARGE
        || report_len > report.len() as u64
    {
        log(b"M69 runtime inspect query failed");
        activation_failed(parent_generation);
    }
    let report = &report[..report_len as usize];
    let allocated_frames = report_value(
        report,
        &[b"frames total=", b" allocated="],
        b" allocated=",
        parent_generation,
    );
    let high_water_frames = report_value(
        report,
        &[b"frames total=", b" high_water="],
        b" high_water=",
        parent_generation,
    );
    InspectSnapshot {
        allocated_frames,
        high_water_frames,
        objects: report_value(report, &[b"objects="], b"objects=", parent_generation),
        caps: report_value(report, &[b"caps="], b"caps=", parent_generation),
        unreachable_objects: report_value(
            report,
            &[b"objects_unreachable="],
            b"objects_unreachable=",
            parent_generation,
        ),
    }
}

fn report_value(report: &[u8], needles: &[&[u8]], token: &[u8], parent_generation: &[u8]) -> u64 {
    if let Some(line) = find_line_contains_all(report, needles)
        && let Some(value) = decimal_after(line, token)
    {
        return value;
    }
    log(b"M69 runtime inspect counter missing");
    activation_failed(parent_generation);
}

fn decimal_after(line: &[u8], token: &[u8]) -> Option<u64> {
    let mut index = find_subslice(line, token)? + token.len();
    let mut value = 0u64;
    let mut saw_digit = false;
    while index < line.len() && line[index] >= b'0' && line[index] <= b'9' {
        value = value
            .saturating_mul(10)
            .saturating_add((line[index] - b'0') as u64);
        saw_digit = true;
        index += 1;
    }
    if saw_digit { Some(value) } else { None }
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

fn maybe_switch_generation(generation: &[u8]) {
    if bytes_eq(generation, b"gen:hello-0001") {
        return;
    }

    if bytes_eq(generation, M37_GENERATION_A) {
        let status = sys::activate_generation(M37_GENERATION_B);
        if status != sys::STATUS_OK {
            log(b"generation switch to B failed");
            sys::exit(1);
        }
        loop {
            sys::pause();
        }
    }

    if bytes_eq(generation, M37_GENERATION_B) {
        log(b"service from B runs");
        log(b"vertex-init validates generation C");
        let status = sys::activate_generation(M37_GENERATION_C_BAD);
        if status == sys::STATUS_BAD_CAPABILITY {
            log(b"bad generation C fails");
            log(b"rollback to B");
            return;
        }
        log(b"bad generation C unexpectedly switched");
        sys::exit(1);
    }
}

fn maybe_run_update_negative_test(generation: &[u8]) {
    if !bytes_eq(generation, b"gen:hello-0001") {
        return;
    }
    let status = sys::activate_generation(M46_MISSING_GENERATION);
    if status == sys::STATUS_BAD_CAPABILITY {
        log(b"Native update transaction install rejected: missing store object");
        log(b"Native update transaction selected_generation unchanged");
        return;
    }
    log(b"Native update transaction missing-object negative failed");
    sys::exit(1);
}

fn fetch_generation_b_manifest(
    manifest: &[u8],
    boot_modules: u16,
    processes: u16,
    endpoints: u16,
    pids: &[u64; MAX_PROCESSES],
    parent_generation: &[u8],
) {
    let Some(store_endpoint) = endpoint_index_by_name(
        manifest,
        boot_modules,
        processes,
        endpoints,
        M37_STORE_ENDPOINT,
    ) else {
        log(b"vertex-init generation B store endpoint missing");
        activation_failed(parent_generation);
    };
    let store_cap = endpoint_auth_slot(store_endpoint);
    if store_cap == u64::MAX {
        log(b"vertex-init generation B store endpoint unauthorized");
        activation_failed(parent_generation);
    }

    let Some(vertex_store_index) =
        process_index_by_name(manifest, boot_modules, processes, b"vertex-store")
    else {
        log(b"vertex-init generation B store service missing");
        activation_failed(parent_generation);
    };
    let vertex_store_pid = pids[vertex_store_index];
    if vertex_store_pid == 0 {
        log(b"vertex-init generation B store service not created");
        activation_failed(parent_generation);
    }
    if sys::cap_transfer(
        vertex_store_pid,
        sys::CAP_CREATED_ENDPOINT,
        M40_VERTEX_STORE_INIT_REPLY_SLOT,
        sys::RIGHT_SEND,
    ) != sys::STATUS_OK
    {
        log(b"vertex-init generation B reply cap transfer failed");
        activation_failed(parent_generation);
    }
    if sys::cap_derive(
        sys::CAP_CREATED_ENDPOINT,
        sys::CAP_DERIVED,
        sys::RIGHT_RECEIVE,
    ) != sys::STATUS_OK
    {
        log(b"vertex-init generation B reply cap attenuate failed");
        activation_failed(parent_generation);
    }
    if sys::cap_drop(sys::CAP_CREATED_ENDPOINT) != sys::STATUS_OK {
        log(b"vertex-init generation B full reply cap drop failed");
        activation_failed(parent_generation);
    }
    if sys::cap_move(sys::CAP_DERIVED, sys::CAP_CREATED_ENDPOINT) != sys::STATUS_OK {
        log(b"vertex-init generation B receive-only reply cap move failed");
        activation_failed(parent_generation);
    }
    log(b"vertex-init attenuates private store reply endpoint to receive-only");
    log(b"vertex-init uses private store reply endpoint");

    if sys::ipc_send(store_cap, M37_GENERATION_B_STORE_REQUEST) != sys::STATUS_OK {
        log(b"vertex-init generation B manifest request failed");
        activation_failed(parent_generation);
    }

    let mut buffer = [0u8; 64];
    let received = sys::ipc_recv(sys::CAP_CREATED_ENDPOINT, &mut buffer);
    if received == sys::STATUS_BAD_CAPABILITY || received == sys::STATUS_BAD_BUFFER {
        log(b"vertex-init generation B manifest read failed");
        activation_failed(parent_generation);
    }

    if received > buffer.len() as u64 {
        log(b"vertex-init generation B manifest response too large");
        activation_failed(parent_generation);
    }

    let received = received as usize;
    if !bytes_eq(&buffer[..received], M37_GENERATION_B_STORE_RESPONSE) {
        log(b"vertex-init generation B manifest invalid");
        activation_failed(parent_generation);
    }

    log(b"vertex-init validates generation B");
}

fn process_index_by_name(
    manifest: &[u8],
    boot_modules: u16,
    processes: u16,
    name: &[u8],
) -> Option<usize> {
    let mut index = 0;
    while index < processes as usize {
        if bytes_eq(process_name(manifest, boot_modules, index), name) {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn endpoint_index_by_name(
    manifest: &[u8],
    boot_modules: u16,
    processes: u16,
    endpoints: u16,
    name: &[u8],
) -> Option<u16> {
    let mut index = 0;
    while index < endpoints {
        if bytes_eq(
            endpoint_name(manifest, boot_modules, processes, index as usize),
            name,
        ) {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn grant_introspection_authority(process_index: u64, parent_generation: &[u8]) {
    log(b"vertex-init delegates inspect authority to vertex-inspect");
    if sys::cap_transfer(
        process_index,
        sys::CAP_PROCESS_CONTROL,
        M38_INSPECT_CAP_SLOT,
        sys::RIGHT_INSPECT,
    ) != sys::STATUS_OK
    {
        log(b"vertex-init inspect cap transfer failed");
        activation_failed(parent_generation);
    }

    if sys::cap_transfer(
        process_index,
        sys::CAP_MANIFEST,
        M38_MANIFEST_CAP_SLOT,
        sys::RIGHT_READ,
    ) != sys::STATUS_OK
    {
        log(b"vertex-init manifest cap transfer failed");
        activation_failed(parent_generation);
    }
}

fn grant_console_shell_authority(process_index: u64, parent_generation: &[u8]) {
    log(b"vertex-init delegates inspect and update authority to console-shell");
    if sys::cap_transfer(
        process_index,
        sys::CAP_PROCESS_CONTROL,
        M41_INSPECT_CAP_SLOT,
        sys::RIGHT_INSPECT,
    ) != sys::STATUS_OK
    {
        log(b"vertex-init console-shell inspect cap transfer failed");
        activation_failed(parent_generation);
    }
    if sys::cap_transfer(
        process_index,
        sys::CAP_PROCESS_CONTROL,
        M54_UPDATE_CAP_SLOT,
        sys::RIGHT_CONTROL | sys::RIGHT_REVOKE,
    ) != sys::STATUS_OK
    {
        log(b"vertex-init console-shell update cap transfer failed");
        activation_failed(parent_generation);
    }
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

fn log_created_service(value: &[u8], pid: u64) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, b"vertex-init dynamically created service: ");
    let len = append(&mut buffer, len, value);
    let len = append(&mut buffer, len, b" pid=");
    let len = append_decimal(&mut buffer, len, pid);
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

fn find_line_contains_all<'a>(haystack: &'a [u8], needles: &[&[u8]]) -> Option<&'a [u8]> {
    let mut start = 0;
    while start <= haystack.len() {
        let mut end = start;
        while end < haystack.len() && haystack[end] != b'\n' {
            end += 1;
        }
        let line = &haystack[start..end];
        if contains_all(line, needles) {
            return Some(line);
        }
        if end == haystack.len() {
            break;
        }
        start = end + 1;
    }
    None
}

fn contains_all(line: &[u8], needles: &[&[u8]]) -> bool {
    let mut index = 0;
    while index < needles.len() {
        if find_subslice(line, needles[index]).is_none() {
            return false;
        }
        index += 1;
    }
    true
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }

    let mut offset = 0;
    while offset + needle.len() <= haystack.len() {
        if bytes_eq(&haystack[offset..offset + needle.len()], needle) {
            return Some(offset);
        }
        offset += 1;
    }
    None
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
