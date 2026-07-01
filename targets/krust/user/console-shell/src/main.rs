#![no_std]
#![no_main]

mod sys;

use core::{cell::UnsafeCell, panic::PanicInfo};
use vertex_operator_shell as operator_shell;

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
const COMMAND_BUFFER_LEN: usize = 160;
const REPORT_BUFFER_LEN: usize = 256 * 1024;
const CONTROL_SHUTDOWN: &[u8] = b"shutdown";
const GENERATION_MANAGER_SHUTDOWN: &[u8] = b"shutdown";
const STATE_CONTROL_PATH: &[u8] = b"/state/counter/control";
const STATE_CLIENT_DRAIN_ATTEMPTS: u64 = 4096;
const CONSOLE_WRITE_ATTEMPTS: u64 = 4096;
const IPC_SEND_ATTEMPTS: u64 = 4096;
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
        let mut command = [0u8; COMMAND_BUFFER_LEN];
        let received = sys::ipc_recv(CAP_SHELL_REQUEST, &mut command);
        if received == sys::STATUS_BAD_CAPABILITY || received > command.len() as u64 {
            log(b"console-shell command receive failed");
            sys::exit(1);
        }
        let mut normalized_command = [0u8; COMMAND_BUFFER_LEN];
        let command =
            normalize_command_verb(&command[..received as usize], &mut normalized_command);
        if command.is_empty() {
            console_write(b"> ");
            continue;
        }
        if command_verb_eq(command, b"help") {
            log(b"console-shell command: help");
            operator_help(command);
            continue;
        }
        if bytes_eq(command, b"overview") {
            log(b"console-shell command: overview");
            let report = runtime_report();
            operator_overview(report);
            continue;
        }
        if bytes_eq(command, b"services") {
            log(b"console-shell command: services");
            let report = runtime_report();
            operator_services(report);
            continue;
        }
        if command_verb_eq(command, b"service") {
            if !require_word_count(command, 2, b"usage: service <service-or-process>\n> ") {
                continue;
            }
            log(b"console-shell command: service");
            let report = runtime_report();
            operator_service_detail(report, command);
            continue;
        }
        if command_verb_eq(command, b"capabilities") {
            let words = command_word_count(command);
            if words != 1 && words != 3 {
                console_write(b"usage: capabilities [for <service-or-process>]\n> ");
                continue;
            }
            log(b"console-shell command: capabilities");
            let report = runtime_report();
            operator_capabilities(report, command);
            continue;
        }
        if command_verb_eq(command, b"capability") {
            if !require_word_count(command, 2, b"usage: capability <capability-id>\n> ") {
                continue;
            }
            log(b"console-shell command: capability");
            let report = runtime_report();
            operator_capability_detail(report, command);
            continue;
        }
        if bytes_eq(command, b"states") {
            log(b"console-shell command: states");
            let report = runtime_report();
            operator_states(report);
            continue;
        }
        if command_verb_eq(command, b"state") {
            if !require_word_count(command, 2, b"usage: state <state-id>\n> ") {
                continue;
            }
            log(b"console-shell command: state");
            let report = runtime_report();
            operator_state_detail(report, command);
            continue;
        }
        if bytes_eq(command, b"devices") {
            log(b"console-shell command: devices");
            let report = runtime_report();
            operator_devices(report);
            continue;
        }
        if command_verb_eq(command, b"device") {
            if !require_word_count(command, 2, b"usage: device <device-id>\n> ") {
                continue;
            }
            log(b"console-shell command: device");
            let report = runtime_report();
            operator_device_detail(report, command);
            continue;
        }
        if bytes_eq(command, b"device-failures") {
            log(b"console-shell command: device-failures");
            let report = runtime_report();
            console_write_device_failure(report);
            console_write(b"> ");
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
        if bytes_eq(command, b"why svc:echo cap:log.sink") {
            log(b"console-shell command: why");
            let report = runtime_report();
            require_echo_log_authority(report);
            console_write(
                b"svc:echo has send authority because generation graph granted cap slot 0\n> ",
            );
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
        if command_verb_eq(command, b"diff-generation") {
            if !require_word_count(command, 3, b"usage: diff-generation <from> <to>\n> ") {
                continue;
            }
            log(b"console-shell command: diff-generation");
            let report = runtime_report();
            operator_diff_generation(report, command, false);
            continue;
        }
        if command_verb_eq(command, b"planned-authority-delta") {
            if !require_word_count(
                command,
                3,
                b"usage: planned-authority-delta <from> <to>\n> ",
            ) {
                continue;
            }
            log(b"console-shell command: planned-authority-delta");
            let report = runtime_report();
            operator_diff_generation(report, command, true);
            continue;
        }
        if command_verb_eq(command, b"why") {
            if !require_word_count(command, 3, b"usage: why <service> <capability>\n> ") {
                continue;
            }
            log(b"console-shell command: operator why");
            let report = runtime_report();
            operator_why(report, command);
            continue;
        }
        if command_verb_eq(command, b"who-can") {
            if !require_word_count(command, 2, b"usage: who-can <object>\n> ") {
                continue;
            }
            log(b"console-shell command: who-can");
            let report = runtime_report();
            operator_who_can(report, command);
            continue;
        }
        if command_verb_eq(command, b"which-generation") {
            if !require_word_count(command, 2, b"usage: which-generation <process>\n> ") {
                continue;
            }
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
            let Some(value) = counter_request(b"G") else {
                console_write(b"counter request failed\n> ");
                continue;
            };
            console_write_counter(b"counter value: ", value);
            continue;
        }
        if bytes_eq(command, b"increment") {
            log(b"console-shell command: increment");
            let Some(value) = counter_request(b"I") else {
                console_write(b"counter request failed\n> ");
                continue;
            };
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
            if !ipc_send_with_backpressure(
                CAP_GENERATION_MANAGER_REQUEST,
                b"install gen:console-new-0002",
                b"console-shell install generation failed",
            ) {
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
            if !ipc_send_with_backpressure(
                CAP_GENERATION_MANAGER_REQUEST,
                b"install gen:state-migration-bad-0003",
                b"console-shell bad state migration install send failed",
            ) {
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
            if !ipc_send_with_backpressure(
                CAP_GENERATION_MANAGER_REQUEST,
                b"install gen:state-migration-new-0002",
                b"console-shell state migration install failed",
            ) {
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
            if !ipc_send_with_backpressure(
                CAP_PACKAGE_IMPORT_REQUEST,
                b"import pkg:logd",
                b"console-shell package import failed",
            ) {
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
            if !ipc_send_with_backpressure(
                CAP_PACKAGE_IMPORT_REQUEST,
                b"import pkg:missing-dependency",
                b"console-shell missing-dependency validation failed",
            ) {
                console_write(b"package import validation failed\n> ");
            }
            continue;
        }
        if bytes_eq(command, b"import package pkg:excess-authority") {
            log(b"console-shell command: import package pkg:excess-authority");
            console_write(b"import package pkg:excess-authority\n> ");
            yield_for_console_driver();
            log(b"console-shell requests package-import excess-authority validation");
            if !ipc_send_with_backpressure(
                CAP_PACKAGE_IMPORT_REQUEST,
                b"import pkg:excess-authority",
                b"console-shell excess-authority validation failed",
            ) {
                console_write(b"package import validation failed\n> ");
            }
            continue;
        }
        if bytes_eq(command, b"rollback to gen:old") {
            log(b"console-shell command: rollback to gen:old");
            let Some(value) = counter_request(b"G") else {
                console_write(b"counter request failed\n> ");
                continue;
            };
            console_write_rollback(value);
            yield_for_console_driver();
            log(b"console-shell requests generation-manager rollback");
            if !ipc_send_with_backpressure(
                CAP_GENERATION_MANAGER_REQUEST,
                b"rollback gen:console-0001",
                b"console-shell rollback failed",
            ) {
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
            if !ipc_send_with_backpressure(
                CAP_GENERATION_MANAGER_REQUEST,
                b"rollback gen:state-migration-0001",
                b"console-shell state migration rollback failed",
            ) {
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
            if !ipc_send_with_backpressure(
                CAP_GENERATION_MANAGER_REQUEST,
                b"rollback gen:package-import-0001",
                b"console-shell rollback failed",
            ) {
                console_write(b"rollback failed\n> ");
                continue;
            }
            loop {
                sys::pause();
            }
        }
        if command_verb_eq(command, b"activate") {
            if !require_word_count(command, 2, b"usage: activate <generation>\n> ") {
                continue;
            }
            log(b"console-shell command: activate");
            operator_activate(command);
            continue;
        }
        if command_verb_eq(command, b"rollback") {
            if !require_word_count(command, 2, b"usage: rollback <generation>\n> ") {
                continue;
            }
            log(b"console-shell command: operator rollback");
            operator_rollback(command);
            continue;
        }
        if command_verb_eq(command, b"mark-known-good") {
            if !require_word_count(command, 2, b"usage: mark-known-good <generation>\n> ") {
                continue;
            }
            log(b"console-shell command: mark-known-good");
            let report = runtime_report();
            operator_mark_known_good(report, command);
            continue;
        }
        if bytes_eq(command, b"halt") {
            log(b"console-shell command: halt");
            console_write(b"Native console shell ok\n");
            let _ =
                ipc_send_with_backpressure(CAP_COUNTER_REQUEST, b"H", b"counter shutdown failed");
            let _ = ipc_send_with_backpressure(
                CAP_PACKAGE_IMPORT_REQUEST,
                b"shutdown",
                b"package-import shutdown failed",
            );
            wait_for_state_clients_to_drain();
            shutdown_state_service();
            shutdown_generation_manager();
            if !ipc_send_with_backpressure(
                CAP_CONSOLE_CONTROL,
                CONTROL_SHUTDOWN,
                b"console-shell shutdown send failed",
            ) {
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

fn console_write_device_failure(report: &[u8]) {
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
    append(&mut payload, &mut len, b"\n");
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

fn operator_help(command: &[u8]) {
    operator_finish(operator_shell::help(command, |line| {
        console_stream_line(line)
    }));
}

fn operator_overview(report: &[u8]) {
    operator_finish(operator_shell::overview(report, |line| {
        console_stream_line(line)
    }));
}

fn operator_services(report: &[u8]) {
    log_core_service_states(report);
    operator_finish(operator_shell::services(report, |line| {
        console_stream_line(line)
    }));
}

fn operator_service_detail(report: &[u8], command: &[u8]) {
    operator_finish(operator_shell::service_detail(report, command, |line| {
        console_stream_line(line)
    }));
}

fn operator_capabilities(report: &[u8], command: &[u8]) {
    operator_finish(operator_shell::capabilities(report, command, |line| {
        console_stream_line(line)
    }));
}

fn operator_capability_detail(report: &[u8], command: &[u8]) {
    operator_finish(operator_shell::capability_detail(report, command, |line| {
        console_stream_line(line)
    }));
}

fn operator_states(report: &[u8]) {
    operator_finish(operator_shell::states(report, |line| {
        console_stream_line(line)
    }));
}

fn operator_state_detail(report: &[u8], command: &[u8]) {
    operator_finish(operator_shell::state_detail(report, command, |line| {
        console_stream_line(line)
    }));
}

fn operator_devices(report: &[u8]) {
    console_write_device_failure(report);
    operator_finish(operator_shell::devices(report, |line| {
        console_stream_line(line)
    }));
}

fn operator_device_detail(report: &[u8], command: &[u8]) {
    operator_finish(operator_shell::device_detail(report, command, |line| {
        console_stream_line(line)
    }));
}

fn log_core_service_states(report: &[u8]) {
    let mut index = 0;
    while index < SERVICE_NAMES.len() {
        let state = process_state(report, SERVICE_NAMES[index]);
        log_service_state(SERVICE_NAMES[index], state);
        index += 1;
    }
    log(b"native shell services query ok");
}

fn operator_finish<T>(result: operator_shell::Result<T>) {
    match result {
        Ok(_) => console_write(b"> "),
        Err(error) => operator_fail(error),
    }
}

fn console_stream_line(line: &[u8]) {
    log(line);
    console_write(line);
    console_write(b"\n");
}

fn operator_fail(error: operator_shell::Error) {
    log(error.message);
    let mut payload = [0u8; 128];
    let mut len = 0;
    append(&mut payload, &mut len, b"error: ");
    append(&mut payload, &mut len, error.message);
    append(&mut payload, &mut len, b"\n> ");
    console_write(&payload[..len]);
}

fn operator_expect<T>(result: operator_shell::Result<T>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            operator_fail(error);
            None
        }
    }
}

fn operator_current_generation(report: &[u8]) {
    let Some(answer) = operator_expect(operator_shell::current_generation(report)) else {
        return;
    };
    let mut buffer = [0u8; 256];
    let mut len = 0;
    append(
        &mut buffer,
        &mut len,
        b"operator current-generation generation=",
    );
    append(&mut buffer, &mut len, answer.generation);
    append(&mut buffer, &mut len, b" policy_hash=");
    append(&mut buffer, &mut len, answer.policy_hash);
    append(&mut buffer, &mut len, b" graph_hash=");
    append(&mut buffer, &mut len, answer.graph_hash);
    log(&buffer[..len]);
    console_write(b"current-generation ok\n> ");
}

fn operator_generations(report: &[u8]) {
    let Some(count) = operator_expect(operator_shell::for_generations(report, |line| log(line)))
    else {
        return;
    };
    log_count_line(b"operator generations listed=", count);
    console_write(b"generations ok\n> ");
}

fn operator_generation_status(report: &[u8]) {
    let Some(answer) = operator_expect(operator_shell::generation_status(report)) else {
        return;
    };

    let mut buffer = [0u8; 256];
    let mut len = 0;
    append(
        &mut buffer,
        &mut len,
        b"operator generation-status selected=",
    );
    append(&mut buffer, &mut len, answer.selected);
    append(&mut buffer, &mut len, b" previous=");
    append(&mut buffer, &mut len, answer.previous);
    append(&mut buffer, &mut len, b" known_good=");
    append(&mut buffer, &mut len, answer.known_good);
    append(&mut buffer, &mut len, b" transaction=");
    append(&mut buffer, &mut len, answer.transaction);
    append(&mut buffer, &mut len, b" target=");
    append(&mut buffer, &mut len, answer.target);
    append(&mut buffer, &mut len, b" policy_hash=");
    append(&mut buffer, &mut len, answer.policy_hash);
    log(&buffer[..len]);
    console_write(b"generation-status ok\n> ");
}

fn operator_diff_generation(report: &[u8], command: &[u8], authority_only: bool) {
    let Some(answer) = operator_expect(operator_shell::diff_generation(report, command)) else {
        return;
    };

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
    append(&mut buffer, &mut len, answer.from);
    append(&mut buffer, &mut len, b" to=");
    append(&mut buffer, &mut len, answer.to);
    if !authority_only {
        append(&mut buffer, &mut len, b" services=+");
        append_u64(&mut buffer, &mut len, answer.service_added);
        append(&mut buffer, &mut len, b"-");
        append_u64(&mut buffer, &mut len, answer.service_removed);
        append(&mut buffer, &mut len, b" changed_services=");
        append_u64(&mut buffer, &mut len, answer.service_changed);
        append(&mut buffer, &mut len, b" packages=unavailable");
    }
    append(&mut buffer, &mut len, b" capabilities=+");
    append_u64(&mut buffer, &mut len, answer.capability_added);
    append(&mut buffer, &mut len, b"-");
    append_u64(&mut buffer, &mut len, answer.capability_removed);
    append(&mut buffer, &mut len, b" changed_capabilities=");
    append_u64(&mut buffer, &mut len, answer.capability_changed);
    if !authority_only {
        append(&mut buffer, &mut len, b" state=+");
        append_u64(&mut buffer, &mut len, answer.state_added);
        append(&mut buffer, &mut len, b"-");
        append_u64(&mut buffer, &mut len, answer.state_removed);
        append(&mut buffer, &mut len, b" changed_state=");
        append_u64(&mut buffer, &mut len, answer.state_changed);
        append(&mut buffer, &mut len, b" devices=+");
        append_u64(&mut buffer, &mut len, answer.device_added);
        append(&mut buffer, &mut len, b"-");
        append_u64(&mut buffer, &mut len, answer.device_removed);
        append(&mut buffer, &mut len, b" changed_devices=");
        append_u64(&mut buffer, &mut len, answer.device_changed);
    }
    append(&mut buffer, &mut len, b" policy_hash=");
    append(&mut buffer, &mut len, answer.policy_hash);
    log(&buffer[..len]);
    if authority_only {
        console_write(b"planned-authority-delta ok\n> ");
    } else {
        console_write(b"diff-generation ok\n> ");
    }
}

fn operator_why(report: &[u8], command: &[u8]) {
    let Some(answer) = operator_expect(operator_shell::why(report, command)) else {
        return;
    };

    let mut buffer = [0u8; 256];
    let mut len = 0;
    append(&mut buffer, &mut len, b"operator why service=");
    append(&mut buffer, &mut len, answer.service);
    append(&mut buffer, &mut len, b" capability=");
    append(&mut buffer, &mut len, answer.capability);
    append(&mut buffer, &mut len, b" provider=");
    append(&mut buffer, &mut len, answer.provider);
    append(&mut buffer, &mut len, b" rights=");
    append(&mut buffer, &mut len, answer.rights);
    append(&mut buffer, &mut len, b" edge=");
    append(&mut buffer, &mut len, answer.edge);
    append(&mut buffer, &mut len, b" generation=");
    append(&mut buffer, &mut len, answer.generation);
    append(&mut buffer, &mut len, b" policy_hash=");
    append(&mut buffer, &mut len, answer.policy_hash);
    log(&buffer[..len]);
    console_write(b"why ok\n> ");
}

fn operator_who_can(report: &[u8], command: &[u8]) {
    let Some(answer) = operator_expect(operator_shell::who_can(
        report,
        command,
        |entry| match entry {
            operator_shell::WhoCanEntry::StateWriter {
                service,
                state,
                rights,
            } => {
                let mut buffer = [0u8; 192];
                let mut len = 0;
                append(&mut buffer, &mut len, b"operator who-can writer service=");
                append(&mut buffer, &mut len, service);
                append(&mut buffer, &mut len, b" state=");
                append(&mut buffer, &mut len, state);
                append(&mut buffer, &mut len, b" rights=");
                append(&mut buffer, &mut len, rights);
                log(&buffer[..len]);
            }
            operator_shell::WhoCanEntry::CapabilityConsumer {
                service,
                capability,
                rights,
            } => {
                let mut buffer = [0u8; 192];
                let mut len = 0;
                append(&mut buffer, &mut len, b"operator who-can consumer service=");
                append(&mut buffer, &mut len, service);
                append(&mut buffer, &mut len, b" capability=");
                append(&mut buffer, &mut len, capability);
                append(&mut buffer, &mut len, b" rights=");
                append(&mut buffer, &mut len, rights);
                log(&buffer[..len]);
            }
        },
    )) else {
        return;
    };

    let mut buffer = [0u8; 192];
    let mut len = 0;
    append(&mut buffer, &mut len, b"operator who-can object=");
    append(&mut buffer, &mut len, answer.object);
    match answer.kind {
        operator_shell::WhoCanKind::StateWriters => {
            append(&mut buffer, &mut len, b" writer_count=");
        }
        operator_shell::WhoCanKind::CapabilityConsumers => {
            append(&mut buffer, &mut len, b" consumer_count=");
        }
    }
    append_u64(&mut buffer, &mut len, answer.count);
    append(&mut buffer, &mut len, b" generation=");
    append(&mut buffer, &mut len, answer.generation);
    append(&mut buffer, &mut len, b" policy_hash=");
    append(&mut buffer, &mut len, answer.policy_hash);
    log(&buffer[..len]);
    console_write(b"who-can ok\n> ");
}

fn operator_which_generation(report: &[u8], command: &[u8]) {
    let Some(answer) = operator_expect(operator_shell::which_generation(report, command)) else {
        return;
    };
    let mut buffer = [0u8; 192];
    let mut len = 0;
    append(
        &mut buffer,
        &mut len,
        b"operator which-generation selector=",
    );
    append(&mut buffer, &mut len, answer.selector);
    append(&mut buffer, &mut len, b" process=");
    append(&mut buffer, &mut len, answer.process());
    append(&mut buffer, &mut len, b" generation=");
    append(&mut buffer, &mut len, answer.generation);
    append(&mut buffer, &mut len, b" policy_hash=");
    append(&mut buffer, &mut len, answer.policy_hash);
    log(&buffer[..len]);
    console_write(b"which-generation ok\n> ");
}

fn operator_package_list(report: &[u8]) {
    if operator_expect(operator_shell::package_list_unavailable(report)).is_none() {
        return;
    }
    log(b"operator package-list unavailable: no native package facts");
    console_write(b"package-list unavailable\n> ");
}

fn operator_activation_log(report: &[u8]) {
    let Some(count) = operator_expect(operator_shell::activation_log(report, |line| log(line)))
    else {
        return;
    };
    log_count_line(b"operator activation-log records=", count);
    console_write(b"activation-log ok\n> ");
}

fn operator_activate(command: &[u8]) {
    let mut request = [0u8; 96];
    let Some(answer) = operator_expect(operator_shell::activate_request(command, &mut request))
    else {
        return;
    };
    log_prefix(
        b"operator activate queues generation-manager install: generation=",
        answer.generation,
    );
    if !ipc_send_with_backpressure(
        CAP_GENERATION_MANAGER_REQUEST,
        answer.request,
        b"operator activate send failed",
    ) {
        console_write(b"activate failed\n> ");
        return;
    }
    console_write(b"activate requested\n> ");
    yield_for_console_driver();
    loop {
        sys::pause();
    }
}

fn operator_rollback(command: &[u8]) {
    let mut request = [0u8; 96];
    let Some(answer) = operator_expect(operator_shell::rollback_request(command, &mut request))
    else {
        return;
    };
    log_prefix(
        b"operator rollback queues generation-manager rollback: generation=",
        answer.generation,
    );
    if !ipc_send_with_backpressure(
        CAP_GENERATION_MANAGER_REQUEST,
        answer.request,
        b"operator rollback send failed",
    ) {
        console_write(b"rollback failed\n> ");
        return;
    }
    console_write(b"rollback requested\n> ");
    yield_for_console_driver();
    loop {
        sys::pause();
    }
}

fn operator_mark_known_good(report: &[u8], command: &[u8]) {
    let mut request = [0u8; 96];
    let Some(answer) = operator_expect(operator_shell::mark_known_good_request(
        report,
        command,
        &mut request,
    )) else {
        return;
    };
    log_prefix(
        b"operator mark-known-good queues generation-manager command: generation=",
        answer.generation,
    );
    if !ipc_send_with_backpressure(
        CAP_GENERATION_MANAGER_REQUEST,
        answer.request,
        b"operator mark-known-good send failed",
    ) {
        console_write(b"mark-known-good failed\n> ");
        return;
    }
    yield_for_console_driver();
    console_write(b"mark-known-good requested\n> ");
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

fn ipc_send_with_backpressure(capability: u64, payload: &[u8], failure_log: &[u8]) -> bool {
    let mut attempts = 0;
    loop {
        let status = sys::ipc_send(capability, payload);
        if status == sys::STATUS_OK {
            return true;
        }
        if status == sys::STATUS_TOO_LARGE && attempts < IPC_SEND_ATTEMPTS {
            attempts += 1;
            sys::yield_now();
            continue;
        }
        log(failure_log);
        return false;
    }
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

fn counter_request(request: &[u8]) -> Option<&'static [u8]> {
    if !ipc_send_with_backpressure(
        CAP_COUNTER_REQUEST,
        request,
        b"console-shell counter request failed",
    ) {
        return None;
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
        if received == sys::STATUS_EMPTY {
            log(b"console-shell counter reply timed out");
            return None;
        }
        if received == sys::STATUS_BAD_CAPABILITY
            || received == sys::STATUS_BAD_BUFFER
            || received == sys::STATUS_TOO_LARGE
            || received > buffer.len() as u64
        {
            log(b"console-shell counter reply failed");
            sys::exit(1);
        }
        return Some(&buffer[..received as usize]);
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
    if !ipc_send_with_backpressure(
        CAP_GENERATION_MANAGER_REQUEST,
        GENERATION_MANAGER_SHUTDOWN,
        b"console-shell generation-manager shutdown failed",
    ) {
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
    let mut attempts = 0;
    loop {
        let status = sys::ipc_send(CAP_CONSOLE_OUTPUT, payload);
        if status == sys::STATUS_OK {
            return;
        }
        if status == sys::STATUS_TOO_LARGE && attempts < CONSOLE_WRITE_ATTEMPTS {
            attempts += 1;
            sys::yield_now();
            continue;
        }
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

fn normalize_command_verb<'a>(input: &[u8], output: &'a mut [u8]) -> &'a [u8] {
    let mut start = 0;
    while start < input.len() && is_command_space(input[start]) {
        start += 1;
    }
    let mut end = input.len();
    while end > start && is_command_space(input[end - 1]) {
        end -= 1;
    }
    let len = end - start;
    if len > output.len() {
        return &output[..0];
    }

    let mut index = 0;
    let mut in_verb = true;
    while index < len {
        let mut byte = input[start + index];
        if is_command_space(byte) {
            byte = b' ';
            in_verb = false;
        } else if in_verb && byte >= b'A' && byte <= b'Z' {
            byte += b'a' - b'A';
        }
        output[index] = byte;
        index += 1;
    }
    &output[..len]
}

fn require_word_count(command: &[u8], expected: usize, usage: &[u8]) -> bool {
    if command_word_count(command) == expected {
        return true;
    }
    log(usage);
    console_write(usage);
    false
}

fn command_verb_eq(command: &[u8], verb: &[u8]) -> bool {
    if let Some(candidate) = command_word_at(command, 0) {
        return bytes_eq(candidate, verb);
    }
    false
}

fn command_word_count(command: &[u8]) -> usize {
    let mut count = 0;
    let mut cursor = 0;
    while cursor < command.len() {
        while cursor < command.len() && is_command_space(command[cursor]) {
            cursor += 1;
        }
        if cursor == command.len() {
            break;
        }
        count += 1;
        while cursor < command.len() && !is_command_space(command[cursor]) {
            cursor += 1;
        }
    }
    count
}

fn command_word_at(command: &[u8], requested: usize) -> Option<&[u8]> {
    let mut cursor = 0;
    let mut index = 0;
    while cursor < command.len() {
        while cursor < command.len() && is_command_space(command[cursor]) {
            cursor += 1;
        }
        if cursor == command.len() {
            return None;
        }
        let start = cursor;
        while cursor < command.len() && !is_command_space(command[cursor]) {
            cursor += 1;
        }
        if index == requested {
            return Some(&command[start..cursor]);
        }
        index += 1;
    }
    None
}

fn is_command_space(byte: u8) -> bool {
    byte == b' ' || byte == b'\t' || byte == b'\r' || byte == b'\n'
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
