use super::*;

pub fn initial_process_name() -> &'static str {
    current_process_name()
}

pub fn current_process_name() -> &'static str {
    runtime()
        .processes
        .current_process()
        .map(|process| process.name)
        .unwrap_or("<none>")
}

pub(super) fn current_process_id() -> ProcessId {
    runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
        .unwrap_or_else(ProcessId::empty)
}
