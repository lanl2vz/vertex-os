#![no_std]
#![no_main]

mod sys;

use core::{cell::UnsafeCell, panic::PanicInfo};

const CAP_SHELL_REQUEST: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_COUNTER_REPLY: u64 = 3;
const CAP_CONSOLE_OUTPUT: u64 = 4;
const CAP_CONSOLE_CONTROL: u64 = 5;
const CAP_GENERATION_MANAGER_REQUEST: u64 = 6;
const CAP_COUNTER_REQUEST: u64 = 7;
const CAP_STATE_CONTROL: u64 = 8;
const CAP_INSPECT: u64 = 9;
const CAP_PACKAGE_IMPORT_REQUEST: u64 = 8;
const CAP_STATE_CONTROL_AFTER_PACKAGE_IMPORT: u64 = 9;
const CAP_INSPECT_AFTER_PACKAGE_IMPORT: u64 = 10;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;
const REPORT_BUFFER_LEN: usize = 256 * 1024;
const CONTROL_SHUTDOWN: &[u8] = b"shutdown";
const GENERATION_MANAGER_SHUTDOWN: &[u8] = b"shutdown";
const STATE_CONTROL_PATH: &[u8] = b"/state/counter/control";
const STATE_CLIENT_DRAIN_ATTEMPTS: u64 = 4096;
const SERVICE_NAMES: [&[u8]; 5] = [
    b"vertex-init",
    b"logd",
    b"vertex-store",
    b"vertex-state",
    b"console-shell",
];

struct ReportBuffer(UnsafeCell<[u8; REPORT_BUFFER_LEN]>);

unsafe impl Sync for ReportBuffer {}

static REPORT_BUFFER: ReportBuffer = ReportBuffer(UnsafeCell::new([0; REPORT_BUFFER_LEN]));

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"console-shell ready");
    send_ready();
    console_write(b"Vertex OS v0 appliance booted\nVertex shell ready\n> ");

    loop {
        let mut command = [0u8; 96];
        let received = sys::ipc_recv(CAP_SHELL_REQUEST, &mut command);
        if received == sys::STATUS_BAD_CAPABILITY || received > command.len() as u64 {
            log(b"console-shell command receive failed");
            sys::exit(1);
        }
        let command = &command[..received as usize];
        if bytes_eq(command, b"help") {
            log(b"console-shell command: help");
            log(
                b"commands: generation services devices counter increment state-health install rollback why halt",
            );
            log(
                b"operator commands: current-generation generations generation-status diff-generation planned-authority-delta why who-can which-generation package-list activation-log activate rollback mark-known-good",
            );
            console_write(
                b"commands: generation services devices counter increment state-health install rollback why halt\n> ",
            );
            continue;
        }
        if bytes_eq(command, b"generation") {
            log(b"console-shell command: generation");
            let report = runtime_report();
            let generation = generation_for_self(report);
            console_write_generation(generation);
            continue;
        }
        if bytes_eq(command, b"current-generation") {
            log(b"console-shell command: current-generation");
            let report = runtime_report();
            operator_current_generation(report);
            continue;
        }
        if bytes_eq(command, b"generations") {
            log(b"console-shell command: generations");
            let report = runtime_report();
            operator_generations(report);
            continue;
        }
        if bytes_eq(command, b"generation-status") {
            log(b"console-shell command: generation-status");
            let report = runtime_report();
            operator_generation_status(report);
            continue;
        }
        if bytes_eq(command, b"services") {
            log(b"console-shell command: services");
            let report = runtime_report();
            console_write_services(report);
            continue;
        }
        if bytes_eq(command, b"devices") {
            log(b"console-shell command: devices");
            let report = runtime_report();
            console_write_devices(report);
            continue;
        }
        if bytes_eq(command, b"why svc:echo cap:log.sink") {
            log(b"console-shell command: why");
            let report = runtime_report();
            require_echo_log_authority(report);
            console_write(
                b"svc:echo has send authority because generation graph granted cap slot 0\n> ",
            );
            continue;
        }
        if starts_with(command, b"diff-generation ") {
            log(b"console-shell command: diff-generation");
            let report = runtime_report();
            operator_diff_generation(report, command, false);
            continue;
        }
        if starts_with(command, b"planned-authority-delta ") {
            log(b"console-shell command: planned-authority-delta");
            let report = runtime_report();
            operator_diff_generation(report, command, true);
            continue;
        }
        if starts_with(command, b"why ") {
            log(b"console-shell command: operator why");
            let report = runtime_report();
            operator_why(report, command);
            continue;
        }
        if starts_with(command, b"who-can ") {
            log(b"console-shell command: who-can");
            let report = runtime_report();
            operator_who_can(report, command);
            continue;
        }
        if starts_with(command, b"which-generation ") {
            log(b"console-shell command: which-generation");
            let report = runtime_report();
            operator_which_generation(report, command);
            continue;
        }
        if bytes_eq(command, b"package-list") {
            log(b"console-shell command: package-list");
            let report = runtime_report();
            operator_package_list(report);
            continue;
        }
        if bytes_eq(command, b"activation-log") {
            log(b"console-shell command: activation-log");
            let report = runtime_report();
            operator_activation_log(report);
            continue;
        }
        if bytes_eq(command, b"counter") {
            log(b"console-shell command: counter");
            let value = counter_request(b"G");
            console_write_counter(b"counter value: ", value);
            continue;
        }
        if bytes_eq(command, b"increment") {
            log(b"console-shell command: increment");
            let value = counter_request(b"I");
            console_write_counter(b"increment -> ", value);
            continue;
        }
        if bytes_eq(command, b"state-health") {
            log(b"console-shell command: state-health");
            let report = runtime_report();
            console_write_state_health(report);
            continue;
        }
        if bytes_eq(command, b"install generation gen:new") {
            log(b"console-shell command: install generation gen:new");
            console_write(b"install generation gen:new\n> ");
            yield_for_console_driver();
            log(b"console-shell requests generation-manager install");
            let status = sys::ipc_send(
                CAP_GENERATION_MANAGER_REQUEST,
                b"install gen:console-new-0002",
            );
            if status != sys::STATUS_OK {
                log(b"console-shell install generation failed");
                console_write(b"install generation failed\n> ");
                continue;
            }
            loop {
                sys::pause();
            }
        }
        if bytes_eq(command, b"install generation gen:state-bad") {
            log(b"console-shell command: install generation gen:state-bad");
            console_write(b"install generation gen:state-bad\n> ");
            yield_for_console_driver();
            log(b"console-shell requests generation-manager bad state migration install");
            let status = sys::ipc_send(
                CAP_GENERATION_MANAGER_REQUEST,
                b"install gen:state-migration-bad-0003",
            );
            if status != sys::STATUS_OK {
                log(b"console-shell bad state migration install send failed");
                console_write(b"install generation failed\n> ");
                continue;
            }
            yield_for_console_driver();
            yield_for_console_driver();
            continue;
        }
        if bytes_eq(command, b"install generation gen:state-new") {
            log(b"console-shell command: install generation gen:state-new");
            console_write(b"install generation gen:state-new\n> ");
            yield_for_console_driver();
            log(b"console-shell requests generation-manager state migration install");
            let status = sys::ipc_send(
                CAP_GENERATION_MANAGER_REQUEST,
                b"install gen:state-migration-new-0002",
            );
            if status != sys::STATUS_OK {
                log(b"console-shell state migration install failed");
                console_write(b"install generation failed\n> ");
                continue;
            }
            loop {
                sys::pause();
            }
        }
        if bytes_eq(command, b"import package pkg:logd") {
            log(b"console-shell command: import package pkg:logd");
            console_write(b"import package pkg:logd\n> ");
            yield_for_console_driver();
            log(b"console-shell requests package-import import");
            let status = sys::ipc_send(CAP_PACKAGE_IMPORT_REQUEST, b"import pkg:logd");
            if status != sys::STATUS_OK {
                log(b"console-shell package import failed");
                console_write(b"package import failed\n> ");
                continue;
            }
            loop {
                sys::pause();
            }
        }
        if bytes_eq(command, b"import package pkg:missing-dependency") {
            log(b"console-shell command: import package pkg:missing-dependency");
            console_write(b"import package pkg:missing-dependency\n> ");
            yield_for_console_driver();
            log(b"console-shell requests package-import missing-dependency validation");
            let status =
                sys::ipc_send(CAP_PACKAGE_IMPORT_REQUEST, b"import pkg:missing-dependency");
            if status != sys::STATUS_OK {
                log(b"console-shell missing-dependency validation failed");
                console_write(b"package import validation failed\n> ");
            }
            continue;
        }
        if bytes_eq(command, b"import package pkg:excess-authority") {
            log(b"console-shell command: import package pkg:excess-authority");
            console_write(b"import package pkg:excess-authority\n> ");
            yield_for_console_driver();
            log(b"console-shell requests package-import excess-authority validation");
            let status = sys::ipc_send(CAP_PACKAGE_IMPORT_REQUEST, b"import pkg:excess-authority");
            if status != sys::STATUS_OK {
                log(b"console-shell excess-authority validation failed");
                console_write(b"package import validation failed\n> ");
            }
            continue;
        }
        if bytes_eq(command, b"rollback to gen:old") {
            log(b"console-shell command: rollback to gen:old");
            let value = counter_request(b"G");
            console_write_rollback(value);
            yield_for_console_driver();
            log(b"console-shell requests generation-manager rollback");
            let status =
                sys::ipc_send(CAP_GENERATION_MANAGER_REQUEST, b"rollback gen:console-0001");
            if status != sys::STATUS_OK {
                log(b"console-shell rollback failed");
                console_write(b"rollback failed\n> ");
                continue;
            }
            loop {
                sys::pause();
            }
        }
        if bytes_eq(command, b"rollback state migration") {
            log(b"console-shell command: rollback state migration");
            console_write(b"rollback state migration\n> ");
            yield_for_console_driver();
            log(b"console-shell requests generation-manager rollback");
            let status = sys::ipc_send(
                CAP_GENERATION_MANAGER_REQUEST,
                b"rollback gen:state-migration-0001",
            );
            if status != sys::STATUS_OK {
                log(b"console-shell state migration rollback failed");
                console_write(b"rollback failed\n> ");
                continue;
            }
            loop {
                sys::pause();
            }
        }
        if bytes_eq(command, b"rollback imported package") {
            log(b"console-shell command: rollback imported package");
            console_write(b"rollback imported package\n> ");
            yield_for_console_driver();
            log(b"console-shell requests generation-manager rollback");
            let status = sys::ipc_send(
                CAP_GENERATION_MANAGER_REQUEST,
                b"rollback gen:package-import-0001",
            );
            if status != sys::STATUS_OK {
                log(b"console-shell rollback failed");
                console_write(b"rollback failed\n> ");
                continue;
            }
            loop {
                sys::pause();
            }
        }
        if starts_with(command, b"activate ") {
            log(b"console-shell command: activate");
            operator_activate(command);
            continue;
        }
        if starts_with(command, b"rollback ") {
            log(b"console-shell command: operator rollback");
            operator_rollback(command);
            continue;
        }
        if starts_with(command, b"mark-known-good ") {
            log(b"console-shell command: mark-known-good");
            let report = runtime_report();
            operator_mark_known_good(report, command);
            continue;
        }
        if bytes_eq(command, b"why svc:counter state:counter") {
            log(b"console-shell command: why counter state");
            let report = runtime_report();
            require_counter_state_authority(report);
            log(b"svc:counter has state authority from generation graph");
            console_write(b"why svc:counter state:counter\nsvc:counter has state authority from generation graph\n> ");
            continue;
        }
        if bytes_eq(command, b"halt") {
            log(b"console-shell command: halt");
            console_write(b"Native console shell ok\n");
            let _ = sys::ipc_send(CAP_COUNTER_REQUEST, b"H");
            let _ = sys::ipc_send(CAP_PACKAGE_IMPORT_REQUEST, b"shutdown");
            wait_for_state_clients_to_drain();
            shutdown_state_service();
            shutdown_generation_manager();
            if sys::ipc_send(CAP_CONSOLE_CONTROL, CONTROL_SHUTDOWN) != sys::STATUS_OK {
                log(b"console-shell shutdown send failed");
                sys::exit(1);
            }
            sys::exit(0);
        }

        log(b"console-shell unknown command");
        console_write(b"unknown command\n> ");
    }
}

fn runtime_report() -> &'static [u8] {
    let report = report_buffer();
    let mut report_len = sys::runtime_inspect(CAP_INSPECT, report);
    if report_len == sys::STATUS_BAD_CAPABILITY {
        report_len = sys::runtime_inspect(CAP_INSPECT_AFTER_PACKAGE_IMPORT, report);
    }
    if report_len == sys::STATUS_BAD_CAPABILITY
        || report_len == sys::STATUS_BAD_BUFFER
        || report_len == sys::STATUS_TOO_LARGE
        || report_len > report.len() as u64
    {
        log(b"console-shell runtime report failed");
        sys::exit(1);
    }
    &report[..report_len as usize]
}

fn report_buffer() -> &'static mut [u8; REPORT_BUFFER_LEN] {
    unsafe { &mut *REPORT_BUFFER.0.get() }
}

fn generation_for_self(report: &[u8]) -> &[u8] {
    let needles: [&[u8]; 2] = [b"name=console-shell", b" generation="];
    if let Some(line) = find_line_contains_all(report, &needles)
        && let Some(generation) = field_slice(line, b"generation=")
    {
        log(b"native shell generation query ok");
        return generation;
    }

    log(b"console-shell generation query failed");
    sys::exit(1);
}

fn console_write_generation(generation: &[u8]) {
    let mut payload = [0u8; 128];
    let mut len = 0;
    append(&mut payload, &mut len, b"current generation: ");
    append(&mut payload, &mut len, generation);
    log(&payload[..len]);
    append(&mut payload, &mut len, b"\n> ");
    console_write(&payload[..len]);
}

fn console_write_services(report: &[u8]) {
    let mut payload = [0u8; 128];
    let mut len = 0;
    append(&mut payload, &mut len, b"services:");
    let mut index = 0;
    while index < SERVICE_NAMES.len() {
        let state = process_state(report, SERVICE_NAMES[index]);
        log_service_state(SERVICE_NAMES[index], state);
        append(&mut payload, &mut len, b" ");
        append(&mut payload, &mut len, SERVICE_NAMES[index]);
        append(&mut payload, &mut len, b"=");
        append(&mut payload, &mut len, state);
        index += 1;
    }
    append(&mut payload, &mut len, b"\n> ");
    console_write(&payload[..len]);
    log(b"native shell services query ok");
}

fn console_write_devices(report: &[u8]) {
    let needles: [&[u8]; 2] = [b"virtio-device-runtime[", b"device=device:virtio-blk0"];
    let Some(line) = find_line_contains_all(report, &needles) else {
        log(b"console-shell device query failed");
        sys::exit(1);
    };
    let Some(owner) = field_slice(line, b"owner=") else {
        log(b"console-shell device owner query failed");
        sys::exit(1);
    };
    let Some(reason) = field_slice(line, b"last_error=") else {
        log(b"console-shell device error query failed");
        sys::exit(1);
    };

    let mut payload = [0u8; 128];
    let mut len = 0;
    append(&mut payload, &mut len, b"last device failure: owner=");
    append(&mut payload, &mut len, owner);
    append(&mut payload, &mut len, b" reason=");
    append(&mut payload, &mut len, reason);
    log(&payload[..len]);
    log(b"appliance shell reports last device failure reason and owner process");
    append(&mut payload, &mut len, b"\n> ");
    console_write(&payload[..len]);
}

fn console_write_state_health(report: &[u8]) {
    let policy_needles: [&[u8]; 3] = [
        b"state-policy[",
        b"id=state:counter",
        b"storage=vertexdisk-v1",
    ];
    let Some(policy_line) = find_line_contains_all(report, &policy_needles) else {
        log(b"console-shell state policy report missing");
        sys::exit(1);
    };
    let Some(storage) = field_slice(policy_line, b"storage=") else {
        log(b"console-shell state policy storage missing");
        sys::exit(1);
    };
    let Some(migration) = field_slice(policy_line, b"migration=") else {
        log(b"console-shell state policy migration missing");
        sys::exit(1);
    };
    let Some(retention) = field_slice(policy_line, b"retention=") else {
        log(b"console-shell state policy retention missing");
        sys::exit(1);
    };
    let Some(sharing) = field_slice(policy_line, b"sharing=") else {
        log(b"console-shell state policy sharing missing");
        sys::exit(1);
    };

    let health_needles: [&[u8]; 2] = [b"state-health[", b"id=state:counter"];
    let Some(line) = find_line_contains_all(report, &health_needles) else {
        log(b"console-shell state health report missing");
        sys::exit(1);
    };
    let Some(owner) = field_slice(line, b"owner=") else {
        log(b"console-shell state health owner missing");
        sys::exit(1);
    };
    let Some(schema) = field_slice(line, b"schema=") else {
        log(b"console-shell state health schema missing");
        sys::exit(1);
    };
    let Some(generation) = field_slice(line, b"generation=") else {
        log(b"console-shell state health generation missing");
        sys::exit(1);
    };
    let Some(status) = field_slice(line, b"migration_status=") else {
        log(b"console-shell state health migration status missing");
        sys::exit(1);
    };
    let Some(error) = field_slice(line, b"last_error=") else {
        log(b"console-shell state health last error missing");
        sys::exit(1);
    };

    let mut payload = [0u8; 160];
    let mut len = 0;
    append(&mut payload, &mut len, b"state-health state:counter owner=");
    append(&mut payload, &mut len, owner);
    append(&mut payload, &mut len, b" schema=");
    append(&mut payload, &mut len, schema);
    append(&mut payload, &mut len, b" generation=");
    append(&mut payload, &mut len, generation);
    append(&mut payload, &mut len, b" migration_status=");
    append(&mut payload, &mut len, status);
    append(&mut payload, &mut len, b" last_error=");
    append(&mut payload, &mut len, error);
    log(&payload[..len]);
    len = 0;
    append(
        &mut payload,
        &mut len,
        b"state-policy state:counter storage=",
    );
    append(&mut payload, &mut len, storage);
    append(&mut payload, &mut len, b" migration=");
    append(&mut payload, &mut len, migration);
    append(&mut payload, &mut len, b" retention=");
    append(&mut payload, &mut len, retention);
    append(&mut payload, &mut len, b" sharing=");
    append(&mut payload, &mut len, sharing);
    log(&payload[..len]);
    log(b"state health reports owner schema generation migration status and last error");
    console_write(b"state-health ok\n> ");
}

fn operator_current_generation(report: &[u8]) {
    let line = operator_report_line(report);
    let generation = required_field(line, b"active=", b"operator current-generation missing active");
    let policy_hash = required_field(
        line,
        b"policy_hash=",
        b"operator current-generation missing policy hash",
    );
    let graph_hash = required_field(
        line,
        b"graph_hash=",
        b"operator current-generation missing graph hash",
    );
    let mut buffer = [0u8; 256];
    let mut len = 0;
    append(&mut buffer, &mut len, b"operator current-generation generation=");
    append(&mut buffer, &mut len, generation);
    append(&mut buffer, &mut len, b" policy_hash=");
    append(&mut buffer, &mut len, policy_hash);
    append(&mut buffer, &mut len, b" graph_hash=");
    append(&mut buffer, &mut len, graph_hash);
    log(&buffer[..len]);
    console_write(b"current-generation ok\n> ");
}

fn operator_generations(report: &[u8]) {
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-generation[") {
            log(line);
            count += 1;
        }
    });
    if count == 0 {
        log(b"operator generations query failed");
        sys::exit(1);
    }
    log_count_line(b"operator generations listed=", count);
    console_write(b"generations ok\n> ");
}

fn operator_generation_status(report: &[u8]) {
    let needles: [&[u8]; 2] = [b"generation-manager v=1", b"selected="];
    let Some(manager) = find_line_contains_all(report, &needles) else {
        log(b"operator generation-status query failed");
        sys::exit(1);
    };
    let selected = required_field(manager, b"selected=", b"generation-status missing selected");
    let previous = required_field(manager, b"previous=", b"generation-status missing previous");
    let known_good =
        required_field(manager, b"known_good=", b"generation-status missing known-good");
    let transaction =
        required_field(manager, b"transaction=", b"generation-status missing transaction");
    let target = required_field(manager, b"target=", b"generation-status missing target");
    let policy_hash = active_policy_hash(report);

    let mut buffer = [0u8; 256];
    let mut len = 0;
    append(&mut buffer, &mut len, b"operator generation-status selected=");
    append(&mut buffer, &mut len, selected);
    append(&mut buffer, &mut len, b" previous=");
    append(&mut buffer, &mut len, previous);
    append(&mut buffer, &mut len, b" known_good=");
    append(&mut buffer, &mut len, known_good);
    append(&mut buffer, &mut len, b" transaction=");
    append(&mut buffer, &mut len, transaction);
    append(&mut buffer, &mut len, b" target=");
    append(&mut buffer, &mut len, target);
    append(&mut buffer, &mut len, b" policy_hash=");
    append(&mut buffer, &mut len, policy_hash);
    log(&buffer[..len]);
    console_write(b"generation-status ok\n> ");
}

fn operator_diff_generation(report: &[u8], command: &[u8], authority_only: bool) {
    let from = word_or_fail(command, 1, b"operator diff missing source generation");
    let to = word_or_fail(command, 2, b"operator diff missing target generation");
    if word_at(command, 3).is_some() {
        log(b"operator diff rejected: too many arguments");
        sys::exit(1);
    }
    require_operator_generation(report, from);
    let target_line = require_operator_generation(report, to);
    let policy_hash = required_field(target_line, b"policy_hash=", b"operator diff missing hash");

    let service_added = count_node_delta(report, from, to, b"service");
    let service_removed = count_node_delta(report, to, from, b"service");
    let state_added = count_node_delta(report, from, to, b"state-volume");
    let state_removed = count_node_delta(report, to, from, b"state-volume");
    let device_added = count_node_delta(report, from, to, b"device");
    let device_removed = count_node_delta(report, to, from, b"device");
    let capability_added = count_capability_delta(report, from, to);
    let capability_removed = count_capability_delta(report, to, from);
    let service_changed = count_service_changed(report, from, to);
    let state_changed = count_state_changed(report, from, to);
    let device_changed = count_node_changed(report, from, to, b"device");
    let capability_changed = count_capability_changed(report, from, to);

    let mut buffer = [0u8; 384];
    let mut len = 0;
    if authority_only {
        append(
            &mut buffer,
            &mut len,
            b"operator planned-authority-delta from=",
        );
    } else {
        append(&mut buffer, &mut len, b"operator diff-generation from=");
    }
    append(&mut buffer, &mut len, from);
    append(&mut buffer, &mut len, b" to=");
    append(&mut buffer, &mut len, to);
    if !authority_only {
        append(&mut buffer, &mut len, b" services=+");
        append_u64(&mut buffer, &mut len, service_added);
        append(&mut buffer, &mut len, b"-");
        append_u64(&mut buffer, &mut len, service_removed);
        append(&mut buffer, &mut len, b" changed_services=");
        append_u64(&mut buffer, &mut len, service_changed);
        append(&mut buffer, &mut len, b" packages=unavailable");
    }
    append(&mut buffer, &mut len, b" capabilities=+");
    append_u64(&mut buffer, &mut len, capability_added);
    append(&mut buffer, &mut len, b"-");
    append_u64(&mut buffer, &mut len, capability_removed);
    append(&mut buffer, &mut len, b" changed_capabilities=");
    append_u64(&mut buffer, &mut len, capability_changed);
    if !authority_only {
        append(&mut buffer, &mut len, b" state=+");
        append_u64(&mut buffer, &mut len, state_added);
        append(&mut buffer, &mut len, b"-");
        append_u64(&mut buffer, &mut len, state_removed);
        append(&mut buffer, &mut len, b" changed_state=");
        append_u64(&mut buffer, &mut len, state_changed);
        append(&mut buffer, &mut len, b" devices=+");
        append_u64(&mut buffer, &mut len, device_added);
        append(&mut buffer, &mut len, b"-");
        append_u64(&mut buffer, &mut len, device_removed);
        append(&mut buffer, &mut len, b" changed_devices=");
        append_u64(&mut buffer, &mut len, device_changed);
    }
    append(&mut buffer, &mut len, b" policy_hash=");
    append(&mut buffer, &mut len, policy_hash);
    log(&buffer[..len]);
    if authority_only {
        console_write(b"planned-authority-delta ok\n> ");
    } else {
        console_write(b"diff-generation ok\n> ");
    }
}

fn operator_why(report: &[u8], command: &[u8]) {
    let service = word_or_fail(command, 1, b"operator why missing service");
    let capability = word_or_fail(command, 2, b"operator why missing capability");
    if word_at(command, 3).is_some() {
        log(b"operator why rejected: too many arguments");
        sys::exit(1);
    }
    let generation = active_generation(report);
    let requirement = require_operator_requirement(report, generation, service, capability);
    let capability_line = require_operator_capability(report, generation, capability);
    let requirement_rights =
        required_field(requirement, b"rights=", b"operator why missing requirement rights");
    let capability_rights =
        required_field(capability_line, b"rights=", b"operator why missing capability rights");
    if !rights_cover(capability_rights, requirement_rights) {
        log(b"operator why rejected: requirement rights exceed capability rights");
        sys::exit(1);
    }
    let object = required_field(
        capability_line,
        b"object=",
        b"operator why missing capability object",
    );
    let edge = require_operator_edge(report, generation, object, requirement_rights);
    let process = operator_service_process(report, generation, service);
    require_live_capability(report, process, service, object, requirement_rights);
    let policy_hash = active_policy_hash(report);
    let provider = required_field(
        capability_line,
        b"provider=",
        b"operator why missing capability provider",
    );

    let mut buffer = [0u8; 256];
    let mut len = 0;
    append(&mut buffer, &mut len, b"operator why service=");
    append(&mut buffer, &mut len, service);
    append(&mut buffer, &mut len, b" capability=");
    append(&mut buffer, &mut len, capability);
    append(&mut buffer, &mut len, b" provider=");
    append(&mut buffer, &mut len, provider);
    append(&mut buffer, &mut len, b" rights=");
    append(&mut buffer, &mut len, requirement_rights);
    append(&mut buffer, &mut len, b" edge=");
    append(
        &mut buffer,
        &mut len,
        required_field(edge, b"id=", b"operator why missing edge id"),
    );
    append(&mut buffer, &mut len, b" generation=");
    append(&mut buffer, &mut len, generation);
    append(&mut buffer, &mut len, b" policy_hash=");
    append(&mut buffer, &mut len, policy_hash);
    log(&buffer[..len]);
    console_write(b"why ok\n> ");
}

fn operator_who_can(report: &[u8], command: &[u8]) {
    let object = word_or_fail(command, 1, b"operator who-can missing object");
    if word_at(command, 2).is_some() {
        log(b"operator who-can rejected: too many arguments");
        sys::exit(1);
    }
    let generation = active_generation(report);
    let policy_hash = active_policy_hash(report);
    if starts_with(object, b"state:") {
        let writers = log_state_writers(report, generation, object);
        if writers == 0 {
            log(b"operator who-can rejected: no graph-authorized state writers");
            sys::exit(1);
        }
        let mut buffer = [0u8; 192];
        let mut len = 0;
        append(&mut buffer, &mut len, b"operator who-can object=");
        append(&mut buffer, &mut len, object);
        append(&mut buffer, &mut len, b" writer_count=");
        append_u64(&mut buffer, &mut len, writers);
        append(&mut buffer, &mut len, b" generation=");
        append(&mut buffer, &mut len, generation);
        append(&mut buffer, &mut len, b" policy_hash=");
        append(&mut buffer, &mut len, policy_hash);
        log(&buffer[..len]);
        console_write(b"who-can ok\n> ");
        return;
    }
    if starts_with(object, b"cap:") {
        let consumers = log_capability_consumers(report, generation, object);
        let mut buffer = [0u8; 192];
        let mut len = 0;
        append(&mut buffer, &mut len, b"operator who-can object=");
        append(&mut buffer, &mut len, object);
        append(&mut buffer, &mut len, b" consumer_count=");
        append_u64(&mut buffer, &mut len, consumers);
        append(&mut buffer, &mut len, b" generation=");
        append(&mut buffer, &mut len, generation);
        append(&mut buffer, &mut len, b" policy_hash=");
        append(&mut buffer, &mut len, policy_hash);
        log(&buffer[..len]);
        console_write(b"who-can ok\n> ");
        return;
    }
    log(b"operator who-can rejected: unsupported object kind");
    sys::exit(1);
}

fn operator_which_generation(report: &[u8], command: &[u8]) {
    let selector = word_or_fail(command, 1, b"operator which-generation missing selector");
    if word_at(command, 2).is_some() {
        log(b"operator which-generation rejected: too many arguments");
        sys::exit(1);
    }
    let process = if starts_with(selector, b"svc:") {
        operator_service_process(report, active_generation(report), selector)
    } else {
        selector
    };
    let Some(line) = find_process_line(report, process) else {
        log(b"operator which-generation rejected: unknown process");
        sys::exit(1);
    };
    let generation =
        required_field(line, b"generation=", b"operator which-generation missing generation");
    let policy_hash = active_policy_hash(report);
    let mut buffer = [0u8; 192];
    let mut len = 0;
    append(&mut buffer, &mut len, b"operator which-generation selector=");
    append(&mut buffer, &mut len, selector);
    append(&mut buffer, &mut len, b" process=");
    append(&mut buffer, &mut len, process);
    append(&mut buffer, &mut len, b" generation=");
    append(&mut buffer, &mut len, generation);
    append(&mut buffer, &mut len, b" policy_hash=");
    append(&mut buffer, &mut len, policy_hash);
    log(&buffer[..len]);
    console_write(b"which-generation ok\n> ");
}

fn operator_package_list(report: &[u8]) {
    let generation = active_generation(report);
    let generation_line = require_operator_generation(report, generation);
    let facts =
        required_field(generation_line, b"package_facts=", b"operator package-list missing facts");
    if !bytes_eq(facts, b"absent") {
        log(b"operator package-list rejected: unsupported package fact encoding");
        sys::exit(1);
    }
    log(b"operator package-list unavailable: no native package facts");
    console_write(b"package-list unavailable\n> ");
}

fn operator_activation_log(report: &[u8]) {
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"service-lifecycle[") {
            log(line);
            count += 1;
        }
    });
    if count == 0 {
        log(b"operator activation-log rejected: no lifecycle records");
        sys::exit(1);
    }
    log_count_line(b"operator activation-log records=", count);
    console_write(b"activation-log ok\n> ");
}

fn operator_activate(command: &[u8]) {
    let generation = word_or_fail(command, 1, b"operator activate missing generation");
    if word_at(command, 2).is_some() {
        log(b"operator activate rejected: too many arguments");
        sys::exit(1);
    }
    let mut request = [0u8; 96];
    let mut len = 0;
    append(&mut request, &mut len, b"install ");
    append(&mut request, &mut len, generation);
    console_write(b"activate requested\n> ");
    yield_for_console_driver();
    log_prefix(b"operator activate queues generation-manager install: generation=", generation);
    if sys::ipc_send(CAP_GENERATION_MANAGER_REQUEST, &request[..len]) != sys::STATUS_OK {
        log(b"operator activate send failed");
        sys::exit(1);
    }
    loop {
        sys::pause();
    }
}

fn operator_rollback(command: &[u8]) {
    let generation = word_or_fail(command, 1, b"operator rollback missing generation");
    if word_at(command, 2).is_some() {
        log(b"operator rollback rejected: too many arguments");
        sys::exit(1);
    }
    let mut request = [0u8; 96];
    let mut len = 0;
    append(&mut request, &mut len, b"rollback ");
    append(&mut request, &mut len, generation);
    console_write(b"rollback requested\n> ");
    yield_for_console_driver();
    log_prefix(b"operator rollback queues generation-manager rollback: generation=", generation);
    if sys::ipc_send(CAP_GENERATION_MANAGER_REQUEST, &request[..len]) != sys::STATUS_OK {
        log(b"operator rollback send failed");
        sys::exit(1);
    }
    loop {
        sys::pause();
    }
}

fn operator_mark_known_good(report: &[u8], command: &[u8]) {
    let generation = word_or_fail(command, 1, b"operator mark-known-good missing generation");
    if word_at(command, 2).is_some() {
        log(b"operator mark-known-good rejected: too many arguments");
        sys::exit(1);
    }
    let active = active_generation(report);
    if !bytes_eq(active, generation) {
        log(b"operator mark-known-good rejected: target is not active generation");
        sys::exit(1);
    }
    let mut request = [0u8; 96];
    let mut len = 0;
    append(&mut request, &mut len, b"mark-known-good ");
    append(&mut request, &mut len, generation);
    log_prefix(
        b"operator mark-known-good queues generation-manager command: generation=",
        generation,
    );
    if sys::ipc_send(CAP_GENERATION_MANAGER_REQUEST, &request[..len]) != sys::STATUS_OK {
        log(b"operator mark-known-good send failed");
        sys::exit(1);
    }
    yield_for_console_driver();
    console_write(b"mark-known-good requested\n> ");
}

fn operator_report_line(report: &[u8]) -> &[u8] {
    let needles: [&[u8]; 2] = [b"operator-report v=1", b"active="];
    let Some(line) = find_line_contains_all(report, &needles) else {
        log(b"operator report missing");
        sys::exit(1);
    };
    line
}

fn active_generation(report: &[u8]) -> &[u8] {
    required_field(
        operator_report_line(report),
        b"active=",
        b"operator report missing active generation",
    )
}

fn active_policy_hash(report: &[u8]) -> &[u8] {
    required_field(
        operator_report_line(report),
        b"policy_hash=",
        b"operator report missing policy hash",
    )
}

fn require_operator_generation<'a>(report: &'a [u8], generation: &[u8]) -> &'a [u8] {
    let Some(line) = find_line_where(report, |line| {
        starts_with(line, b"operator-generation[") && field_eq(line, b"id=", generation)
    }) else {
        log(b"operator rejected: unknown generation");
        sys::exit(1);
    };
    line
}

fn require_operator_requirement<'a>(
    report: &'a [u8],
    generation: &[u8],
    service: &[u8],
    capability: &[u8],
) -> &'a [u8] {
    let Some(line) = find_line_where(report, |line| {
        starts_with(line, b"operator-requirement[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"service=", service)
            && field_eq(line, b"capability=", capability)
    }) else {
        log(b"operator rejected: missing policy requirement");
        sys::exit(1);
    };
    line
}

fn require_operator_capability<'a>(
    report: &'a [u8],
    generation: &[u8],
    capability: &[u8],
) -> &'a [u8] {
    let Some(line) = find_line_where(report, |line| {
        starts_with(line, b"operator-capability[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"id=", capability)
    }) else {
        log(b"operator rejected: missing policy capability");
        sys::exit(1);
    };
    line
}

fn require_operator_edge<'a>(
    report: &'a [u8],
    generation: &[u8],
    object: &[u8],
    required_rights: &[u8],
) -> &'a [u8] {
    let Some(line) = find_line_where(report, |line| {
        if !(starts_with(line, b"operator-edge[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"kind=", b"capability")
            && field_eq(line, b"to=", object))
        {
            return false;
        }
        field_slice(line, b"rights=")
            .is_some_and(|edge_rights| rights_cover(edge_rights, required_rights))
    }) else {
        log(b"operator rejected: missing graph capability edge");
        sys::exit(1);
    };
    line
}

fn operator_service_process<'a>(
    report: &'a [u8],
    generation: &[u8],
    service: &[u8],
) -> &'a [u8] {
    let Some(line) = find_line_where(report, |line| {
        starts_with(line, b"operator-service[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"id=", service)
    }) else {
        log(b"operator rejected: unknown service");
        sys::exit(1);
    };
    required_field(line, b"process=", b"operator service missing process")
}

fn require_live_capability(
    report: &[u8],
    process: &[u8],
    service: &[u8],
    object: &[u8],
    required_rights: &[u8],
) {
    let generation = active_generation(report);
    let mut accepted = false;
    for_each_line(report, |line| {
        if starts_with(line, b"space=")
            && field_eq(line, b"proc=", process)
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"graph_from=", service)
            && field_eq(line, b"graph_target=", object)
            && field_eq(line, b"revoked=", b"no")
            && let Some(rights) = field_slice(line, b"rights=")
            && rights_cover(rights, required_rights)
        {
            accepted = true;
        }
    });
    if !accepted {
        log(b"operator rejected: live capability missing or insufficient");
        sys::exit(1);
    }
}

fn log_state_writers(report: &[u8], generation: &[u8], state: &[u8]) -> u64 {
    let mut writers = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-state-path[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"state=", state)
            && let Some(rights) = field_slice(line, b"rights=")
            && right_present(rights, b"write")
        {
            let service = required_field(line, b"service=", b"operator state path missing service");
            let mut buffer = [0u8; 192];
            let mut len = 0;
            append(&mut buffer, &mut len, b"operator who-can writer service=");
            append(&mut buffer, &mut len, service);
            append(&mut buffer, &mut len, b" state=");
            append(&mut buffer, &mut len, state);
            append(&mut buffer, &mut len, b" rights=");
            append(&mut buffer, &mut len, rights);
            log(&buffer[..len]);
            writers += 1;
        }
    });
    writers
}

fn log_capability_consumers(report: &[u8], generation: &[u8], capability: &[u8]) -> u64 {
    let mut consumers = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-requirement[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"capability=", capability)
        {
            let service = required_field(line, b"service=", b"operator requirement missing service");
            let rights = required_field(line, b"rights=", b"operator requirement missing rights");
            let mut buffer = [0u8; 192];
            let mut len = 0;
            append(
                &mut buffer,
                &mut len,
                b"operator who-can consumer service=",
            );
            append(&mut buffer, &mut len, service);
            append(&mut buffer, &mut len, b" capability=");
            append(&mut buffer, &mut len, capability);
            append(&mut buffer, &mut len, b" rights=");
            append(&mut buffer, &mut len, rights);
            log(&buffer[..len]);
            consumers += 1;
        }
    });
    consumers
}

fn count_node_delta(report: &[u8], from: &[u8], to: &[u8], kind: &[u8]) -> u64 {
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-node[")
            && field_eq(line, b"generation=", to)
            && field_eq(line, b"kind=", kind)
            && let Some(id) = field_slice(line, b"id=")
            && !operator_node_exists(report, from, kind, id)
        {
            count += 1;
        }
    });
    count
}

fn count_capability_delta(report: &[u8], from: &[u8], to: &[u8]) -> u64 {
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-capability[")
            && field_eq(line, b"generation=", to)
            && let Some(id) = field_slice(line, b"id=")
            && !operator_capability_exists(report, from, id)
        {
            count += 1;
        }
    });
    count
}

fn count_service_changed(report: &[u8], from: &[u8], to: &[u8]) -> u64 {
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-node[")
            && field_eq(line, b"generation=", to)
            && field_eq(line, b"kind=", b"service")
            && let Some(id) = field_slice(line, b"id=")
            && let Some(previous_node) = operator_node_line(report, from, b"service", id)
        {
            let previous_service = require_operator_service_line(report, from, id);
            let current_service = require_operator_service_line(report, to, id);
            if !operator_node_semantically_equal(previous_node, line)
                || !operator_service_semantically_equal(previous_service, current_service)
            {
                count += 1;
            }
        }
    });
    count
}

fn count_state_changed(report: &[u8], from: &[u8], to: &[u8]) -> u64 {
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-node[")
            && field_eq(line, b"generation=", to)
            && field_eq(line, b"kind=", b"state-volume")
            && let Some(id) = field_slice(line, b"id=")
            && let Some(previous_node) = operator_node_line(report, from, b"state-volume", id)
        {
            let previous_state = require_operator_state_line(report, from, id);
            let current_state = require_operator_state_line(report, to, id);
            if !operator_node_semantically_equal(previous_node, line)
                || !operator_state_semantically_equal(previous_state, current_state)
            {
                count += 1;
            }
        }
    });
    count
}

fn count_node_changed(report: &[u8], from: &[u8], to: &[u8], kind: &[u8]) -> u64 {
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-node[")
            && field_eq(line, b"generation=", to)
            && field_eq(line, b"kind=", kind)
            && let Some(id) = field_slice(line, b"id=")
            && let Some(previous) = operator_node_line(report, from, kind, id)
            && !operator_node_semantically_equal(previous, line)
        {
            count += 1;
        }
    });
    count
}

fn count_capability_changed(report: &[u8], from: &[u8], to: &[u8]) -> u64 {
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-capability[")
            && field_eq(line, b"generation=", to)
            && let Some(id) = field_slice(line, b"id=")
            && let Some(previous) = operator_capability_line(report, from, id)
            && !operator_capability_semantically_equal(previous, line)
        {
            count += 1;
        }
    });
    count
}

fn operator_node_line<'a>(
    report: &'a [u8],
    generation: &[u8],
    kind: &[u8],
    id: &[u8],
) -> Option<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-node[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"kind=", kind)
            && field_eq(line, b"id=", id)
    })
}

fn operator_capability_line<'a>(
    report: &'a [u8],
    generation: &[u8],
    id: &[u8],
) -> Option<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-capability[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"id=", id)
    })
}

fn operator_service_line<'a>(
    report: &'a [u8],
    generation: &[u8],
    id: &[u8],
) -> Option<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-service[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"id=", id)
    })
}

fn operator_state_line<'a>(
    report: &'a [u8],
    generation: &[u8],
    id: &[u8],
) -> Option<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-state[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"id=", id)
    })
}

fn require_operator_service_line<'a>(
    report: &'a [u8],
    generation: &[u8],
    id: &[u8],
) -> &'a [u8] {
    let Some(line) = operator_service_line(report, generation, id) else {
        log(b"operator diff rejected: service fact missing");
        sys::exit(1);
    };
    line
}

fn require_operator_state_line<'a>(
    report: &'a [u8],
    generation: &[u8],
    id: &[u8],
) -> &'a [u8] {
    let Some(line) = operator_state_line(report, generation, id) else {
        log(b"operator diff rejected: state fact missing");
        sys::exit(1);
    };
    line
}

fn operator_node_exists(report: &[u8], generation: &[u8], kind: &[u8], id: &[u8]) -> bool {
    operator_node_line(report, generation, kind, id).is_some()
}

fn operator_capability_exists(report: &[u8], generation: &[u8], id: &[u8]) -> bool {
    operator_capability_line(report, generation, id)
    .is_some()
}

fn operator_node_semantically_equal(left: &[u8], right: &[u8]) -> bool {
    field_pair_eq(left, right, b"kind=", b"operator node missing kind")
        && field_pair_eq(left, right, b"id=", b"operator node missing id")
        && field_pair_eq(
            left,
            right,
            b"object_kind=",
            b"operator node missing object kind",
        )
        && field_pair_eq(left, right, b"label=", b"operator node missing label")
}

fn operator_capability_semantically_equal(left: &[u8], right: &[u8]) -> bool {
    field_pair_eq(
        left,
        right,
        b"id=",
        b"operator capability missing id",
    ) && field_pair_eq(
        left,
        right,
        b"provider=",
        b"operator capability missing provider",
    ) && field_pair_eq(
        left,
        right,
        b"object_kind=",
        b"operator capability missing object kind",
    ) && field_pair_eq(
        left,
        right,
        b"object=",
        b"operator capability missing object",
    ) && rights_pair_eq(
        left,
        right,
        b"operator capability missing rights",
    )
}

fn operator_service_semantically_equal(left: &[u8], right: &[u8]) -> bool {
    field_pair_eq(left, right, b"id=", b"operator service missing id")
        && field_pair_eq(left, right, b"process=", b"operator service missing process")
        && field_pair_eq(left, right, b"restart=", b"operator service missing restart")
        && field_pair_eq(
            left,
            right,
            b"mount_root=",
            b"operator service missing mount root",
        )
}

fn operator_state_semantically_equal(left: &[u8], right: &[u8]) -> bool {
    field_pair_eq(left, right, b"id=", b"operator state missing id")
        && field_pair_eq(left, right, b"owner=", b"operator state missing owner")
        && field_pair_eq(left, right, b"schema=", b"operator state missing schema")
        && field_pair_eq(left, right, b"storage=", b"operator state missing storage")
        && field_pair_eq(
            left,
            right,
            b"migration=",
            b"operator state missing migration",
        )
        && field_pair_eq(left, right, b"retention=", b"operator state missing retention")
        && field_pair_eq(left, right, b"sharing=", b"operator state missing sharing")
}

fn field_pair_eq(left: &[u8], right: &[u8], prefix: &[u8], message: &[u8]) -> bool {
    bytes_eq(
        required_field(left, prefix, message),
        required_field(right, prefix, message),
    )
}

fn rights_pair_eq(left: &[u8], right: &[u8], message: &[u8]) -> bool {
    let left_rights = required_field(left, b"rights=", message);
    let right_rights = required_field(right, b"rights=", message);
    rights_cover(left_rights, right_rights) && rights_cover(right_rights, left_rights)
}

fn find_process_line<'a>(report: &'a [u8], process: &[u8]) -> Option<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"process[") && field_eq(line, b"name=", process)
    })
}

fn word_or_fail<'a>(command: &'a [u8], index: usize, message: &[u8]) -> &'a [u8] {
    let Some(word) = word_at(command, index) else {
        log(message);
        sys::exit(1);
    };
    word
}

fn word_at(command: &[u8], requested: usize) -> Option<&[u8]> {
    let mut cursor = 0;
    let mut index = 0;
    while cursor < command.len() {
        while cursor < command.len() && command[cursor] == b' ' {
            cursor += 1;
        }
        if cursor == command.len() {
            return None;
        }
        let start = cursor;
        while cursor < command.len() && command[cursor] != b' ' {
            cursor += 1;
        }
        if index == requested {
            return Some(&command[start..cursor]);
        }
        index += 1;
    }
    None
}

fn required_field<'a>(line: &'a [u8], prefix: &[u8], message: &[u8]) -> &'a [u8] {
    let Some(value) = field_slice(line, prefix) else {
        log(message);
        sys::exit(1);
    };
    value
}

fn rights_cover(available: &[u8], required: &[u8]) -> bool {
    if bytes_eq(required, b"none") {
        return true;
    }
    let mut start = 0;
    while start <= required.len() {
        let mut end = start;
        while end < required.len() && required[end] != b'|' {
            end += 1;
        }
        if end == start || !right_present(available, &required[start..end]) {
            return false;
        }
        if end == required.len() {
            break;
        }
        start = end + 1;
    }
    true
}

fn right_present(rights: &[u8], right: &[u8]) -> bool {
    let mut start = 0;
    while start <= rights.len() {
        let mut end = start;
        while end < rights.len() && rights[end] != b'|' {
            end += 1;
        }
        if bytes_eq(&rights[start..end], right) {
            return true;
        }
        if end == rights.len() {
            break;
        }
        start = end + 1;
    }
    false
}

fn find_line_where<'a, F>(haystack: &'a [u8], mut predicate: F) -> Option<&'a [u8]>
where
    F: FnMut(&[u8]) -> bool,
{
    let mut start = 0;
    while start <= haystack.len() {
        let mut end = start;
        while end < haystack.len() && haystack[end] != b'\n' {
            end += 1;
        }
        let line = &haystack[start..end];
        if predicate(line) {
            return Some(line);
        }
        if end == haystack.len() {
            break;
        }
        start = end + 1;
    }
    None
}

fn for_each_line<F>(haystack: &[u8], mut visit: F)
where
    F: FnMut(&[u8]),
{
    let mut start = 0;
    while start <= haystack.len() {
        let mut end = start;
        while end < haystack.len() && haystack[end] != b'\n' {
            end += 1;
        }
        visit(&haystack[start..end]);
        if end == haystack.len() {
            break;
        }
        start = end + 1;
    }
}

fn log_count_line(prefix: &[u8], count: u64) {
    let mut buffer = [0u8; 96];
    let mut len = 0;
    append(&mut buffer, &mut len, prefix);
    append_u64(&mut buffer, &mut len, count);
    log(&buffer[..len]);
}

fn log_prefix(prefix: &[u8], value: &[u8]) {
    let mut buffer = [0u8; 192];
    let mut len = 0;
    append(&mut buffer, &mut len, prefix);
    append(&mut buffer, &mut len, value);
    log(&buffer[..len]);
}

fn append_u64(buffer: &mut [u8], len: &mut usize, value: u64) {
    if value == 0 {
        append(buffer, len, b"0");
        return;
    }
    let mut digits = [0u8; 20];
    let mut digit_count = 0;
    let mut remaining = value;
    while remaining > 0 {
        digits[digit_count] = b'0' + (remaining % 10) as u8;
        digit_count += 1;
        remaining /= 10;
    }
    while digit_count > 0 {
        digit_count -= 1;
        if *len >= buffer.len() {
            log(b"console-shell payload too large");
            sys::exit(1);
        }
        buffer[*len] = digits[digit_count];
        *len += 1;
    }
}

fn counter_request(request: &[u8]) -> &[u8] {
    if sys::ipc_send(CAP_COUNTER_REQUEST, request) != sys::STATUS_OK {
        log(b"console-shell counter request failed");
        sys::exit(1);
    }
    let mut attempts = 0;
    loop {
        let buffer = report_buffer();
        let received = sys::ipc_recv(CAP_COUNTER_REPLY, buffer);
        if received == sys::STATUS_EMPTY && attempts < 64 {
            attempts += 1;
            sys::yield_now();
            continue;
        }
        if received == sys::STATUS_BAD_CAPABILITY
            || received == sys::STATUS_BAD_BUFFER
            || received == sys::STATUS_TOO_LARGE
            || received > buffer.len() as u64
        {
            log(b"console-shell counter reply failed");
            sys::exit(1);
        }
        return &buffer[..received as usize];
    }
}

fn console_write_counter(prefix: &[u8], value: &[u8]) {
    let mut payload = [0u8; 128];
    let mut len = 0;
    append(&mut payload, &mut len, prefix);
    append(&mut payload, &mut len, value);
    append(&mut payload, &mut len, b"\n> ");
    console_write(&payload[..len]);
}

fn shutdown_state_service() {
    let mut control = sys::vfs_open_path_write(CAP_STATE_CONTROL, STATE_CONTROL_PATH);
    if status_is_error(control) {
        control =
            sys::vfs_open_path_write(CAP_STATE_CONTROL_AFTER_PACKAGE_IMPORT, STATE_CONTROL_PATH);
    }
    if status_is_error(control)
        || sys::vfs_write(control, b"Q") != 1
        || sys::vfs_close(control) != sys::STATUS_OK
    {
        log(b"console-shell state shutdown failed");
        sys::exit(1);
    }
    log(b"console-shell requested state shutdown");
}

fn wait_for_state_clients_to_drain() {
    log(b"console-shell waits for state clients to drain");
    let mut stable_drained = false;
    let mut attempt = 0;
    while attempt < STATE_CLIENT_DRAIN_ATTEMPTS {
        let report = runtime_report();
        if state_clients_drained(report) {
            if stable_drained {
                log(b"console-shell observed state clients drained");
                return;
            }
            stable_drained = true;
        } else {
            stable_drained = false;
        }
        let _ = sys::yield_now();
        attempt += 1;
    }

    log(b"console-shell state client drain timed out");
    sys::exit(1);
}

fn state_clients_drained(report: &[u8]) -> bool {
    let mut start = 0;
    while start <= report.len() {
        let mut end = start;
        while end < report.len() && report[end] != b'\n' {
            end += 1;
        }
        let line = &report[start..end];
        if let Some(name) = state_client_name(line)
            && state_client_requires_drain(name)
            && !state_client_drained(report, name)
        {
            return false;
        }
        if end == report.len() {
            break;
        }
        start = end + 1;
    }
    true
}

fn state_client_name(line: &[u8]) -> Option<&[u8]> {
    if state_client_cap_line(line) {
        return field_slice(line, b"proc=");
    }
    if state_client_process_line(line) {
        return field_slice(line, b"name=");
    }
    None
}

fn state_client_drained(report: &[u8], name: &[u8]) -> bool {
    if !bytes_eq(process_state(report, name), b"exited") {
        return false;
    }
    if bytes_eq(process_restart_policy(report, name), b"always")
        && !service_lifecycle_seen(report, name, b"restarting")
    {
        return false;
    }
    true
}

fn state_client_process_line(line: &[u8]) -> bool {
    if !starts_with(line, b"process[") {
        return false;
    }
    let Some(root) = field_slice(line, b"mount_root=") else {
        return false;
    };
    bytes_eq(root, b"/state") || starts_with(root, b"/state/")
}

fn state_client_cap_line(line: &[u8]) -> bool {
    if !starts_with(line, b"space=initial proc=") || find_subslice(line, b"vfs-root=").is_none() {
        return false;
    }
    let Some(root) = field_slice(line, b"root=") else {
        return false;
    };
    bytes_eq(root, b"/state") || starts_with(root, b"/state/")
}

fn state_client_requires_drain(name: &[u8]) -> bool {
    !bytes_eq(name, b"console-shell") && !bytes_eq(name, b"vertex-state")
}

fn service_lifecycle_seen(report: &[u8], service: &[u8], state: &[u8]) -> bool {
    let needles: [&[u8]; 3] = [b"service-lifecycle[", b" service=", b" state="];
    let mut start = 0;
    while start <= report.len() {
        let mut end = start;
        while end < report.len() && report[end] != b'\n' {
            end += 1;
        }
        let line = &report[start..end];
        if contains_all(line, &needles)
            && field_eq(line, b"service=", service)
            && field_eq(line, b"state=", state)
        {
            return true;
        }
        if end == report.len() {
            break;
        }
        start = end + 1;
    }
    false
}

fn shutdown_generation_manager() {
    if sys::ipc_send(CAP_GENERATION_MANAGER_REQUEST, GENERATION_MANAGER_SHUTDOWN) != sys::STATUS_OK
    {
        log(b"console-shell generation-manager shutdown failed");
        sys::exit(1);
    }
}

fn console_write_rollback(value: &[u8]) {
    let mut payload = [0u8; 128];
    let mut len = 0;
    append(
        &mut payload,
        &mut len,
        b"rollback to gen:old\ncounter state policy: preserve\ncounter value: ",
    );
    append(&mut payload, &mut len, value);
    append(&mut payload, &mut len, b"\n> ");
    console_write(&payload[..len]);
}

fn status_is_error(value: u64) -> bool {
    value >= u64::MAX - 4096
}

fn yield_for_console_driver() {
    let mut attempts = 0;
    while attempts < 8 {
        sys::yield_now();
        attempts += 1;
    }
}

fn process_state<'a>(report: &'a [u8], name: &[u8]) -> &'a [u8] {
    let mut start = 0;
    while start <= report.len() {
        let mut end = start;
        while end < report.len() && report[end] != b'\n' {
            end += 1;
        }
        let line = &report[start..end];
        if starts_with(line, b"process[")
            && field_eq(line, b"name=", name)
            && let Some(state) = field_slice(line, b"state=")
        {
            return state;
        }
        if end == report.len() {
            break;
        }
        start = end + 1;
    }

    log(b"console-shell services query failed");
    sys::exit(1);
}

fn process_restart_policy<'a>(report: &'a [u8], name: &[u8]) -> &'a [u8] {
    let mut start = 0;
    while start <= report.len() {
        let mut end = start;
        while end < report.len() && report[end] != b'\n' {
            end += 1;
        }
        let line = &report[start..end];
        if starts_with(line, b"process[")
            && field_eq(line, b"name=", name)
            && let Some(policy) = field_slice(line, b"restart_policy=")
        {
            return policy;
        }
        if end == report.len() {
            break;
        }
        start = end + 1;
    }

    log(b"console-shell services query failed");
    sys::exit(1);
}

fn require_echo_log_authority(report: &[u8]) {
    let needles: [&[u8]; 6] = [
        b"space=initial proc=echo cap[0] endpoint=log-sink",
        b"rights=send",
        b"parent_cap_id=",
        b"owner=echo",
        b"delegated_by=vertex-init",
        b"revoked=no",
    ];
    if let Some(line) = find_line_contains_all(report, &needles)
        && field_u64(line, b"parent_cap_id=").unwrap_or(0) != 0
    {
        log(b"native shell why query ok");
        log(b"console-shell why result: svc:echo cap:log.sink send slot 0");
        return;
    }

    log(b"console-shell why query failed");
    sys::exit(1);
}

fn require_counter_state_authority(report: &[u8]) {
    let needles: [&[u8]; 6] = [
        b"space=initial proc=counter-service cap[4]",
        b"vfs-root=cap:vfs.counter-state",
        b"root=/state/counter",
        b"rights=read|write|resolve",
        b"owner=counter-service",
        b"revoked=no",
    ];
    if find_line_contains_all(report, &needles).is_some() {
        log(b"native shell why counter state query ok");
        return;
    }

    log(b"console-shell why counter state query failed");
    sys::exit(1);
}

fn console_write(payload: &[u8]) {
    if payload.len() > 128 {
        log(b"console-shell payload too large");
        sys::exit(1);
    }
    if sys::ipc_send(CAP_CONSOLE_OUTPUT, payload) != sys::STATUS_OK {
        log(b"console-shell console write failed");
        sys::exit(1);
    }
}

fn append(buffer: &mut [u8], len: &mut usize, value: &[u8]) {
    let mut index = 0;
    while index < value.len() {
        if *len >= buffer.len() {
            log(b"console-shell payload too large");
            sys::exit(1);
        }
        buffer[*len] = value[index];
        *len += 1;
        index += 1;
    }
}

fn log(value: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, value) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn log_service_state(name: &[u8], state: &[u8]) {
    let mut buffer = [0u8; 128];
    let mut len = 0;
    append(&mut buffer, &mut len, b"console-shell service state: ");
    append(&mut buffer, &mut len, name);
    append(&mut buffer, &mut len, b"=");
    append(&mut buffer, &mut len, state);
    log(&buffer[..len]);
}

fn send_ready() {
    let ready = ready_message(b"console-shell");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"console-shell ready send failed");
        sys::exit(1);
    }
}

fn ready_message(service: &[u8]) -> [u8; 32] {
    let mut message = [0u8; 32];
    write_u16(&mut message, 0, PROTOCOL_HEALTH_V0);
    write_u16(&mut message, 2, MESSAGE_READY);
    write_u32(&mut message, 4, service.len() as u32);
    write_u64(&mut message, 8, 1);
    let mut index = 0;
    while index < service.len() && ENVELOPE_LEN + index < message.len() {
        message[ENVELOPE_LEN + index] = service[index];
        index += 1;
    }
    message
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

fn contains_all(haystack: &[u8], needles: &[&[u8]]) -> bool {
    let mut index = 0;
    while index < needles.len() {
        if find_subslice(haystack, needles[index]).is_none() {
            return false;
        }
        index += 1;
    }
    true
}

fn field_eq(line: &[u8], prefix: &[u8], expected: &[u8]) -> bool {
    field_slice(line, prefix).is_some_and(|value| bytes_eq(value, expected))
}

fn field_slice<'a>(line: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    let start = find_subslice(line, prefix)? + prefix.len();
    let mut end = start;
    while end < line.len() && line[end] != b' ' && line[end] != b'\n' {
        end += 1;
    }
    Some(&line[start..end])
}

fn field_u64(line: &[u8], prefix: &[u8]) -> Option<u64> {
    let value = field_slice(line, prefix)?;
    let mut out = 0u64;
    if value.is_empty() {
        return None;
    }
    let mut index = 0;
    while index < value.len() {
        let byte = value[index];
        if !byte.is_ascii_digit() {
            return None;
        }
        out = out.checked_mul(10)?.checked_add((byte - b'0') as u64)?;
        index += 1;
    }
    Some(out)
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    let mut start = 0;
    while start + needle.len() <= haystack.len() {
        let mut index = 0;
        while index < needle.len() && haystack[start + index] == needle[index] {
            index += 1;
        }
        if index == needle.len() {
            return Some(start);
        }
        start += 1;
    }
    None
}

fn starts_with(value: &[u8], prefix: &[u8]) -> bool {
    if value.len() < prefix.len() {
        return false;
    }
    let mut index = 0;
    while index < prefix.len() {
        if value[index] != prefix[index] {
            return false;
        }
        index += 1;
    }
    true
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

fn write_u16(buffer: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    buffer[offset] = bytes[0];
    buffer[offset + 1] = bytes[1];
}

fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    buffer[offset] = bytes[0];
    buffer[offset + 1] = bytes[1];
    buffer[offset + 2] = bytes[2];
    buffer[offset + 3] = bytes[3];
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        buffer[offset + index] = bytes[index];
        index += 1;
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
