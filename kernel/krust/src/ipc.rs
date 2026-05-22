use core::cell::UnsafeCell;

use crate::{
    capability, paging, serial,
    usercopy::{self, UserPtr},
};

pub const ENDPOINT_CAP_SLOT: u64 = 0;
pub const BOOT_ENDPOINT_ID: u64 = 1;

const MAX_MESSAGE_BYTES: usize = 128;
const MAX_OBJECTS: usize = 8;
const MAX_PROCESSES: usize = 4;
const MAX_CAPS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessId {
    Sender,
    Receiver,
}

impl ProcessId {
    fn name(self) -> &'static str {
        match self {
            Self::Sender => "ipc-sender",
            Self::Receiver => "ipc-receiver",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct KernelObjectId(u64);

impl KernelObjectId {
    fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy)]
pub struct ProcessContext {
    pub cr3: u64,
    pub entry: u64,
    pub stack_top: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProcessState {
    Empty,
    Ready,
    Running,
    Exited,
}

#[derive(Clone, Copy)]
pub enum ExitAction {
    Switch {
        name: &'static str,
        context: ProcessContext,
    },
    Halt {
        ok: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitError {
    ObjectTableFull,
    ProcessTableFull,
    CapabilityTableFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    BadCapability,
    InvalidUserBuffer,
    MessageTooLarge,
    Empty,
}

#[derive(Clone, Copy)]
struct Capability {
    object: KernelObjectId,
    rights: u64,
}

#[derive(Clone, Copy)]
struct CapabilitySpace {
    caps: [Option<Capability>; MAX_CAPS],
}

#[derive(Clone, Copy)]
struct Process {
    pid: ProcessId,
    name: &'static str,
    context: ProcessContext,
    state: ProcessState,
    caps: CapabilitySpace,
}

#[derive(Clone, Copy)]
struct IpcEndpoint {
    id: KernelObjectId,
    message_ready: bool,
    message_len: usize,
    message: [u8; MAX_MESSAGE_BYTES],
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct ProcessObject {
    pid: ProcessId,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct MemoryObject {
    base: u64,
    length: u64,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct BootModuleObject {
    base: u64,
    length: u64,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum KernelObject {
    IpcEndpoint(IpcEndpoint),
    Process(ProcessObject),
    MemoryObject(MemoryObject),
    BootModule(BootModuleObject),
}

struct ObjectTable {
    objects: [Option<KernelObject>; MAX_OBJECTS],
    count: usize,
    next_id: u64,
}

struct ProcessTable {
    processes: [Option<Process>; MAX_PROCESSES],
    count: usize,
    current: Option<ProcessId>,
}

struct RuntimeState {
    objects: ObjectTable,
    processes: ProcessTable,
}

struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

static RUNTIME: Global<RuntimeState> = Global(UnsafeCell::new(RuntimeState::new()));

impl CapabilitySpace {
    const fn new() -> Self {
        Self {
            caps: [None; MAX_CAPS],
        }
    }

    fn grant(&mut self, slot: u64, object: KernelObjectId, rights: u64) -> Result<(), InitError> {
        let Ok(slot) = usize::try_from(slot) else {
            return Err(InitError::CapabilityTableFull);
        };
        if slot >= self.caps.len() {
            return Err(InitError::CapabilityTableFull);
        }

        self.caps[slot] = Some(Capability { object, rights });
        Ok(())
    }

    fn lookup(&self, slot: u64) -> Option<Capability> {
        let slot = usize::try_from(slot).ok()?;
        self.caps.get(slot).copied().flatten()
    }
}

impl Process {
    const fn empty() -> Self {
        Self {
            pid: ProcessId::Sender,
            name: "",
            context: ProcessContext {
                cr3: 0,
                entry: 0,
                stack_top: 0,
            },
            state: ProcessState::Empty,
            caps: CapabilitySpace::new(),
        }
    }

    fn new(
        pid: ProcessId,
        context: ProcessContext,
        state: ProcessState,
        caps: CapabilitySpace,
    ) -> Self {
        Self {
            pid,
            name: pid.name(),
            context,
            state,
            caps,
        }
    }
}

impl IpcEndpoint {
    const fn new(id: KernelObjectId) -> Self {
        Self {
            id,
            message_ready: false,
            message_len: 0,
            message: [0; MAX_MESSAGE_BYTES],
        }
    }
}

impl ObjectTable {
    const fn new() -> Self {
        Self {
            objects: [None; MAX_OBJECTS],
            count: 0,
            next_id: BOOT_ENDPOINT_ID,
        }
    }

    fn add_endpoint(&mut self) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::IpcEndpoint(IpcEndpoint::new(id)));
        self.count += 1;
        Ok(id)
    }

    fn endpoint_count(&self) -> usize {
        let mut count = 0;
        let mut index = 0;
        while index < self.count {
            if matches!(self.objects[index], Some(KernelObject::IpcEndpoint(_))) {
                count += 1;
            }
            index += 1;
        }
        count
    }

    fn get_endpoint_mut(&mut self, id: KernelObjectId) -> Option<&mut IpcEndpoint> {
        let mut found = None;
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::IpcEndpoint(endpoint)) = self.objects[index]
                && endpoint.id == id
            {
                found = Some(index);
                break;
            }
            index += 1;
        }

        match &mut self.objects[found?] {
            Some(KernelObject::IpcEndpoint(endpoint)) => Some(endpoint),
            _ => None,
        }
    }
}

impl ProcessTable {
    const fn new() -> Self {
        Self {
            processes: [Some(Process::empty()); MAX_PROCESSES],
            count: 0,
            current: None,
        }
    }

    fn add_process(&mut self, process: Process) -> Result<(), InitError> {
        if self.count == self.processes.len() {
            return Err(InitError::ProcessTableFull);
        }

        self.processes[self.count] = Some(process);
        self.count += 1;
        Ok(())
    }

    fn set_current(&mut self, pid: ProcessId) {
        self.current = Some(pid);
    }

    fn current_process(&self) -> Option<Process> {
        let pid = self.current?;
        self.process(pid).copied()
    }

    fn current_process_mut(&mut self) -> Option<&mut Process> {
        let pid = self.current?;
        self.process_mut(pid)
    }

    fn process(&self, pid: ProcessId) -> Option<&Process> {
        let mut index = 0;
        while index < self.count {
            if let Some(process) = &self.processes[index]
                && process.pid == pid
            {
                return Some(process);
            }
            index += 1;
        }

        None
    }

    fn process_mut(&mut self, pid: ProcessId) -> Option<&mut Process> {
        let mut found = None;
        let mut index = 0;
        while index < self.count {
            if let Some(process) = self.processes[index]
                && process.pid == pid
            {
                found = Some(index);
                break;
            }
            index += 1;
        }

        self.processes[found?].as_mut()
    }

    fn next_ready_process(&mut self) -> Option<Process> {
        let mut index = 0;
        while index < self.count {
            if let Some(process) = &mut self.processes[index]
                && process.state == ProcessState::Ready
            {
                process.state = ProcessState::Running;
                self.current = Some(process.pid);
                return Some(*process);
            }
            index += 1;
        }

        None
    }
}

impl RuntimeState {
    const fn new() -> Self {
        Self {
            objects: ObjectTable::new(),
            processes: ProcessTable::new(),
        }
    }
}

pub fn init_for_boot(sender: ProcessContext, receiver: ProcessContext) -> Result<(), InitError> {
    let runtime = runtime();
    *runtime = RuntimeState::new();

    let endpoint = runtime.objects.add_endpoint()?;

    let mut sender_caps = CapabilitySpace::new();
    sender_caps.grant(ENDPOINT_CAP_SLOT, endpoint, capability::RIGHT_SEND)?;
    runtime.processes.add_process(Process::new(
        ProcessId::Sender,
        sender,
        ProcessState::Running,
        sender_caps,
    ))?;

    let mut receiver_caps = CapabilitySpace::new();
    receiver_caps.grant(ENDPOINT_CAP_SLOT, endpoint, capability::RIGHT_RECEIVE)?;
    runtime.processes.add_process(Process::new(
        ProcessId::Receiver,
        receiver,
        ProcessState::Ready,
        receiver_caps,
    ))?;

    runtime.processes.set_current(ProcessId::Sender);
    print_boot_tables(runtime);
    Ok(())
}

pub fn initial_process_context() -> Option<ProcessContext> {
    runtime()
        .processes
        .current_process()
        .map(|process| process.context)
}

pub fn current_process_name() -> &'static str {
    runtime()
        .processes
        .current_process()
        .map(|process| process.name)
        .unwrap_or("<none>")
}

pub fn exit_current_process(status: u64) -> ExitAction {
    let runtime = runtime();

    if let Some(process) = runtime.processes.current_process_mut() {
        process.state = ProcessState::Exited;
    }

    if status != 0 {
        return ExitAction::Halt { ok: false };
    }

    if let Some(next) = runtime.processes.next_ready_process() {
        ExitAction::Switch {
            name: next.name,
            context: next.context,
        }
    } else {
        ExitAction::Halt { ok: true }
    }
}

pub fn send(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let endpoint_id = match endpoint_from_cap(cap_slot, capability::RIGHT_SEND) {
        Ok(endpoint_id) => endpoint_id,
        Err(error) => {
            print_negative("send");
            return Err(error);
        }
    };

    let mut message = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut message, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let endpoint = runtime()
        .objects
        .get_endpoint_mut(endpoint_id)
        .ok_or(IpcError::BadCapability)?;

    endpoint.message[..len].copy_from_slice(&message[..len]);
    endpoint.message_len = len;
    endpoint.message_ready = true;

    serial::write_str("IPC send accepted: endpoint=");
    serial::write_u64_dec(endpoint.id.raw());
    serial::write_str(" bytes=");
    serial::write_u64_dec(len as u64);
    serial::write_str("\n");

    Ok(())
}

pub fn receive(cap_slot: u64, destination: *mut u8, max_len: usize) -> Result<usize, IpcError> {
    let endpoint_id = match endpoint_from_cap(cap_slot, capability::RIGHT_RECEIVE) {
        Ok(endpoint_id) => endpoint_id,
        Err(error) => {
            print_negative("receive");
            return Err(error);
        }
    };

    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        max_len,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;

    let mut message = [0u8; MAX_MESSAGE_BYTES];
    let copy_len = {
        let endpoint = runtime()
            .objects
            .get_endpoint_mut(endpoint_id)
            .ok_or(IpcError::BadCapability)?;

        if !endpoint.message_ready {
            return Err(IpcError::Empty);
        }

        let copy_len = min(endpoint.message_len, max_len);
        message[..copy_len].copy_from_slice(&endpoint.message[..copy_len]);
        copy_len
    };

    usercopy::copy_to_user(UserPtr::new(destination as u64), &message[..copy_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let endpoint = runtime()
        .objects
        .get_endpoint_mut(endpoint_id)
        .ok_or(IpcError::BadCapability)?;
    endpoint.message_ready = false;

    serial::write_str("IPC receive delivered: endpoint=");
    serial::write_u64_dec(endpoint.id.raw());
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");

    Ok(copy_len)
}

fn endpoint_from_cap(cap_slot: u64, required_right: u64) -> Result<KernelObjectId, IpcError> {
    let process = runtime()
        .processes
        .current_process()
        .ok_or(IpcError::BadCapability)?;
    let cap = process
        .caps
        .lookup(cap_slot)
        .ok_or(IpcError::BadCapability)?;

    if cap.rights & required_right == 0 {
        return Err(IpcError::BadCapability);
    }

    match runtime().objects.get_endpoint_mut(cap.object) {
        Some(_) => Ok(cap.object),
        None => Err(IpcError::BadCapability),
    }
}

fn print_boot_tables(runtime: &RuntimeState) {
    serial::write_str("Process table entries: ");
    serial::write_u64_dec(runtime.processes.count as u64);
    serial::write_str("\n");

    serial::write_str("Endpoint table entries: ");
    serial::write_u64_dec(runtime.objects.endpoint_count() as u64);
    serial::write_str("\n");

    print_process_caps(runtime, ProcessId::Sender);
    print_process_caps(runtime, ProcessId::Receiver);
}

fn print_process_caps(runtime: &RuntimeState, pid: ProcessId) {
    let Some(process) = runtime.processes.process(pid) else {
        return;
    };

    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = process.caps.caps[slot] {
            serial::write_str("proc=");
            serial::write_str(process.name);
            serial::write_str(" cap[");
            serial::write_u64_dec(slot as u64);
            serial::write_str("] endpoint=");
            serial::write_u64_dec(cap.object.raw());
            serial::write_str(" rights=");
            print_rights(cap.rights);
            serial::write_str("\n");
        }
        slot += 1;
    }
}

fn print_rights(rights: u64) {
    let mut wrote = false;
    wrote = print_right(rights, capability::RIGHT_SEND, "send", wrote);
    wrote = print_right(rights, capability::RIGHT_RECEIVE, "receive", wrote);

    if !wrote {
        serial::write_str("none");
    }
}

fn print_right(rights: u64, right: u64, label: &str, wrote: bool) -> bool {
    if rights & right == 0 {
        return wrote;
    }

    if wrote {
        serial::write_str("|");
    }
    serial::write_str(label);
    true
}

fn print_negative(operation: &str) {
    serial::write_str("IPC negative test: ");
    serial::write_str(current_process_short_name());
    serial::write_str(" ");
    serial::write_str(operation);
    serial::write_str(" rejected: bad capability\n");
}

fn current_process_short_name() -> &'static str {
    match runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
    {
        Some(ProcessId::Sender) => "sender",
        Some(ProcessId::Receiver) => "receiver",
        None => "<none>",
    }
}

fn runtime() -> &'static mut RuntimeState {
    unsafe { &mut *RUNTIME.0.get() }
}

fn min(left: usize, right: usize) -> usize {
    if left < right { left } else { right }
}
