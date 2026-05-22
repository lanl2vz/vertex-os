use core::{arch::asm, cell::UnsafeCell};

use crate::{
    capability, gdt, paging, serial,
    usercopy::{self, UserPtr},
};

pub const BOOT_ENDPOINT_ID: u64 = 1;

const MAX_MESSAGE_BYTES: usize = 128;
const MAX_BOOT_READ_BYTES: usize = 16 * 1024;
const MAX_OBJECTS: usize = 32;
const MAX_PROCESSES: usize = 16;
const MAX_CAPS: usize = 32;
const MAX_BOOT_GRANTS: usize = 64;
const MAX_STATE_VALUE_BYTES: usize = 64;
const INITIAL_USER_RFLAGS: u64 = 0x2;
const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;
pub const BOOT_OBJECT_ENDPOINT: u16 = 1;
pub const BOOT_OBJECT_STORE: u16 = 2;
pub const BOOT_OBJECT_STATE: u16 = 3;
pub const BOOT_OBJECT_TIMER: u16 = 4;
pub const BOOT_OBJECT_NETWORK_PORT: u16 = 5;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SyscallFrame {
    pub user_rsp: u64,
    pub user_rip: u64,
    pub user_rflags: u64,
    pub rax: u64,
}

impl SyscallFrame {
    const fn empty() -> Self {
        Self {
            user_rsp: 0,
            user_rip: 0,
            user_rflags: 0,
            rax: 0,
        }
    }

    fn from_context(context: ProcessContext) -> Self {
        Self {
            user_rsp: context.stack_top,
            user_rip: context.entry,
            user_rflags: INITIAL_USER_RFLAGS,
            rax: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessId(u64);

impl ProcessId {
    const fn empty() -> Self {
        Self(0)
    }

    fn new(raw: u64) -> Self {
        Self(raw)
    }

    fn raw(self) -> u64 {
        self.0
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

#[derive(Clone, Copy)]
pub struct BootProcessConfig {
    pub name: &'static str,
    pub context: ProcessContext,
    pub initial: bool,
}

#[derive(Clone, Copy)]
pub struct BootEndpointConfig {
    pub name: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootModuleConfig {
    pub name: &'static str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct BootStoreObjectConfig {
    pub id: &'static str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct BootStateVolumeConfig {
    pub id: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootNetworkPortConfig {
    pub id: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootGrantConfig {
    pub process_index: usize,
    pub cap_slot: u64,
    pub object_kind: u16,
    pub object_index: usize,
    pub rights: u64,
}

#[derive(Clone, Copy)]
pub struct BootRuntimeConfig {
    processes: [Option<BootProcessConfig>; MAX_PROCESSES],
    process_count: usize,
    endpoints: [Option<BootEndpointConfig>; MAX_OBJECTS],
    endpoint_count: usize,
    manifest_module: Option<BootModuleConfig>,
    store_objects: [Option<BootStoreObjectConfig>; MAX_OBJECTS],
    store_object_count: usize,
    state_volumes: [Option<BootStateVolumeConfig>; MAX_OBJECTS],
    state_volume_count: usize,
    network_ports: [Option<BootNetworkPortConfig>; MAX_OBJECTS],
    network_port_count: usize,
    grants: [Option<BootGrantConfig>; MAX_BOOT_GRANTS],
    grant_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProcessState {
    Empty,
    Declared,
    Ready,
    Running,
    BlockedOnEndpoint {
        endpoint: KernelObjectId,
        destination: u64,
        max_len: usize,
    },
    Exited,
}

impl ProcessState {
    fn label(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Declared => "declared",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::BlockedOnEndpoint { .. } => "blocked",
            Self::Exited => "exited",
        }
    }
}

#[derive(Clone, Copy)]
pub enum ScheduleResult {
    Continue,
    Switched,
    Halt { ok: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitError {
    ObjectTableFull,
    ProcessTableFull,
    CapabilityTableFull,
    InvalidBootManifest,
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
    saved_frame: SyscallFrame,
    has_saved_frame: bool,
    exit_status: u64,
    has_exited: bool,
}

#[derive(Clone, Copy)]
struct IpcEndpoint {
    id: KernelObjectId,
    name: &'static str,
    message_ready: bool,
    message_len: usize,
    message: [u8; MAX_MESSAGE_BYTES],
}

#[derive(Clone, Copy)]
struct BootModuleObject {
    id: KernelObjectId,
    name: &'static str,
    base: u64,
    length: u64,
}

#[derive(Clone, Copy)]
struct StoreObject {
    id: KernelObjectId,
    name: &'static str,
    base: u64,
    length: u64,
}

#[derive(Clone, Copy)]
struct StateVolumeObject {
    id: KernelObjectId,
    name: &'static str,
    value_ready: bool,
    value_len: usize,
    value: [u8; MAX_STATE_VALUE_BYTES],
}

#[derive(Clone, Copy)]
struct TimerObject {
    id: KernelObjectId,
    name: &'static str,
}

#[derive(Clone, Copy)]
struct NetworkPortObject {
    id: KernelObjectId,
    name: &'static str,
}

#[derive(Clone, Copy)]
struct ProcessControlObject {
    id: KernelObjectId,
    name: &'static str,
}

#[derive(Clone, Copy)]
enum KernelObject {
    IpcEndpoint(IpcEndpoint),
    BootModule(BootModuleObject),
    StoreObject(StoreObject),
    StateVolume(StateVolumeObject),
    Timer(TimerObject),
    NetworkPort(NetworkPortObject),
    ProcessControl(ProcessControlObject),
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
    next_id: u64,
}

struct RuntimeState {
    objects: ObjectTable,
    processes: ProcessTable,
}

#[derive(Clone, Copy)]
struct FallbackRuntime {
    generation_id: &'static str,
    config: BootRuntimeConfig,
}

struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

static RUNTIME: Global<RuntimeState> = Global(UnsafeCell::new(RuntimeState::new()));
static FALLBACK_RUNTIME: Global<Option<FallbackRuntime>> = Global(UnsafeCell::new(None));

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

    fn drop(&mut self, slot: u64) -> Result<(), IpcError> {
        let Ok(slot) = usize::try_from(slot) else {
            return Err(IpcError::BadCapability);
        };
        if slot >= self.caps.len() || self.caps[slot].is_none() {
            return Err(IpcError::BadCapability);
        }
        self.caps[slot] = None;
        Ok(())
    }
}

impl Process {
    const fn empty() -> Self {
        Self {
            pid: ProcessId::empty(),
            name: "",
            context: ProcessContext {
                cr3: 0,
                entry: 0,
                stack_top: 0,
            },
            state: ProcessState::Empty,
            caps: CapabilitySpace::new(),
            saved_frame: SyscallFrame::empty(),
            has_saved_frame: false,
            exit_status: 0,
            has_exited: false,
        }
    }

    fn new(
        pid: ProcessId,
        name: &'static str,
        context: ProcessContext,
        state: ProcessState,
        caps: CapabilitySpace,
    ) -> Self {
        Self {
            pid,
            name,
            context,
            state,
            caps,
            saved_frame: SyscallFrame::empty(),
            has_saved_frame: false,
            exit_status: 0,
            has_exited: false,
        }
    }
}

impl IpcEndpoint {
    const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self {
            id,
            name,
            message_ready: false,
            message_len: 0,
            message: [0; MAX_MESSAGE_BYTES],
        }
    }
}

impl BootModuleObject {
    const fn new(id: KernelObjectId, name: &'static str, base: u64, length: u64) -> Self {
        Self {
            id,
            name,
            base,
            length,
        }
    }
}

impl StoreObject {
    const fn new(id: KernelObjectId, name: &'static str, base: u64, length: u64) -> Self {
        Self {
            id,
            name,
            base,
            length,
        }
    }
}

impl StateVolumeObject {
    const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self {
            id,
            name,
            value_ready: false,
            value_len: 0,
            value: [0; MAX_STATE_VALUE_BYTES],
        }
    }
}

impl TimerObject {
    const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self { id, name }
    }
}

impl NetworkPortObject {
    const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self { id, name }
    }
}

impl ProcessControlObject {
    const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self { id, name }
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

    fn reset(&mut self) {
        self.count = 0;
        self.next_id = BOOT_ENDPOINT_ID;
        let mut index = 0;
        while index < self.objects.len() {
            self.objects[index] = None;
            index += 1;
        }
    }

    fn add_endpoint(&mut self, name: &'static str) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::IpcEndpoint(IpcEndpoint::new(id, name)));
        self.count += 1;
        Ok(id)
    }

    fn add_boot_module(
        &mut self,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::BootModule(BootModuleObject::new(
            id, name, base, length,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_store_object(
        &mut self,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::StoreObject(StoreObject::new(
            id, name, base, length,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_state_volume(&mut self, name: &'static str) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] =
            Some(KernelObject::StateVolume(StateVolumeObject::new(id, name)));
        self.count += 1;
        Ok(id)
    }

    fn add_timer(&mut self, name: &'static str) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::Timer(TimerObject::new(id, name)));
        self.count += 1;
        Ok(id)
    }

    fn add_network_port(&mut self, name: &'static str) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::NetworkPort(NetworkPortObject::new(id, name)));
        self.count += 1;
        Ok(id)
    }

    fn add_process_control(&mut self, name: &'static str) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::ProcessControl(ProcessControlObject::new(
            id, name,
        )));
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

    fn get_boot_module(&self, id: KernelObjectId) -> Option<BootModuleObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::BootModule(module)) = self.objects[index]
                && module.id == id
            {
                return Some(module);
            }
            index += 1;
        }

        None
    }

    fn get_store_object(&self, id: KernelObjectId) -> Option<StoreObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::StoreObject(object)) = self.objects[index]
                && object.id == id
            {
                return Some(object);
            }
            index += 1;
        }

        None
    }

    fn get_state_volume_mut(&mut self, id: KernelObjectId) -> Option<&mut StateVolumeObject> {
        let mut found = None;
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::StateVolume(state)) = self.objects[index]
                && state.id == id
            {
                found = Some(index);
                break;
            }
            index += 1;
        }

        match &mut self.objects[found?] {
            Some(KernelObject::StateVolume(state)) => Some(state),
            _ => None,
        }
    }

    fn get_timer(&self, id: KernelObjectId) -> Option<TimerObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::Timer(timer)) = self.objects[index]
                && timer.id == id
            {
                return Some(timer);
            }
            index += 1;
        }

        None
    }

    fn get_process_control(&self, id: KernelObjectId) -> Option<ProcessControlObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::ProcessControl(process_control)) = self.objects[index]
                && process_control.id == id
            {
                return Some(process_control);
            }
            index += 1;
        }

        None
    }
}

impl ProcessTable {
    const fn new() -> Self {
        Self {
            processes: [Some(Process::empty()); MAX_PROCESSES],
            count: 0,
            current: None,
            next_id: 1,
        }
    }

    fn reset(&mut self) {
        self.count = 0;
        self.current = None;
        self.next_id = 1;
        let mut index = 0;
        while index < self.processes.len() {
            self.processes[index] = Some(Process::empty());
            index += 1;
        }
    }

    fn add_process(
        &mut self,
        name: &'static str,
        context: ProcessContext,
        state: ProcessState,
        caps: CapabilitySpace,
    ) -> Result<ProcessId, InitError> {
        if self.count == self.processes.len() {
            return Err(InitError::ProcessTableFull);
        }

        let pid = ProcessId::new(self.next_id);
        self.next_id += 1;
        self.processes[self.count] = Some(Process::new(pid, name, context, state, caps));
        self.count += 1;
        Ok(pid)
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

    fn current_index(&self) -> Option<usize> {
        let pid = self.current?;
        let mut index = 0;
        while index < self.count {
            if let Some(process) = self.processes[index]
                && process.pid == pid
            {
                return Some(index);
            }
            index += 1;
        }

        None
    }

    fn next_ready_index_round_robin(&self) -> Option<usize> {
        if self.count == 0 {
            return None;
        }

        let start = self
            .current_index()
            .map(|index| (index + 1) % self.count)
            .unwrap_or(0);
        let mut offset = 0;

        while offset < self.count {
            let index = (start + offset) % self.count;
            if let Some(process) = self.processes[index]
                && process.state == ProcessState::Ready
            {
                return Some(index);
            }
            offset += 1;
        }

        None
    }

    fn all_exited_successfully(&self) -> bool {
        let mut index = 0;
        while index < self.count {
            if let Some(process) = self.processes[index]
                && (process.state != ProcessState::Exited || process.exit_status != 0)
            {
                return false;
            }
            index += 1;
        }

        true
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

impl BootRuntimeConfig {
    pub const fn new() -> Self {
        Self {
            processes: [None; MAX_PROCESSES],
            process_count: 0,
            endpoints: [None; MAX_OBJECTS],
            endpoint_count: 0,
            manifest_module: None,
            store_objects: [None; MAX_OBJECTS],
            store_object_count: 0,
            state_volumes: [None; MAX_OBJECTS],
            state_volume_count: 0,
            network_ports: [None; MAX_OBJECTS],
            network_port_count: 0,
            grants: [None; MAX_BOOT_GRANTS],
            grant_count: 0,
        }
    }

    pub fn add_process(&mut self, process: BootProcessConfig) -> Result<(), InitError> {
        if self.process_count == self.processes.len() {
            return Err(InitError::ProcessTableFull);
        }
        self.processes[self.process_count] = Some(process);
        self.process_count += 1;
        Ok(())
    }

    pub fn add_endpoint(&mut self, endpoint: BootEndpointConfig) -> Result<(), InitError> {
        if self.endpoint_count == self.endpoints.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.endpoints[self.endpoint_count] = Some(endpoint);
        self.endpoint_count += 1;
        Ok(())
    }

    pub fn set_manifest_module(&mut self, module: BootModuleConfig) {
        self.manifest_module = Some(module);
    }

    pub fn add_store_object(&mut self, object: BootStoreObjectConfig) -> Result<(), InitError> {
        if self.store_object_count == self.store_objects.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.store_objects[self.store_object_count] = Some(object);
        self.store_object_count += 1;
        Ok(())
    }

    pub fn add_state_volume(&mut self, state: BootStateVolumeConfig) -> Result<(), InitError> {
        if self.state_volume_count == self.state_volumes.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.state_volumes[self.state_volume_count] = Some(state);
        self.state_volume_count += 1;
        Ok(())
    }

    pub fn add_network_port(&mut self, port: BootNetworkPortConfig) -> Result<(), InitError> {
        if self.network_port_count == self.network_ports.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.network_ports[self.network_port_count] = Some(port);
        self.network_port_count += 1;
        Ok(())
    }

    pub fn add_grant(&mut self, grant: BootGrantConfig) -> Result<(), InitError> {
        if self.grant_count == self.grants.len() {
            return Err(InitError::CapabilityTableFull);
        }
        if grant.process_index >= self.process_count {
            return Err(InitError::InvalidBootManifest);
        }
        match grant.object_kind {
            BOOT_OBJECT_ENDPOINT if grant.object_index < self.endpoint_count => {}
            BOOT_OBJECT_STORE if grant.object_index < self.store_object_count => {}
            BOOT_OBJECT_STATE if grant.object_index < self.state_volume_count => {}
            BOOT_OBJECT_TIMER if grant.object_index == 0 => {}
            BOOT_OBJECT_NETWORK_PORT if grant.object_index < self.network_port_count => {}
            BOOT_OBJECT_ENDPOINT
            | BOOT_OBJECT_STORE
            | BOOT_OBJECT_STATE
            | BOOT_OBJECT_TIMER
            | BOOT_OBJECT_NETWORK_PORT => return Err(InitError::InvalidBootManifest),
            _ => return Err(InitError::InvalidBootManifest),
        }
        self.grants[self.grant_count] = Some(grant);
        self.grant_count += 1;
        Ok(())
    }
}

pub fn init_from_boot_config(config: &BootRuntimeConfig) -> Result<(), InitError> {
    let runtime = runtime();
    runtime.objects.reset();
    runtime.processes.reset();

    let mut endpoint_ids = [None; MAX_OBJECTS];
    let mut endpoint_index = 0;
    while endpoint_index < config.endpoint_count {
        let endpoint = config.endpoints[endpoint_index].ok_or(InitError::InvalidBootManifest)?;
        endpoint_ids[endpoint_index] = Some(runtime.objects.add_endpoint(endpoint.name)?);
        endpoint_index += 1;
    }

    let mut store_object_ids = [None; MAX_OBJECTS];
    let mut store_index = 0;
    while store_index < config.store_object_count {
        let object = config.store_objects[store_index].ok_or(InitError::InvalidBootManifest)?;
        store_object_ids[store_index] = Some(runtime.objects.add_store_object(
            object.id,
            object.base,
            object.length,
        )?);
        store_index += 1;
    }

    let mut state_volume_ids = [None; MAX_OBJECTS];
    let mut state_index = 0;
    while state_index < config.state_volume_count {
        let state = config.state_volumes[state_index].ok_or(InitError::InvalidBootManifest)?;
        state_volume_ids[state_index] = Some(runtime.objects.add_state_volume(state.id)?);
        state_index += 1;
    }

    let mut network_port_ids = [None; MAX_OBJECTS];
    let mut network_index = 0;
    while network_index < config.network_port_count {
        let port = config.network_ports[network_index].ok_or(InitError::InvalidBootManifest)?;
        network_port_ids[network_index] = Some(runtime.objects.add_network_port(port.id)?);
        network_index += 1;
    }

    let timer_id = runtime.objects.add_timer("monotonic-timer")?;
    let mut process_caps = [CapabilitySpace::new(); MAX_PROCESSES];
    let mut grant_index = 0;
    while grant_index < config.grant_count {
        let grant = config.grants[grant_index].ok_or(InitError::InvalidBootManifest)?;
        let object = match grant.object_kind {
            BOOT_OBJECT_ENDPOINT => {
                endpoint_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)?
            }
            BOOT_OBJECT_STORE => {
                store_object_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)?
            }
            BOOT_OBJECT_STATE => {
                state_volume_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)?
            }
            BOOT_OBJECT_TIMER if grant.object_index == 0 => timer_id,
            BOOT_OBJECT_NETWORK_PORT => {
                network_port_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)?
            }
            _ => return Err(InitError::InvalidBootManifest),
        };
        process_caps[grant.process_index].grant(grant.cap_slot, object, grant.rights)?;
        grant_index += 1;
    }

    let initial_index = initial_process_index(&config)?;
    if let Some(module) = config.manifest_module {
        let module_id = runtime
            .objects
            .add_boot_module(module.name, module.base, module.length)?;
        process_caps[initial_index].grant(0, module_id, capability::RIGHT_READ)?;
    }

    let process_control_id = runtime.objects.add_process_control("process-control")?;
    process_caps[initial_index].grant(2, process_control_id, capability::RIGHT_CONTROL)?;

    let mut saw_initial = false;
    let mut process_index = 0;
    while process_index < config.process_count {
        let process = config.processes[process_index].ok_or(InitError::InvalidBootManifest)?;

        let state = if process.initial {
            if saw_initial {
                return Err(InitError::InvalidBootManifest);
            }
            saw_initial = true;
            ProcessState::Running
        } else {
            ProcessState::Declared
        };

        let pid = runtime.processes.add_process(
            process.name,
            process.context,
            state,
            process_caps[process_index],
        )?;

        if process.initial {
            runtime.processes.set_current(pid);
        }
        process_index += 1;
    }

    if !saw_initial || config.endpoint_count == 0 {
        return Err(InitError::InvalidBootManifest);
    }

    print_boot_tables(runtime);
    Ok(())
}

fn initial_process_index(config: &BootRuntimeConfig) -> Result<usize, InitError> {
    let mut found = None;
    let mut index = 0;

    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        if process.initial {
            if found.is_some() {
                return Err(InitError::InvalidBootManifest);
            }
            found = Some(index);
        }
        index += 1;
    }

    found.ok_or(InitError::InvalidBootManifest)
}

pub fn initial_process_context() -> Option<ProcessContext> {
    runtime()
        .processes
        .current_process()
        .map(|process| process.context)
}

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

pub fn exit_current_process(status: u64, frame: &mut SyscallFrame) -> ScheduleResult {
    let initial_exited = {
        let runtime = runtime();
        runtime
            .processes
            .current_process()
            .map(|process| process.pid.raw() == 1)
            .unwrap_or(true)
    };

    {
        let runtime = runtime();

        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Exited;
            process.has_saved_frame = false;
            process.exit_status = status;
            process.has_exited = true;
        }
    }

    if initial_exited && status != 0 {
        return ScheduleResult::Halt { ok: false };
    }

    if schedule_next_ready(frame) {
        ScheduleResult::Switched
    } else {
        ScheduleResult::Halt {
            ok: runtime().processes.all_exited_successfully(),
        }
    }
}

pub fn yield_current_process(frame: &mut SyscallFrame) -> ScheduleResult {
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return ScheduleResult::Halt { ok: false };
        };

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::Ready;
        process.name
    };

    if schedule_next_ready(frame) {
        ScheduleResult::Switched
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }

        serial::write_str("Scheduler yield: proc=");
        serial::write_str(current);
        serial::write_str(" no other ready process\n");
        ScheduleResult::Continue
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

    wake_blocked_receiver(endpoint_id);

    Ok(())
}

pub fn receive(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
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
    let message_ready = {
        let endpoint = runtime()
            .objects
            .get_endpoint_mut(endpoint_id)
            .ok_or(IpcError::BadCapability)?;
        endpoint.message_ready
    };

    if !message_ready {
        if block_current_on_endpoint(endpoint_id, destination as u64, max_len, frame) {
            return Ok(());
        }

        return Err(IpcError::Empty);
    }

    let copy_len = {
        let endpoint = runtime()
            .objects
            .get_endpoint_mut(endpoint_id)
            .ok_or(IpcError::BadCapability)?;

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

    frame.rax = copy_len as u64;
    Ok(())
}

pub fn read_boot_module(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
) -> Result<usize, IpcError> {
    if max_len > MAX_BOOT_READ_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let module = boot_module_from_cap(cap_slot, capability::RIGHT_READ)?;
    let Ok(module_len) = usize::try_from(module.length) else {
        return Err(IpcError::MessageTooLarge);
    };
    if module_len > max_len {
        return Err(IpcError::MessageTooLarge);
    }
    let copy_len = module_len;

    let bytes = unsafe { core::slice::from_raw_parts(module.base as *const u8, copy_len) };
    usercopy::copy_to_user(UserPtr::new(destination as u64), bytes)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Boot module read accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" module=");
    serial::write_str(module.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");

    Ok(copy_len)
}

pub fn log_write(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let _endpoint_id = endpoint_from_cap(cap_slot, capability::RIGHT_SEND)?;
    let mut message = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut message, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_ascii_bytes(&message[..len]);
    serial::write_str("\n");
    Ok(())
}

pub fn activate_generation(
    cap_slot: u64,
    generation: *const u8,
    len: usize,
) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    let mut generation_id = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut generation_id, UserPtr::new(generation as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Krust process authority accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" generation=");
    serial::write_ascii_bytes(&generation_id[..len]);
    serial::write_str("\n");
    serial::write_str("Krust native generation activation ok\n");
    Ok(())
}

pub fn set_fallback_boot_config(generation_id: &'static str, config: &BootRuntimeConfig) {
    unsafe {
        *FALLBACK_RUNTIME.0.get() = Some(FallbackRuntime {
            generation_id,
            config: *config,
        });
    }
}

pub fn rollback_generation(
    cap_slot: u64,
    generation: *const u8,
    len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_CONTROL)?;

    let mut requested = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut requested, UserPtr::new(generation as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let fallback = match unsafe { *FALLBACK_RUNTIME.0.get() } {
        Some(fallback) => fallback,
        None => {
            serial::write_str("Krust rollback rejected: no fallback runtime\n");
            return Err(IpcError::BadCapability);
        }
    };
    if fallback.generation_id.as_bytes() != &requested[..len] {
        serial::write_str("Krust rollback rejected: requested=");
        serial::write_ascii_bytes(&requested[..len]);
        serial::write_str(" available=");
        serial::write_str(fallback.generation_id);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }

    serial::write_str("Krust rollback generation accepted: target=");
    serial::write_str(fallback.generation_id);
    serial::write_str("\n");

    init_from_boot_config(&fallback.config).map_err(|_| IpcError::BadCapability)?;
    let context = initial_process_context().ok_or(IpcError::BadCapability)?;
    *frame = SyscallFrame::from_context(context);
    unsafe {
        gdt::switch_address_space(context.cr3);
    }
    Ok(())
}

pub fn start_process(cap_slot: u64, process_index: u64) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    let caller = current_process_name();
    let Ok(process_index) = usize::try_from(process_index) else {
        return Err(IpcError::BadCapability);
    };

    let target = {
        let runtime = runtime();
        if process_index >= runtime.processes.count {
            return Err(IpcError::BadCapability);
        }

        let Some(process) = runtime.processes.processes[process_index].as_mut() else {
            return Err(IpcError::BadCapability);
        };

        if process.state != ProcessState::Declared && process.state != ProcessState::Exited {
            return Err(IpcError::BadCapability);
        }

        process.state = ProcessState::Ready;
        process.has_saved_frame = false;
        process.exit_status = 0;
        process.has_exited = false;
        process.name
    };

    serial::write_str("Krust process start accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" target=");
    serial::write_str(target);
    serial::write_str("\n");
    Ok(())
}

pub fn process_status(cap_slot: u64, process_index: u64) -> Result<u64, IpcError> {
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    let Ok(process_index) = usize::try_from(process_index) else {
        return Err(IpcError::BadCapability);
    };

    let runtime = runtime();
    if process_index >= runtime.processes.count {
        return Err(IpcError::BadCapability);
    }

    let Some(process) = runtime.processes.processes[process_index] else {
        return Err(IpcError::BadCapability);
    };

    if process.state == ProcessState::Exited {
        Ok(process.exit_status)
    } else {
        Ok(u64::MAX - 8)
    }
}

pub fn cap_derive(parent_slot: u64, new_slot: u64, rights_mask: u64) -> Result<(), IpcError> {
    let parent = lookup_capability(parent_slot, 0)?;
    if rights_mask == 0 || rights_mask & !parent.rights != 0 {
        return Err(IpcError::BadCapability);
    }

    let process_name = current_process_name();
    let runtime = runtime();
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    process
        .caps
        .grant(new_slot, parent.object, rights_mask)
        .map_err(|_| IpcError::BadCapability)?;

    serial::write_str("Capability derive accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" parent=");
    serial::write_u64_dec(parent_slot);
    serial::write_str(" new=");
    serial::write_u64_dec(new_slot);
    serial::write_str(" rights=");
    print_rights(rights_mask);
    serial::write_str("\n");
    Ok(())
}

pub fn cap_drop(slot: u64) -> Result<(), IpcError> {
    let process_name = current_process_name();
    let runtime = runtime();
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    process.caps.drop(slot)?;

    serial::write_str("Capability drop accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" slot=");
    serial::write_u64_dec(slot);
    serial::write_str("\n");
    Ok(())
}

pub fn cap_transfer(
    control_slot: u64,
    target_process_index: u64,
    packed_transfer: u64,
) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(control_slot, capability::RIGHT_CONTROL)?;
    let cap_slot = packed_transfer & 0xffff;
    let target_slot = (packed_transfer >> 16) & 0xffff;
    let rights_mask = packed_transfer >> 32;
    let cap = lookup_capability(cap_slot, 0)?;
    if rights_mask == 0 || rights_mask & !cap.rights != 0 {
        return Err(IpcError::BadCapability);
    }
    let Ok(target_process_index) = usize::try_from(target_process_index) else {
        return Err(IpcError::BadCapability);
    };

    let (caller, target) = {
        let runtime = runtime();
        let caller = runtime
            .processes
            .current_process()
            .map(|process| process.name)
            .unwrap_or("<none>");
        if target_process_index >= runtime.processes.count {
            return Err(IpcError::BadCapability);
        }
        let Some(target_process) = runtime.processes.processes[target_process_index].as_mut()
        else {
            return Err(IpcError::BadCapability);
        };
        target_process
            .caps
            .grant(target_slot, cap.object, rights_mask)
            .map_err(|_| IpcError::BadCapability)?;
        (caller, target_process.name)
    };

    serial::write_str("Capability transfer accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" target=");
    serial::write_str(target);
    serial::write_str(" slot=");
    serial::write_u64_dec(target_slot);
    serial::write_str(" rights=");
    print_rights(rights_mask);
    serial::write_str("\n");
    Ok(())
}

pub fn object_read(cap_slot: u64, destination: *mut u8, max_len: usize) -> Result<usize, IpcError> {
    let object = store_object_from_cap(cap_slot, capability::RIGHT_READ)?;
    let Ok(object_len) = usize::try_from(object.length) else {
        return Err(IpcError::MessageTooLarge);
    };
    let copy_len = min(object_len, max_len);
    let bytes = unsafe { core::slice::from_raw_parts(object.base as *const u8, copy_len) };
    usercopy::copy_to_user(UserPtr::new(destination as u64), bytes)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Object read accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" object=");
    serial::write_str(object.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");
    Ok(copy_len)
}

pub fn state_write(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_STATE_VALUE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let state_id = state_volume_from_cap(cap_slot, capability::RIGHT_WRITE)?;
    let mut value = [0u8; MAX_STATE_VALUE_BYTES];
    usercopy::copy_from_user(&mut value, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let process_name = current_process_name();
    let state = runtime()
        .objects
        .get_state_volume_mut(state_id)
        .ok_or(IpcError::BadCapability)?;
    state.value[..len].copy_from_slice(&value[..len]);
    state.value_len = len;
    state.value_ready = true;

    serial::write_str("State write accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" state=");
    serial::write_str(state.name);
    serial::write_str("\n");
    Ok(())
}

pub fn state_read(cap_slot: u64, destination: *mut u8, max_len: usize) -> Result<usize, IpcError> {
    let state_id = state_volume_from_cap(cap_slot, capability::RIGHT_READ)?;
    let (name, value, copy_len) = {
        let state = runtime()
            .objects
            .get_state_volume_mut(state_id)
            .ok_or(IpcError::BadCapability)?;
        if !state.value_ready {
            return Err(IpcError::Empty);
        }
        let copy_len = min(state.value_len, max_len);
        let mut value = [0u8; MAX_STATE_VALUE_BYTES];
        value[..copy_len].copy_from_slice(&state.value[..copy_len]);
        (state.name, value, copy_len)
    };

    usercopy::copy_to_user(UserPtr::new(destination as u64), &value[..copy_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("State read accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" state=");
    serial::write_str(name);
    serial::write_str("\n");
    Ok(copy_len)
}

pub fn sleep_ms(cap_slot: u64, milliseconds: u64) -> Result<(), IpcError> {
    let timer = timer_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    serial::write_str("Timer sleep accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" timer=");
    serial::write_str(timer.name);
    serial::write_str(" ms=");
    serial::write_u64_dec(milliseconds);
    serial::write_str("\n");

    let start = read_tsc();
    let cycles = milliseconds.saturating_mul(1_000_000);
    while read_tsc().wrapping_sub(start) < cycles {
        core::hint::spin_loop();
    }

    Ok(())
}

fn read_tsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "rdtsc",
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags)
        );
    }
    ((high as u64) << 32) | low as u64
}

fn block_current_on_endpoint(
    endpoint: KernelObjectId,
    destination: u64,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> bool {
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return false;
        };

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnEndpoint {
            endpoint,
            destination,
            max_len,
        };
        process.name
    };

    serial::write_str("IPC receive blocked: proc=");
    serial::write_str(current);
    serial::write_str(" endpoint=");
    serial::write_u64_dec(endpoint.raw());
    serial::write_str("\n");

    if schedule_next_ready(frame) {
        true
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }

        serial::write_str("Scheduler blocked: proc=");
        serial::write_str(current);
        serial::write_str(" no ready process\n");
        false
    }
}

fn wake_blocked_receiver(endpoint: KernelObjectId) {
    let mut message = [0u8; MAX_MESSAGE_BYTES];
    let message_len = {
        let runtime = runtime();
        let Some(endpoint_object) = runtime.objects.get_endpoint_mut(endpoint) else {
            return;
        };
        if !endpoint_object.message_ready {
            return;
        }

        let message_len = endpoint_object.message_len;
        message[..message_len].copy_from_slice(&endpoint_object.message[..message_len]);
        message_len
    };

    let Some(waiter_index) = blocked_receiver_index(endpoint) else {
        return;
    };

    let (name, receiver_cr3, destination, max_len, current_cr3) = {
        let runtime = runtime();
        let Some(waiter) = runtime.processes.processes[waiter_index] else {
            return;
        };
        let ProcessState::BlockedOnEndpoint {
            destination,
            max_len,
            ..
        } = waiter.state
        else {
            return;
        };

        let current_cr3 = runtime
            .processes
            .current_process()
            .map(|process| process.context.cr3)
            .unwrap_or_else(paging::active_root_table_physical);

        (
            waiter.name,
            waiter.context.cr3,
            destination,
            max_len,
            current_cr3,
        )
    };

    let copy_len = min(message_len, max_len);
    let copy_result = unsafe {
        gdt::switch_address_space(receiver_cr3);
        let result = usercopy::copy_to_user(UserPtr::new(destination), &message[..copy_len]);
        gdt::switch_address_space(current_cr3);
        result
    };

    match copy_result {
        Ok(()) => {
            {
                let runtime = runtime();
                let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() else {
                    return;
                };
                waiter.saved_frame.rax = copy_len as u64;
                waiter.state = ProcessState::Ready;
            }

            let runtime = runtime();
            if let Some(endpoint_object) = runtime.objects.get_endpoint_mut(endpoint) {
                endpoint_object.message_ready = false;
            }

            serial::write_str("IPC receive delivered: endpoint=");
            serial::write_u64_dec(endpoint.raw());
            serial::write_str(" bytes=");
            serial::write_u64_dec(copy_len as u64);
            serial::write_str("\n");

            serial::write_str("IPC wake receiver: proc=");
            serial::write_str(name);
            serial::write_str(" endpoint=");
            serial::write_u64_dec(endpoint.raw());
            serial::write_str("\n");
        }
        Err(_) => {
            {
                let runtime = runtime();
                let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() else {
                    return;
                };
                waiter.saved_frame.rax = STATUS_BAD_BUFFER;
                waiter.state = ProcessState::Ready;
            }
            serial::write_str("IPC wake receiver failed: bad user buffer proc=");
            serial::write_str(name);
            serial::write_str("\n");
        }
    }
}

fn blocked_receiver_index(endpoint: KernelObjectId) -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;

    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnEndpoint {
                endpoint: waiting_endpoint,
                ..
            } = process.state
            && waiting_endpoint == endpoint
        {
            return Some(index);
        }
        index += 1;
    }

    None
}

fn schedule_next_ready(frame: &mut SyscallFrame) -> bool {
    let (from, to, next_frame, next_cr3) = {
        let runtime = runtime();
        let from = runtime
            .processes
            .current_process()
            .map(|process| process.name)
            .unwrap_or("<none>");
        let Some(next_index) = runtime.processes.next_ready_index_round_robin() else {
            return false;
        };
        let (next_pid, to, next_frame, next_cr3) = {
            let Some(next) = runtime.processes.processes[next_index].as_mut() else {
                return false;
            };

            next.state = ProcessState::Running;

            let next_frame = if next.has_saved_frame {
                next.saved_frame
            } else {
                SyscallFrame::from_context(next.context)
            };

            (next.pid, next.name, next_frame, next.context.cr3)
        };

        runtime.processes.current = Some(next_pid);

        (from, to, next_frame, next_cr3)
    };

    *frame = next_frame;

    serial::write_str("Scheduler switch: from=");
    serial::write_str(from);
    serial::write_str(" to=");
    serial::write_str(to);
    serial::write_str("\n");

    unsafe {
        gdt::switch_address_space(next_cr3);
    }

    true
}

fn endpoint_from_cap(cap_slot: u64, required_right: u64) -> Result<KernelObjectId, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;

    match runtime().objects.get_endpoint_mut(cap.object) {
        Some(_) => Ok(cap.object),
        None => Err(IpcError::BadCapability),
    }
}

fn boot_module_from_cap(cap_slot: u64, required_right: u64) -> Result<BootModuleObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_boot_module(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn store_object_from_cap(cap_slot: u64, required_right: u64) -> Result<StoreObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_store_object(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn state_volume_from_cap(cap_slot: u64, required_right: u64) -> Result<KernelObjectId, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    match runtime().objects.get_state_volume_mut(cap.object) {
        Some(_) => Ok(cap.object),
        None => Err(IpcError::BadCapability),
    }
}

fn timer_from_cap(cap_slot: u64, required_right: u64) -> Result<TimerObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_timer(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn process_control_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<ProcessControlObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_process_control(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn lookup_capability(cap_slot: u64, required_right: u64) -> Result<Capability, IpcError> {
    let process = runtime()
        .processes
        .current_process()
        .ok_or(IpcError::BadCapability)?;
    let cap = process
        .caps
        .lookup(cap_slot)
        .ok_or(IpcError::BadCapability)?;

    if required_right != 0 && cap.rights & required_right != required_right {
        return Err(IpcError::BadCapability);
    }

    Ok(cap)
}

fn print_boot_tables(runtime: &RuntimeState) {
    serial::write_str("Process table entries: ");
    serial::write_u64_dec(runtime.processes.count as u64);
    serial::write_str("\n");

    serial::write_str("Endpoint table entries: ");
    serial::write_u64_dec(runtime.objects.endpoint_count() as u64);
    serial::write_str("\n");

    print_endpoint_labels(runtime);

    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index] {
            print_process_state(index, &process);
            print_process_caps(&process);
        }
        index += 1;
    }
}

fn print_endpoint_labels(runtime: &RuntimeState) {
    let mut printed = 0;
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(KernelObject::IpcEndpoint(endpoint)) = runtime.objects.objects[index] {
            serial::write_str("endpoint[");
            serial::write_u64_dec(printed as u64);
            serial::write_str("] id=");
            serial::write_u64_dec(endpoint.id.raw());
            serial::write_str(" name=");
            serial::write_str(endpoint.name);
            serial::write_str("\n");
            printed += 1;
        }
        index += 1;
    }
}

fn print_process_caps(process: &Process) {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = process.caps.caps[slot] {
            serial::write_str("proc=");
            serial::write_str(process.name);
            serial::write_str(" cap[");
            serial::write_u64_dec(slot as u64);
            serial::write_str("] ");
            print_capability_object(cap.object);
            serial::write_str(" rights=");
            print_rights(cap.rights);
            serial::write_str("\n");
        }
        slot += 1;
    }
}

fn print_capability_object(object: KernelObjectId) {
    let runtime = runtime();
    let mut index = 0;

    while index < runtime.objects.count {
        if let Some(entry) = runtime.objects.objects[index] {
            match entry {
                KernelObject::IpcEndpoint(endpoint) if endpoint.id == object => {
                    serial::write_str("endpoint=");
                    serial::write_str(endpoint.name);
                    return;
                }
                KernelObject::BootModule(module) if module.id == object => {
                    serial::write_str("boot-module=");
                    serial::write_str(module.name);
                    return;
                }
                KernelObject::StoreObject(store) if store.id == object => {
                    serial::write_str("store-object=");
                    serial::write_str(store.name);
                    return;
                }
                KernelObject::StateVolume(state) if state.id == object => {
                    serial::write_str("state-volume=");
                    serial::write_str(state.name);
                    return;
                }
                KernelObject::Timer(timer) if timer.id == object => {
                    serial::write_str("timer=");
                    serial::write_str(timer.name);
                    return;
                }
                KernelObject::NetworkPort(port) if port.id == object => {
                    serial::write_str("network-port=");
                    serial::write_str(port.name);
                    return;
                }
                KernelObject::ProcessControl(process_control) if process_control.id == object => {
                    serial::write_str("process-control=");
                    serial::write_str(process_control.name);
                    return;
                }
                _ => {}
            }
        }
        index += 1;
    }

    serial::write_str("object=");
    serial::write_u64_dec(object.raw());
}

fn print_process_state(index: usize, process: &Process) {
    serial::write_str("process[");
    serial::write_u64_dec(index as u64);
    serial::write_str("] id=");
    serial::write_u64_dec(process.pid.raw());
    serial::write_str(" name=");
    serial::write_str(process.name);
    serial::write_str(" state=");
    serial::write_str(process.state.label());
    serial::write_str("\n");
}

fn print_rights(rights: u64) {
    let mut wrote = false;
    wrote = print_right(rights, capability::RIGHT_READ, "read", wrote);
    wrote = print_right(rights, capability::RIGHT_WRITE, "write", wrote);
    wrote = print_right(rights, capability::RIGHT_SEND, "send", wrote);
    wrote = print_right(rights, capability::RIGHT_RECEIVE, "receive", wrote);
    wrote = print_right(rights, capability::RIGHT_CONTROL, "control", wrote);
    wrote = print_right(rights, capability::RIGHT_SNAPSHOT, "snapshot", wrote);
    wrote = print_right(rights, capability::RIGHT_RESTORE, "restore", wrote);
    wrote = print_right(rights, capability::RIGHT_BIND, "bind", wrote);
    wrote = print_right(rights, capability::RIGHT_LISTEN, "listen", wrote);

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
    serial::write_str(current_process_label());
    serial::write_str(" ");
    serial::write_str(operation);
    serial::write_str(" rejected: bad capability\n");
}

fn current_process_label() -> &'static str {
    runtime()
        .processes
        .current_process()
        .map(|process| process.name)
        .unwrap_or("<none>")
}

fn runtime() -> &'static mut RuntimeState {
    unsafe { &mut *RUNTIME.0.get() }
}

fn min(left: usize, right: usize) -> usize {
    if left < right { left } else { right }
}
