use core::{
    arch::{asm, x86_64::__cpuid_count},
    cell::UnsafeCell,
};

use crate::{
    capability, gdt, limine, memory, paging, serial,
    usercopy::{self, UserPtr},
};

pub const BOOT_ENDPOINT_ID: u64 = 1;

const MAX_MESSAGE_BYTES: usize = 512;
const ENDPOINT_QUEUE_CAPACITY: usize = 4;
const MAX_BOOT_READ_BYTES: usize = 16 * 1024;
const MAX_OBJECTS: usize = 64;
const MAX_PROCESSES: usize = 16;
const MAX_CAPS: usize = 32;
const MAX_BOOT_GRANTS: usize = 128;
const MAX_REVOKED_CAPS: usize = 128;
const MAX_GENERATION_CONFIGS: usize = 4;
const MAX_INSPECT_REPORT_BYTES: usize = 32 * 1024;
const DMA_MAPPING_INFO_BYTES: usize = 24;
const USER_DMA_MAPPING_BASE: u64 = 0x0000_6000_0000_0000;
const INITIAL_USER_RFLAGS: u64 = 0x202;
const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;
const STATUS_OK: u64 = 0;
const STATUS_TIMEOUT: u64 = u64::MAX - 9;
pub const STATUS_PROCESS_FAULT: u64 = u64::MAX - 10;
const FALLBACK_TSC_TICKS_PER_MS: u64 = 1_000_000;
pub const BOOT_OBJECT_ENDPOINT: u16 = 1;
pub const BOOT_OBJECT_STORE: u16 = 2;
pub const BOOT_OBJECT_STATE: u16 = 3;
pub const BOOT_OBJECT_TIMER: u16 = 4;
pub const BOOT_OBJECT_NETWORK_PORT: u16 = 5;
pub const BOOT_OBJECT_IO_PORT_RANGE: u16 = 6;
pub const BOOT_OBJECT_MMIO_REGION: u16 = 7;
pub const BOOT_OBJECT_INTERRUPT_LINE: u16 = 8;
pub const BOOT_OBJECT_DMA_REGION: u16 = 9;

pub const FRAME_R15: usize = 0;
pub const FRAME_R14: usize = 8;
pub const FRAME_R13: usize = 16;
pub const FRAME_R12: usize = 24;
pub const FRAME_R11: usize = 32;
pub const FRAME_R10: usize = 40;
pub const FRAME_R9: usize = 48;
pub const FRAME_R8: usize = 56;
pub const FRAME_RSI: usize = 64;
pub const FRAME_RDI: usize = 72;
pub const FRAME_RBP: usize = 80;
pub const FRAME_RDX: usize = 88;
pub const FRAME_RCX: usize = 96;
pub const FRAME_RBX: usize = 104;
pub const FRAME_RAX: usize = 112;
pub const FRAME_USER_RIP: usize = 120;
pub const FRAME_USER_CS: usize = 128;
pub const FRAME_USER_RFLAGS: usize = 136;
pub const FRAME_USER_RSP: usize = 144;
pub const FRAME_USER_SS: usize = 152;
pub const FRAME_SIZE: usize = 160;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SyscallFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    pub rax: u64,
    pub user_rip: u64,
    pub user_cs: u64,
    pub user_rflags: u64,
    pub user_rsp: u64,
    pub user_ss: u64,
}

impl SyscallFrame {
    const fn empty() -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,
            user_rip: 0,
            user_cs: 0,
            user_rflags: 0,
            user_rsp: 0,
            user_ss: 0,
        }
    }

    fn from_context(context: ProcessContext) -> Self {
        Self {
            user_rip: context.entry,
            user_cs: gdt::USER_CODE_SELECTOR as u64,
            user_rflags: INITIAL_USER_RFLAGS,
            user_rsp: context.stack_top,
            user_ss: gdt::USER_DATA_SELECTOR as u64,
            ..Self::empty()
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
    pub restart_context: ProcessContext,
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
    pub hash: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootNetworkPortConfig {
    pub id: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootIoPortRangeConfig {
    pub id: &'static str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct BootMmioRegionConfig {
    pub id: &'static str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct BootInterruptLineConfig {
    pub id: &'static str,
    pub line: u64,
}

#[derive(Clone, Copy)]
pub struct BootDmaRegionConfig {
    pub id: &'static str,
    pub base: u64,
    pub length: u64,
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
    generation_id: &'static str,
    manifest_hash: [u8; 64],
    processes: [Option<BootProcessConfig>; MAX_PROCESSES],
    process_count: usize,
    endpoints: [Option<BootEndpointConfig>; MAX_OBJECTS],
    endpoint_count: usize,
    manifest_module: Option<BootModuleConfig>,
    store_objects: [Option<BootStoreObjectConfig>; MAX_OBJECTS],
    store_object_count: usize,
    network_ports: [Option<BootNetworkPortConfig>; MAX_OBJECTS],
    network_port_count: usize,
    io_ports: [Option<BootIoPortRangeConfig>; MAX_OBJECTS],
    io_port_count: usize,
    mmio_regions: [Option<BootMmioRegionConfig>; MAX_OBJECTS],
    mmio_region_count: usize,
    interrupt_lines: [Option<BootInterruptLineConfig>; MAX_OBJECTS],
    interrupt_line_count: usize,
    dma_regions: [Option<BootDmaRegionConfig>; MAX_OBJECTS],
    dma_region_count: usize,
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
        timeout_tsc: Option<u64>,
    },
    Sleeping {
        wake_tsc: u64,
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
            Self::Sleeping { .. } => "sleeping",
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
    id: u64,
    object: KernelObjectId,
    rights: u64,
    owner_process: ProcessId,
    parent_cap_id: u64,
    generation_id: &'static str,
    delegated_by: ProcessId,
    revoked: bool,
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
    restart_context: ProcessContext,
    state: ProcessState,
    caps: CapabilitySpace,
    initial_caps: CapabilitySpace,
    saved_frame: SyscallFrame,
    has_saved_frame: bool,
    exit_status: u64,
    has_exited: bool,
    start_count: u64,
    quota: ProcessQuota,
}

#[derive(Clone, Copy)]
struct ProcessQuota {
    max_caps: u64,
    max_endpoints: u64,
    max_memory_pages: u64,
    max_child_processes: u64,
    max_ipc_bytes: u64,
    used_endpoints: u64,
}

#[derive(Clone, Copy)]
struct IpcMessage {
    sender: ProcessId,
    len: usize,
    bytes: [u8; MAX_MESSAGE_BYTES],
}

#[derive(Clone, Copy)]
struct IpcEndpoint {
    id: KernelObjectId,
    name: &'static str,
    queue: [IpcMessage; ENDPOINT_QUEUE_CAPACITY],
    queue_len: usize,
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
    hash: &'static str,
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
struct IoPortRangeObject {
    id: KernelObjectId,
    name: &'static str,
    base: u64,
    length: u64,
}

#[derive(Clone, Copy)]
struct MmioRegionObject {
    id: KernelObjectId,
    name: &'static str,
    base: u64,
    length: u64,
}

#[derive(Clone, Copy)]
struct InterruptLineObject {
    id: KernelObjectId,
    name: &'static str,
    line: u64,
}

#[derive(Clone, Copy)]
struct DmaRegionObject {
    id: KernelObjectId,
    name: &'static str,
    base: u64,
    length: u64,
}

#[derive(Clone, Copy)]
struct ProcessControlObject {
    id: KernelObjectId,
    name: &'static str,
}

struct InspectReport {
    bytes: [u8; MAX_INSPECT_REPORT_BYTES],
    len: usize,
    truncated: bool,
}

#[derive(Clone, Copy)]
enum KernelObject {
    IpcEndpoint(IpcEndpoint),
    BootModule(BootModuleObject),
    StoreObject(StoreObject),
    Timer(TimerObject),
    NetworkPort(NetworkPortObject),
    IoPortRange(IoPortRangeObject),
    MmioRegion(MmioRegionObject),
    InterruptLine(InterruptLineObject),
    DmaRegion(DmaRegionObject),
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
    generation_id: &'static str,
    active_config: Option<&'static BootRuntimeConfig>,
    next_cap_id: u64,
    revoked_caps: [u64; MAX_REVOKED_CAPS],
    revoked_cap_count: usize,
}

#[derive(Clone, Copy)]
struct GenerationRuntime {
    generation_id: &'static str,
    config: &'static BootRuntimeConfig,
}

struct GenerationRuntimeTable {
    entries: [Option<GenerationRuntime>; MAX_GENERATION_CONFIGS],
    count: usize,
}

struct BootManagerState {
    selected_generation: &'static str,
    previous_generation: &'static str,
    known_good_generation: &'static str,
    last_failed_generation: &'static str,
    boot_attempt_counter: u64,
}

struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

static RUNTIME: Global<RuntimeState> = Global(UnsafeCell::new(RuntimeState::new()));
static GENERATION_RUNTIMES: Global<GenerationRuntimeTable> =
    Global(UnsafeCell::new(GenerationRuntimeTable::new()));
static ROLLBACK_RUNTIME: Global<Option<GenerationRuntime>> = Global(UnsafeCell::new(None));
static FAILED_GENERATION: Global<Option<&'static str>> = Global(UnsafeCell::new(None));
static BOOT_MANAGER: Global<BootManagerState> = Global(UnsafeCell::new(BootManagerState::new()));
static FRAME_ALLOCATOR: Global<Option<*mut memory::FrameAllocator>> = Global(UnsafeCell::new(None));

impl CapabilitySpace {
    const fn new() -> Self {
        Self {
            caps: [None; MAX_CAPS],
        }
    }

    fn grant(&mut self, slot: u64, cap: Capability) -> Result<(), InitError> {
        let Ok(slot) = usize::try_from(slot) else {
            return Err(InitError::CapabilityTableFull);
        };
        if slot >= self.caps.len() {
            return Err(InitError::CapabilityTableFull);
        }
        if self.caps[slot].is_some() {
            return Err(InitError::InvalidBootManifest);
        }

        self.caps[slot] = Some(cap);
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

    fn clear(&mut self, slot: u64) -> Result<Capability, IpcError> {
        let Ok(slot) = usize::try_from(slot) else {
            return Err(IpcError::BadCapability);
        };
        if slot >= self.caps.len() {
            return Err(IpcError::BadCapability);
        }
        let Some(cap) = self.caps[slot] else {
            return Err(IpcError::BadCapability);
        };
        self.caps[slot] = None;
        Ok(cap)
    }

    fn mark_revoked(&mut self, cap_id: u64) {
        let mut index = 0;
        while index < self.caps.len() {
            if let Some(mut cap) = self.caps[index]
                && cap.id == cap_id
            {
                cap.revoked = true;
                self.caps[index] = Some(cap);
            }
            index += 1;
        }
    }
}

impl ProcessQuota {
    const fn initial() -> Self {
        Self {
            max_caps: MAX_CAPS as u64,
            max_endpoints: 1,
            max_memory_pages: 0,
            max_child_processes: MAX_PROCESSES as u64,
            max_ipc_bytes: MAX_MESSAGE_BYTES as u64,
            used_endpoints: 0,
        }
    }

    const fn service() -> Self {
        Self {
            max_caps: MAX_CAPS as u64,
            max_endpoints: 0,
            max_memory_pages: 0,
            max_child_processes: 0,
            max_ipc_bytes: MAX_MESSAGE_BYTES as u64,
            used_endpoints: 0,
        }
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
            restart_context: ProcessContext {
                cr3: 0,
                entry: 0,
                stack_top: 0,
            },
            state: ProcessState::Empty,
            caps: CapabilitySpace::new(),
            initial_caps: CapabilitySpace::new(),
            saved_frame: SyscallFrame::empty(),
            has_saved_frame: false,
            exit_status: 0,
            has_exited: false,
            start_count: 0,
            quota: ProcessQuota::service(),
        }
    }

    fn new(
        pid: ProcessId,
        name: &'static str,
        context: ProcessContext,
        restart_context: ProcessContext,
        state: ProcessState,
        caps: CapabilitySpace,
    ) -> Self {
        let initial = state == ProcessState::Running;
        let start_count = if initial { 1 } else { 0 };
        Self {
            pid,
            name,
            context,
            restart_context,
            state,
            caps,
            initial_caps: caps,
            saved_frame: SyscallFrame::empty(),
            has_saved_frame: false,
            exit_status: 0,
            has_exited: false,
            start_count,
            quota: if initial {
                ProcessQuota::initial()
            } else {
                ProcessQuota::service()
            },
        }
    }
}

impl IpcEndpoint {
    const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self {
            id,
            name,
            queue: [IpcMessage::empty(); ENDPOINT_QUEUE_CAPACITY],
            queue_len: 0,
        }
    }

    fn enqueue(
        &mut self,
        sender: ProcessId,
        bytes: &[u8; MAX_MESSAGE_BYTES],
        len: usize,
    ) -> Result<(), IpcError> {
        if self.queue_len == ENDPOINT_QUEUE_CAPACITY {
            return Err(IpcError::MessageTooLarge);
        }

        let mut message = IpcMessage::empty();
        message.sender = sender;
        message.len = len;
        message.bytes[..len].copy_from_slice(&bytes[..len]);
        self.queue[self.queue_len] = message;
        self.queue_len += 1;
        Ok(())
    }

    fn has_message_for(&self, receiver: ProcessId) -> bool {
        let mut index = 0;
        while index < self.queue_len {
            if self.queue[index].sender != receiver {
                return true;
            }
            index += 1;
        }
        false
    }

    fn dequeue_for(&mut self, receiver: ProcessId) -> Option<IpcMessage> {
        let mut index = 0;
        while index < self.queue_len {
            if self.queue[index].sender != receiver {
                let message = self.queue[index];
                while index + 1 < self.queue_len {
                    self.queue[index] = self.queue[index + 1];
                    index += 1;
                }
                self.queue_len -= 1;
                self.queue[self.queue_len] = IpcMessage::empty();
                return Some(message);
            }
            index += 1;
        }
        None
    }
}

impl IpcMessage {
    const fn empty() -> Self {
        Self {
            sender: ProcessId::empty(),
            len: 0,
            bytes: [0; MAX_MESSAGE_BYTES],
        }
    }
}

pub fn run_fifo_regression() {
    let provider = ProcessId::new(1);
    let client_a = ProcessId::new(2);
    let client_b = ProcessId::new(3);
    let mut endpoint = IpcEndpoint::new(KernelObjectId(0xf100), "fifo-regression");
    let mut message = [0u8; MAX_MESSAGE_BYTES];

    message[0] = b'a';
    if endpoint.enqueue(client_a, &message, 1).is_err() {
        fifo_regression_failed("enqueue a");
        return;
    }
    message[0] = b'b';
    if endpoint.enqueue(client_b, &message, 1).is_err() {
        fifo_regression_failed("enqueue b");
        return;
    }
    if endpoint
        .dequeue_for(provider)
        .map(|queued| queued.len == 1 && queued.bytes[0] == b'a')
        != Some(true)
    {
        fifo_regression_failed("fifo first");
        return;
    }
    if endpoint
        .dequeue_for(provider)
        .map(|queued| queued.len == 1 && queued.bytes[0] == b'b')
        != Some(true)
    {
        fifo_regression_failed("fifo second");
        return;
    }
    serial::write_str("IPC FIFO regression: queued sends preserve FIFO order\n");

    let mut full_endpoint = IpcEndpoint::new(KernelObjectId(0xf101), "fifo-full-regression");
    let mut index = 0;
    while index < ENDPOINT_QUEUE_CAPACITY {
        message[0] = b'0' + index as u8;
        if full_endpoint.enqueue(client_a, &message, 1).is_err() {
            fifo_regression_failed("fill queue");
            return;
        }
        index += 1;
    }
    if !matches!(
        full_endpoint.enqueue(client_b, &message, 1),
        Err(IpcError::MessageTooLarge)
    ) {
        fifo_regression_failed("queue full");
        return;
    }
    serial::write_str("IPC FIFO regression: queue-full send rejected\n");

    let mut receiver_endpoint =
        IpcEndpoint::new(KernelObjectId(0xf102), "fifo-receiver-regression");
    message[0] = b'a';
    if receiver_endpoint.enqueue(client_a, &message, 1).is_err() {
        fifo_regression_failed("receiver enqueue a");
        return;
    }
    if receiver_endpoint.has_message_for(client_a) {
        fifo_regression_failed("self message visible");
        return;
    }
    if !receiver_endpoint.has_message_for(client_b) {
        fifo_regression_failed("other receiver hidden");
        return;
    }
    message[0] = b'b';
    if receiver_endpoint.enqueue(client_b, &message, 1).is_err() {
        fifo_regression_failed("receiver enqueue b");
        return;
    }
    if !receiver_endpoint.has_message_for(client_a) || !receiver_endpoint.has_message_for(client_b)
    {
        fifo_regression_failed("blocked receiver eligibility");
        return;
    }
    if receiver_endpoint
        .dequeue_for(client_a)
        .map(|queued| queued.len == 1 && queued.bytes[0] == b'b')
        != Some(true)
    {
        fifo_regression_failed("receiver a eligible message");
        return;
    }
    if receiver_endpoint
        .dequeue_for(client_b)
        .map(|queued| queued.len == 1 && queued.bytes[0] == b'a')
        != Some(true)
    {
        fifo_regression_failed("receiver b eligible message");
        return;
    }
    serial::write_str(
        "IPC FIFO regression: receiver-specific dequeue preserves eligible ordering\n",
    );
    serial::write_str("IPC FIFO regression: multiple blocked receivers match eligible messages\n");
    serial::write_str("IPC FIFO regression ok\n");
}

fn fifo_regression_failed(reason: &str) {
    serial::write_str("IPC FIFO regression failed: ");
    serial::write_str(reason);
    serial::write_str("\n");
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
    const fn new(
        id: KernelObjectId,
        name: &'static str,
        base: u64,
        length: u64,
        hash: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            base,
            length,
            hash,
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

impl IoPortRangeObject {
    const fn new(id: KernelObjectId, name: &'static str, base: u64, length: u64) -> Self {
        Self {
            id,
            name,
            base,
            length,
        }
    }
}

impl MmioRegionObject {
    const fn new(id: KernelObjectId, name: &'static str, base: u64, length: u64) -> Self {
        Self {
            id,
            name,
            base,
            length,
        }
    }
}

impl InterruptLineObject {
    const fn new(id: KernelObjectId, name: &'static str, line: u64) -> Self {
        Self { id, name, line }
    }
}

impl DmaRegionObject {
    const fn new(id: KernelObjectId, name: &'static str, base: u64, length: u64) -> Self {
        Self {
            id,
            name,
            base,
            length,
        }
    }
}

impl ProcessControlObject {
    const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self { id, name }
    }
}

impl InspectReport {
    const fn new() -> Self {
        Self {
            bytes: [0; MAX_INSPECT_REPORT_BYTES],
            len: 0,
            truncated: false,
        }
    }

    fn push_byte(&mut self, byte: u8) {
        if self.len == self.bytes.len() {
            self.truncated = true;
            return;
        }
        self.bytes[self.len] = byte;
        self.len += 1;
    }

    fn push_str(&mut self, value: &str) {
        self.push_bytes(value.as_bytes());
    }

    fn push_bytes(&mut self, value: &[u8]) {
        let mut index = 0;
        while index < value.len() {
            self.push_byte(value[index]);
            index += 1;
        }
    }

    fn push_u64_dec(&mut self, mut value: u64) {
        if value == 0 {
            self.push_byte(b'0');
            return;
        }

        let mut digits = [0u8; 20];
        let mut len = 0;
        while value > 0 {
            digits[len] = b'0' + (value % 10) as u8;
            value /= 10;
            len += 1;
        }
        while len > 0 {
            len -= 1;
            self.push_byte(digits[len]);
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
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
        hash: &'static str,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::StoreObject(StoreObject::new(
            id, name, base, length, hash,
        )));
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
        self.objects[self.count] =
            Some(KernelObject::NetworkPort(NetworkPortObject::new(id, name)));
        self.count += 1;
        Ok(id)
    }

    fn add_io_port(
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
        self.objects[self.count] = Some(KernelObject::IoPortRange(IoPortRangeObject::new(
            id, name, base, length,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_mmio_region(
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
        self.objects[self.count] = Some(KernelObject::MmioRegion(MmioRegionObject::new(
            id, name, base, length,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_interrupt_line(
        &mut self,
        name: &'static str,
        line: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::InterruptLine(InterruptLineObject::new(
            id, name, line,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_dma_region(
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
        self.objects[self.count] = Some(KernelObject::DmaRegion(DmaRegionObject::new(
            id, name, base, length,
        )));
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

    fn get_endpoint(&self, id: KernelObjectId) -> Option<IpcEndpoint> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::IpcEndpoint(endpoint)) = self.objects[index]
                && endpoint.id == id
            {
                return Some(endpoint);
            }
            index += 1;
        }
        None
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

    fn get_io_port(&self, id: KernelObjectId) -> Option<IoPortRangeObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::IoPortRange(port)) = self.objects[index]
                && port.id == id
            {
                return Some(port);
            }
            index += 1;
        }

        None
    }

    fn get_mmio_region(&self, id: KernelObjectId) -> Option<MmioRegionObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::MmioRegion(region)) = self.objects[index]
                && region.id == id
            {
                return Some(region);
            }
            index += 1;
        }

        None
    }

    fn get_interrupt_line(&self, id: KernelObjectId) -> Option<InterruptLineObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::InterruptLine(line)) = self.objects[index]
                && line.id == id
            {
                return Some(line);
            }
            index += 1;
        }

        None
    }

    fn get_dma_region(&self, id: KernelObjectId) -> Option<DmaRegionObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::DmaRegion(region)) = self.objects[index]
                && region.id == id
            {
                return Some(region);
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
        restart_context: ProcessContext,
        state: ProcessState,
        caps: CapabilitySpace,
    ) -> Result<ProcessId, InitError> {
        if self.count == self.processes.len() {
            return Err(InitError::ProcessTableFull);
        }

        let pid = ProcessId::new(self.next_id);
        self.next_id += 1;
        self.processes[self.count] = Some(Process::new(
            pid,
            name,
            context,
            restart_context,
            state,
            caps,
        ));
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

    fn next_ready_index_round_robin(&self, include_current: bool) -> Option<usize> {
        if self.count == 0 {
            return None;
        }

        let current = self.current_index();
        let start = self
            .current_index()
            .map(|index| (index + 1) % self.count)
            .unwrap_or(0);
        let mut offset = 0;

        while offset < self.count {
            let index = (start + offset) % self.count;
            if !include_current && current == Some(index) {
                offset += 1;
                continue;
            }
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
            generation_id: "",
            active_config: None,
            next_cap_id: 1,
            revoked_caps: [0; MAX_REVOKED_CAPS],
            revoked_cap_count: 0,
        }
    }

    fn reset_capability_lifecycle(&mut self, config: &'static BootRuntimeConfig) {
        self.generation_id = config.generation_id;
        self.active_config = Some(config);
        self.next_cap_id = 1;
        self.revoked_cap_count = 0;
        let mut index = 0;
        while index < self.revoked_caps.len() {
            self.revoked_caps[index] = 0;
            index += 1;
        }
    }

    fn generation_cap_count(&self, generation_id: &'static str) -> u64 {
        let mut count = 0;
        let mut process_index = 0;
        while process_index < self.processes.count {
            if let Some(process) = self.processes.processes[process_index] {
                count += generation_cap_count_in_space(process.caps, generation_id);
                count += generation_cap_count_in_space(process.initial_caps, generation_id);
            }
            process_index += 1;
        }
        count
    }

    fn new_capability(
        &mut self,
        object: KernelObjectId,
        rights: u64,
        owner_process: ProcessId,
        parent_cap_id: u64,
        delegated_by: ProcessId,
    ) -> Capability {
        let cap = Capability {
            id: self.next_cap_id,
            object,
            rights,
            owner_process,
            parent_cap_id,
            generation_id: self.generation_id,
            delegated_by,
            revoked: false,
        };
        self.next_cap_id = self.next_cap_id.saturating_add(1);
        cap
    }

    fn cap_id_revoked(&self, cap_id: u64) -> bool {
        let mut index = 0;
        while index < self.revoked_cap_count {
            if self.revoked_caps[index] == cap_id {
                return true;
            }
            index += 1;
        }
        false
    }

    fn revoke_cap_id(&mut self, cap_id: u64) -> Result<(), IpcError> {
        if cap_id == 0 {
            return Err(IpcError::BadCapability);
        }
        if !self.cap_id_revoked(cap_id) {
            if self.revoked_cap_count == self.revoked_caps.len() {
                return Err(IpcError::BadCapability);
            }
            self.revoked_caps[self.revoked_cap_count] = cap_id;
            self.revoked_cap_count += 1;
        }
        self.mark_cap_revoked(cap_id);

        let mut changed = true;
        while changed {
            changed = false;
            let mut index = 0;
            while index < self.processes.count {
                if let Some(process) = self.processes.processes[index].as_mut() {
                    changed |= revoke_descendants_in_space(
                        &mut process.caps,
                        cap_id,
                        &mut self.revoked_caps,
                        &mut self.revoked_cap_count,
                    )?;
                    changed |= revoke_descendants_in_space(
                        &mut process.initial_caps,
                        cap_id,
                        &mut self.revoked_caps,
                        &mut self.revoked_cap_count,
                    )?;
                }
                index += 1;
            }
        }
        Ok(())
    }

    fn mark_cap_revoked(&mut self, cap_id: u64) {
        let mut index = 0;
        while index < self.processes.count {
            if let Some(process) = self.processes.processes[index].as_mut() {
                process.caps.mark_revoked(cap_id);
                process.initial_caps.mark_revoked(cap_id);
            }
            index += 1;
        }
    }
}

impl BootRuntimeConfig {
    pub const fn new() -> Self {
        Self {
            generation_id: "",
            manifest_hash: [0; 64],
            processes: [None; MAX_PROCESSES],
            process_count: 0,
            endpoints: [None; MAX_OBJECTS],
            endpoint_count: 0,
            manifest_module: None,
            store_objects: [None; MAX_OBJECTS],
            store_object_count: 0,
            network_ports: [None; MAX_OBJECTS],
            network_port_count: 0,
            io_ports: [None; MAX_OBJECTS],
            io_port_count: 0,
            mmio_regions: [None; MAX_OBJECTS],
            mmio_region_count: 0,
            interrupt_lines: [None; MAX_OBJECTS],
            interrupt_line_count: 0,
            dma_regions: [None; MAX_OBJECTS],
            dma_region_count: 0,
            grants: [None; MAX_BOOT_GRANTS],
            grant_count: 0,
        }
    }

    pub fn set_generation_id(&mut self, generation_id: &'static str) {
        self.generation_id = generation_id;
    }

    pub fn set_manifest_hash(&mut self, hash: [u8; 64]) {
        self.manifest_hash = hash;
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

    pub fn add_network_port(&mut self, port: BootNetworkPortConfig) -> Result<(), InitError> {
        if self.network_port_count == self.network_ports.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.network_ports[self.network_port_count] = Some(port);
        self.network_port_count += 1;
        Ok(())
    }

    pub fn add_io_port(&mut self, port: BootIoPortRangeConfig) -> Result<(), InitError> {
        if self.io_port_count == self.io_ports.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.io_ports[self.io_port_count] = Some(port);
        self.io_port_count += 1;
        Ok(())
    }

    pub fn add_mmio_region(&mut self, region: BootMmioRegionConfig) -> Result<(), InitError> {
        if self.mmio_region_count == self.mmio_regions.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.mmio_regions[self.mmio_region_count] = Some(region);
        self.mmio_region_count += 1;
        Ok(())
    }

    pub fn add_interrupt_line(&mut self, line: BootInterruptLineConfig) -> Result<(), InitError> {
        if self.interrupt_line_count == self.interrupt_lines.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.interrupt_lines[self.interrupt_line_count] = Some(line);
        self.interrupt_line_count += 1;
        Ok(())
    }

    pub fn add_dma_region(&mut self, region: BootDmaRegionConfig) -> Result<(), InitError> {
        if self.dma_region_count == self.dma_regions.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.dma_regions[self.dma_region_count] = Some(region);
        self.dma_region_count += 1;
        Ok(())
    }

    pub fn add_grant(&mut self, grant: BootGrantConfig) -> Result<(), InitError> {
        if self.grant_count == self.grants.len() {
            return Err(InitError::CapabilityTableFull);
        }
        if grant.process_index >= self.process_count {
            return Err(InitError::InvalidBootManifest);
        }
        let mut index = 0;
        while index < self.grant_count {
            if let Some(existing) = self.grants[index]
                && existing.process_index == grant.process_index
                && existing.cap_slot == grant.cap_slot
            {
                return Err(InitError::InvalidBootManifest);
            }
            index += 1;
        }
        match grant.object_kind {
            BOOT_OBJECT_ENDPOINT if grant.object_index < self.endpoint_count => {}
            BOOT_OBJECT_STORE if grant.object_index < self.store_object_count => {}
            BOOT_OBJECT_TIMER if grant.object_index == 0 => {}
            BOOT_OBJECT_NETWORK_PORT if grant.object_index < self.network_port_count => {}
            BOOT_OBJECT_IO_PORT_RANGE if grant.object_index < self.io_port_count => {}
            BOOT_OBJECT_MMIO_REGION if grant.object_index < self.mmio_region_count => {}
            BOOT_OBJECT_INTERRUPT_LINE if grant.object_index < self.interrupt_line_count => {}
            BOOT_OBJECT_DMA_REGION if grant.object_index < self.dma_region_count => {}
            BOOT_OBJECT_ENDPOINT
            | BOOT_OBJECT_STORE
            | BOOT_OBJECT_STATE
            | BOOT_OBJECT_TIMER
            | BOOT_OBJECT_NETWORK_PORT
            | BOOT_OBJECT_IO_PORT_RANGE
            | BOOT_OBJECT_MMIO_REGION
            | BOOT_OBJECT_INTERRUPT_LINE
            | BOOT_OBJECT_DMA_REGION => return Err(InitError::InvalidBootManifest),
            _ => return Err(InitError::InvalidBootManifest),
        }
        self.grants[self.grant_count] = Some(grant);
        self.grant_count += 1;
        Ok(())
    }
}

impl GenerationRuntimeTable {
    const fn new() -> Self {
        Self {
            entries: [None; MAX_GENERATION_CONFIGS],
            count: 0,
        }
    }

    fn register(&mut self, runtime: GenerationRuntime) -> Result<(), InitError> {
        let mut index = 0;
        while index < self.count {
            if let Some(existing) = self.entries[index]
                && existing.generation_id == runtime.generation_id
            {
                self.entries[index] = Some(runtime);
                return Ok(());
            }
            index += 1;
        }

        if self.count == self.entries.len() {
            return Err(InitError::ObjectTableFull);
        }

        self.entries[self.count] = Some(runtime);
        self.count += 1;
        Ok(())
    }

    fn find(&self, generation_id: &[u8]) -> Option<GenerationRuntime> {
        let mut index = 0;
        while index < self.count {
            if let Some(runtime) = self.entries[index]
                && runtime.generation_id.as_bytes() == generation_id
            {
                return Some(runtime);
            }
            index += 1;
        }
        None
    }
}

impl BootManagerState {
    const fn new() -> Self {
        Self {
            selected_generation: "",
            previous_generation: "",
            known_good_generation: "",
            last_failed_generation: "",
            boot_attempt_counter: 0,
        }
    }

    fn start_boot(&mut self, generation_id: &'static str) {
        if self.selected_generation.is_empty() {
            self.selected_generation = generation_id;
        }
        self.boot_attempt_counter = self.boot_attempt_counter.saturating_add(1);
        serial::write_str("Native boot manager selected_generation=");
        serial::write_str(self.selected_generation);
        serial::write_str("\n");
        serial::write_str("Native boot manager previous_generation=");
        serial::write_str(if self.previous_generation.is_empty() {
            "<none>"
        } else {
            self.previous_generation
        });
        serial::write_str("\n");
        serial::write_str("Native boot manager known_good_generation=");
        serial::write_str(if self.known_good_generation.is_empty() {
            "<none>"
        } else {
            self.known_good_generation
        });
        serial::write_str("\n");
        serial::write_str("Native boot manager boot_attempt_counter=");
        serial::write_u64_dec(self.boot_attempt_counter);
        serial::write_str("\n");
    }

    fn install_selected(&mut self, previous: &'static str, selected: &'static str) {
        self.previous_generation = previous;
        self.selected_generation = selected;
        self.boot_attempt_counter = self.boot_attempt_counter.saturating_add(1);
        serial::write_str("Native update transaction selected_generation updated: ");
        serial::write_str(selected);
        serial::write_str("\n");
    }

    fn mark_known_good(&mut self, generation_id: &'static str) {
        self.known_good_generation = generation_id;
        self.selected_generation = generation_id;
        serial::write_str("Native boot manager known_good_generation=");
        serial::write_str(generation_id);
        serial::write_str("\n");
        serial::write_str("Native boot manager journal: activation-ok generation=");
        serial::write_str(generation_id);
        serial::write_str("\n");
    }

    fn mark_failed_and_fallback(&mut self, failed: &'static str, fallback: &'static str) {
        self.last_failed_generation = failed;
        self.previous_generation = failed;
        self.selected_generation = fallback;
        serial::write_str("Native boot manager last_failed_generation=");
        serial::write_str(failed);
        serial::write_str("\n");
        serial::write_str("Native boot manager fallback selected_generation=");
        serial::write_str(fallback);
        serial::write_str("\n");
        serial::write_str("Native boot manager journal: failed generation=");
        serial::write_str(failed);
        serial::write_str(" fallback=");
        serial::write_str(fallback);
        serial::write_str("\n");
    }
}

fn generation_cap_count_in_space(space: CapabilitySpace, generation_id: &'static str) -> u64 {
    let mut count = 0;
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = space.caps[slot]
            && cap.generation_id == generation_id
            && !cap.revoked
        {
            count += 1;
        }
        slot += 1;
    }
    count
}

fn revoke_descendants_in_space(
    space: &mut CapabilitySpace,
    parent_cap_id: u64,
    revoked_caps: &mut [u64; MAX_REVOKED_CAPS],
    revoked_cap_count: &mut usize,
) -> Result<bool, IpcError> {
    let mut changed = false;
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(mut cap) = space.caps[slot]
            && cap.parent_cap_id == parent_cap_id
            && !cap.revoked
        {
            cap.revoked = true;
            space.caps[slot] = Some(cap);
            if !revoked_contains(revoked_caps, *revoked_cap_count, cap.id) {
                if *revoked_cap_count == revoked_caps.len() {
                    return Err(IpcError::BadCapability);
                }
                revoked_caps[*revoked_cap_count] = cap.id;
                *revoked_cap_count += 1;
            }
            changed = true;
        }
        slot += 1;
    }
    Ok(changed)
}

fn revoked_contains(revoked_caps: &[u64; MAX_REVOKED_CAPS], count: usize, cap_id: u64) -> bool {
    let mut index = 0;
    while index < count {
        if revoked_caps[index] == cap_id {
            return true;
        }
        index += 1;
    }
    false
}

pub fn init_from_boot_config(config: &'static BootRuntimeConfig) -> Result<(), InitError> {
    boot_manager().start_boot(config.generation_id);
    let runtime = runtime();
    runtime.objects.reset();
    runtime.processes.reset();
    runtime.reset_capability_lifecycle(config);

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
            object.hash,
        )?);
        store_index += 1;
    }

    let mut network_port_ids = [None; MAX_OBJECTS];
    let mut network_index = 0;
    while network_index < config.network_port_count {
        let port = config.network_ports[network_index].ok_or(InitError::InvalidBootManifest)?;
        network_port_ids[network_index] = Some(runtime.objects.add_network_port(port.id)?);
        network_index += 1;
    }

    let mut io_port_ids = [None; MAX_OBJECTS];
    let mut io_index = 0;
    while io_index < config.io_port_count {
        let port = config.io_ports[io_index].ok_or(InitError::InvalidBootManifest)?;
        io_port_ids[io_index] = Some(runtime.objects.add_io_port(
            port.id,
            port.base,
            port.length,
        )?);
        io_index += 1;
    }

    let mut mmio_region_ids = [None; MAX_OBJECTS];
    let mut mmio_index = 0;
    while mmio_index < config.mmio_region_count {
        let region = config.mmio_regions[mmio_index].ok_or(InitError::InvalidBootManifest)?;
        mmio_region_ids[mmio_index] = Some(runtime.objects.add_mmio_region(
            region.id,
            region.base,
            region.length,
        )?);
        mmio_index += 1;
    }

    let mut interrupt_line_ids = [None; MAX_OBJECTS];
    let mut irq_index = 0;
    while irq_index < config.interrupt_line_count {
        let line = config.interrupt_lines[irq_index].ok_or(InitError::InvalidBootManifest)?;
        interrupt_line_ids[irq_index] =
            Some(runtime.objects.add_interrupt_line(line.id, line.line)?);
        irq_index += 1;
    }

    let mut dma_region_ids = [None; MAX_OBJECTS];
    let mut dma_index = 0;
    while dma_index < config.dma_region_count {
        let region = config.dma_regions[dma_index].ok_or(InitError::InvalidBootManifest)?;
        dma_region_ids[dma_index] = Some(runtime.objects.add_dma_region(
            region.id,
            region.base,
            region.length,
        )?);
        dma_index += 1;
    }

    let timer_id = runtime.objects.add_timer("monotonic-timer")?;

    let initial_index = initial_process_index(config)?;
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
            process.restart_context,
            state,
            CapabilitySpace::new(),
        )?;

        if process.initial {
            runtime.processes.set_current(pid);
        }
        process_index += 1;
    }

    if !saw_initial || config.endpoint_count == 0 {
        return Err(InitError::InvalidBootManifest);
    }

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
            BOOT_OBJECT_STATE => return Err(InitError::InvalidBootManifest),
            BOOT_OBJECT_TIMER if grant.object_index == 0 => timer_id,
            BOOT_OBJECT_NETWORK_PORT => {
                network_port_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)?
            }
            BOOT_OBJECT_IO_PORT_RANGE => {
                io_port_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)?
            }
            BOOT_OBJECT_MMIO_REGION => {
                mmio_region_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)?
            }
            BOOT_OBJECT_INTERRUPT_LINE => {
                interrupt_line_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)?
            }
            BOOT_OBJECT_DMA_REGION => {
                dma_region_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)?
            }
            _ => return Err(InitError::InvalidBootManifest),
        };
        let owner = process_pid_at(runtime, grant.process_index)?;
        let cap = runtime.new_capability(object, grant.rights, owner, 0, ProcessId::empty());
        grant_process_cap(runtime, grant.process_index, grant.cap_slot, cap)?;
        grant_index += 1;
    }

    if let Some(module) = config.manifest_module {
        let module_id = runtime
            .objects
            .add_boot_module(module.name, module.base, module.length)?;
        let owner = process_pid_at(runtime, initial_index)?;
        let cap = runtime.new_capability(
            module_id,
            capability::RIGHT_READ,
            owner,
            0,
            ProcessId::empty(),
        );
        grant_process_cap(runtime, initial_index, 0, cap)?;
    }

    let process_control_id = runtime.objects.add_process_control("process-control")?;
    let owner = process_pid_at(runtime, initial_index)?;
    let process_control_rights = capability::RIGHT_CONTROL
        | capability::RIGHT_ALLOCATE
        | capability::RIGHT_DELEGATE
        | capability::RIGHT_REVOKE
        | capability::RIGHT_INSPECT;
    let cap = runtime.new_capability(
        process_control_id,
        process_control_rights,
        owner,
        0,
        ProcessId::empty(),
    );
    grant_process_cap(runtime, initial_index, 2, cap)?;

    print_boot_tables(runtime);
    Ok(())
}

pub fn register_generation_config(config: &'static BootRuntimeConfig) -> Result<(), InitError> {
    let table = generation_runtimes();
    table.register(GenerationRuntime {
        generation_id: config.generation_id,
        config,
    })
}

pub fn set_rollback_boot_config(config: &'static BootRuntimeConfig) {
    set_rollback_runtime(GenerationRuntime {
        generation_id: config.generation_id,
        config,
    });
}

pub fn install_frame_allocator(allocator: *mut memory::FrameAllocator) {
    unsafe {
        *FRAME_ALLOCATOR.0.get() = Some(allocator);
    }
}

fn process_pid_at(runtime: &RuntimeState, process_index: usize) -> Result<ProcessId, InitError> {
    if process_index >= runtime.processes.count {
        return Err(InitError::InvalidBootManifest);
    }
    runtime.processes.processes[process_index]
        .map(|process| process.pid)
        .ok_or(InitError::InvalidBootManifest)
}

fn grant_process_cap(
    runtime: &mut RuntimeState,
    process_index: usize,
    slot: u64,
    cap: Capability,
) -> Result<(), InitError> {
    if process_index >= runtime.processes.count {
        return Err(InitError::InvalidBootManifest);
    }
    let Some(process) = runtime.processes.processes[process_index].as_mut() else {
        return Err(InitError::InvalidBootManifest);
    };

    let mut caps = process.caps;
    let mut initial_caps = process.initial_caps;
    caps.grant(slot, cap)?;
    initial_caps.grant(slot, cap)?;
    process.caps = caps;
    process.initial_caps = initial_caps;
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
        let ok = runtime().processes.all_exited_successfully();
        if ok {
            let generation_id = runtime().generation_id;
            boot_manager().mark_known_good(generation_id);
        }
        ScheduleResult::Halt { ok }
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

    if schedule_next_ready_excluding_current(frame) {
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

pub fn preempt_current_process(frame: &mut SyscallFrame) -> ScheduleResult {
    wake_timed_processes(read_tsc());
    let current = {
        let runtime = runtime();
        if runtime
            .processes
            .next_ready_index_round_robin(false)
            .is_none()
        {
            return ScheduleResult::Continue;
        }
        let Some(process) = runtime.processes.current_process_mut() else {
            return ScheduleResult::Halt { ok: false };
        };
        if process.state != ProcessState::Running {
            return ScheduleResult::Continue;
        }

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::Ready;
        process.name
    };

    if schedule_next_ready_no_wait_excluding_current(frame) {
        serial::write_str("Scheduler preempted process without explicit yield: from=");
        serial::write_str(current);
        serial::write_str(" to=");
        serial::write_str(current_process_name());
        serial::write_str("\n");
        ScheduleResult::Switched
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }
        ScheduleResult::Continue
    }
}

pub fn wake_timed_from_interrupt() {
    wake_timed_processes(read_tsc());
}

pub fn fault_current_process(
    reason: &str,
    address: u64,
    error_code: u64,
    frame: &mut SyscallFrame,
) -> ScheduleResult {
    let (name, initial_faulted) = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return ScheduleResult::Halt { ok: false };
        };
        let initial = process.pid.raw() == 1;
        process.state = ProcessState::Exited;
        process.has_saved_frame = false;
        process.exit_status = STATUS_PROCESS_FAULT;
        process.has_exited = true;
        (process.name, initial)
    };

    serial::write_str("User process fault contained: proc=");
    serial::write_str(name);
    serial::write_str(" reason=");
    serial::write_str(reason);
    serial::write_str(" address=");
    serial::write_u64_hex(address);
    serial::write_str(" error=");
    serial::write_u64_hex(error_code);
    serial::write_str("\n");

    if initial_faulted {
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

    let sender = runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
        .unwrap_or_else(ProcessId::empty);

    let endpoint = runtime()
        .objects
        .get_endpoint_mut(endpoint_id)
        .ok_or(IpcError::BadCapability)?;

    endpoint.enqueue(sender, &message, len)?;

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
    receive_with_timeout(cap_slot, destination, max_len, None, frame)
}

pub fn receive_timeout(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
    timeout_ms: u64,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    receive_with_timeout(
        cap_slot,
        destination,
        max_len,
        Some(deadline_after_ms(timeout_ms)),
        frame,
    )
}

fn receive_with_timeout(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
    timeout_tsc: Option<u64>,
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

    let current_pid = runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
        .unwrap_or_else(ProcessId::empty);
    let queued_message = {
        let endpoint = runtime()
            .objects
            .get_endpoint_mut(endpoint_id)
            .ok_or(IpcError::BadCapability)?;
        endpoint.dequeue_for(current_pid)
    };

    let Some(message) = queued_message else {
        if block_current_on_endpoint(endpoint_id, destination as u64, max_len, timeout_tsc, frame) {
            return Ok(());
        }

        return Err(IpcError::Empty);
    };

    let copy_len = min(message.len, max_len);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &message.bytes[..copy_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("IPC receive delivered: endpoint=");
    serial::write_u64_dec(endpoint_id.raw());
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
    usercopy::copy_to_user(UserPtr::new(destination as u64), &bytes[..copy_len])
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
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let _process_control = process_control_from_cap(
        cap_slot,
        capability::RIGHT_CONTROL | capability::RIGHT_REVOKE,
    )?;
    let mut generation_id = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut generation_id, UserPtr::new(generation as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let requested = &generation_id[..len];
    let target = match generation_runtimes().find(requested) {
        Some(target) => target,
        None => {
            serial::write_str("Krust generation switch rejected: requested=");
            serial::write_ascii_bytes(requested);
            serial::write_str("\n");
            serial::write_str("Native update transaction rejected: missing store object\n");
            serial::write_str("Native update transaction selected_generation unchanged: ");
            serial::write_str(runtime().generation_id);
            serial::write_str("\n");
            return Err(IpcError::BadCapability);
        }
    };
    if failed_generation_is(target.generation_id) {
        serial::write_str("Krust generation switch rejected: requested=");
        serial::write_ascii_bytes(requested);
        serial::write_str(" failed=yes\n");
        return Err(IpcError::BadCapability);
    }

    let (previous_generation, previous_config, old_cap_count) = {
        let runtime = runtime();
        (
            runtime.generation_id,
            runtime.active_config,
            runtime.generation_cap_count(runtime.generation_id),
        )
    };

    if previous_generation == target.generation_id {
        serial::write_str("Krust generation switch already active: ");
        serial::write_str(target.generation_id);
        serial::write_str("\n");
        return Ok(());
    }

    if verify_generation_transaction(target).is_err() {
        serial::write_str("Native update transaction selected_generation unchanged: ");
        serial::write_str(previous_generation);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }

    if let Some(previous_config) = previous_config {
        set_rollback_runtime(GenerationRuntime {
            generation_id: previous_generation,
            config: previous_config,
        });
    }

    serial::write_str("Native update transaction journal commit\n");
    boot_manager().install_selected(previous_generation, target.generation_id);

    serial::write_str("Krust generation switch accepted: from=");
    serial::write_str(previous_generation);
    serial::write_str(" to=");
    serial::write_str(target.generation_id);
    serial::write_str("\n");
    serial::write_str("Krust generation switch revoked old generation authority: generation=");
    serial::write_str(previous_generation);
    serial::write_str(" caps=");
    serial::write_u64_dec(old_cap_count);
    serial::write_str("\n");
    serial::write_str("old generation service loses old capability\n");

    init_from_boot_config(target.config).map_err(|_| IpcError::BadCapability)?;
    let context = initial_process_context().ok_or(IpcError::BadCapability)?;
    serial::write_str("Krust generation switch entering generation: ");
    serial::write_str(target.generation_id);
    serial::write_str("\n");
    let _ = frame;
    unsafe {
        gdt::enter_user_mode(context.cr3, context.entry, context.stack_top);
    }
}

fn verify_generation_transaction(target: GenerationRuntime) -> Result<(), IpcError> {
    verify_generation_manifest(target.config)?;
    verify_generation_store_closure(target.config)?;
    Ok(())
}

fn verify_generation_manifest(config: &BootRuntimeConfig) -> Result<(), IpcError> {
    let Some(module) = config.manifest_module else {
        serial::write_str("Native update transaction rejected: missing manifest\n");
        return Err(IpcError::BadCapability);
    };
    let Ok(len) = usize::try_from(module.length) else {
        serial::write_str("Native update transaction rejected: manifest too large\n");
        return Err(IpcError::MessageTooLarge);
    };
    if len == 0 {
        serial::write_str("Native update transaction rejected: empty manifest\n");
        return Err(IpcError::BadCapability);
    }

    let bytes = unsafe { core::slice::from_raw_parts(module.base as *const u8, len) };
    let mut actual = [0u8; 64];
    store_hash_hex(blake3::hash(bytes).as_bytes(), &mut actual);
    if actual != config.manifest_hash {
        serial::write_str("Native update transaction rejected: manifest hash mismatch\n");
        return Err(IpcError::BadCapability);
    }

    serial::write_str("Native update transaction verifies manifest hash: generation=");
    serial::write_str(config.generation_id);
    serial::write_str(" identity=store:blake3:");
    serial::write_ascii_bytes(&config.manifest_hash);
    serial::write_str("\n");
    Ok(())
}

fn verify_generation_store_closure(config: &BootRuntimeConfig) -> Result<(), IpcError> {
    if config.store_object_count == 0 {
        serial::write_str("Native update transaction rejected: missing store closure\n");
        return Err(IpcError::BadCapability);
    }

    let mut index = 0;
    while index < config.store_object_count {
        let Some(object) = config.store_objects[index] else {
            serial::write_str("Native update transaction rejected: store closure gap\n");
            return Err(IpcError::BadCapability);
        };
        let Ok(len) = usize::try_from(object.length) else {
            serial::write_str("Native update transaction rejected: store object too large object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            return Err(IpcError::MessageTooLarge);
        };
        if len == 0 {
            serial::write_str("Native update transaction rejected: missing store object object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            return Err(IpcError::BadCapability);
        }
        let bytes = unsafe { core::slice::from_raw_parts(object.base as *const u8, len) };
        if !store_hash_matches(bytes, object.hash) {
            serial::write_str("Native update transaction rejected: store hash mismatch object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            serial::write_str("vertex-inspect security event: store hash mismatch object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            return Err(IpcError::BadCapability);
        }
        index += 1;
    }

    serial::write_str("Native update transaction verifies store closure: generation=");
    serial::write_str(config.generation_id);
    serial::write_str(" objects=");
    serial::write_u64_dec(config.store_object_count as u64);
    serial::write_str("\n");
    Ok(())
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
    let _process_control = process_control_from_cap(
        cap_slot,
        capability::RIGHT_CONTROL | capability::RIGHT_REVOKE,
    )?;

    let mut requested = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut requested, UserPtr::new(generation as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let rollback = match unsafe { *ROLLBACK_RUNTIME.0.get() } {
        Some(rollback) => rollback,
        None => {
            serial::write_str("Krust rollback rejected: no rollback runtime\n");
            return Err(IpcError::BadCapability);
        }
    };
    if rollback.generation_id.as_bytes() != &requested[..len] {
        serial::write_str("Krust rollback rejected: requested=");
        serial::write_ascii_bytes(&requested[..len]);
        serial::write_str(" available=");
        serial::write_str(rollback.generation_id);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }

    let (previous_generation, previous_config, old_cap_count) = {
        let runtime = runtime();
        (
            runtime.generation_id,
            runtime.active_config,
            runtime.generation_cap_count(runtime.generation_id),
        )
    };
    if let Some(previous_config) = previous_config {
        set_rollback_runtime(GenerationRuntime {
            generation_id: previous_generation,
            config: previous_config,
        });
        set_failed_generation(previous_generation);
    }
    boot_manager().mark_failed_and_fallback(previous_generation, rollback.generation_id);

    serial::write_str("Krust rollback generation accepted: target=");
    serial::write_str(rollback.generation_id);
    serial::write_str("\n");
    serial::write_str("Krust rollback revoked failed generation authority: generation=");
    serial::write_str(previous_generation);
    serial::write_str(" caps=");
    serial::write_u64_dec(old_cap_count);
    serial::write_str("\n");

    init_from_boot_config(rollback.config).map_err(|_| IpcError::BadCapability)?;
    let context = initial_process_context().ok_or(IpcError::BadCapability)?;
    serial::write_str("Krust rollback entering generation: ");
    serial::write_str(rollback.generation_id);
    serial::write_str("\n");
    let _ = frame;
    unsafe {
        gdt::enter_user_mode(context.cr3, context.entry, context.stack_top);
    }
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

        if process.state == ProcessState::Exited {
            process.context = process.restart_context;
            process.caps = process.initial_caps;
            serial::write_str("Krust process restart reload: proc=");
            serial::write_str(process.name);
            serial::write_str("\n");
        }

        process.state = ProcessState::Ready;
        process.has_saved_frame = false;
        process.exit_status = 0;
        process.has_exited = false;
        process.start_count = process.start_count.saturating_add(1);
        process.name
    };

    serial::write_str("Krust process start accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" target=");
    serial::write_str(target);
    serial::write_str("\n");
    Ok(())
}

pub fn process_attempt() -> Result<u64, IpcError> {
    let runtime = runtime();
    runtime
        .processes
        .current_process()
        .map(|process| process.start_count)
        .ok_or(IpcError::BadCapability)
}

pub fn process_status(cap_slot: u64, process_index: u64) -> Result<u64, IpcError> {
    wake_timed_processes(read_tsc());
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
    let owner = runtime
        .processes
        .current_process()
        .map(|process| process.pid)
        .ok_or(IpcError::BadCapability)?;
    let cap = runtime.new_capability(parent.object, rights_mask, owner, parent.id, owner);
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    process
        .caps
        .grant(new_slot, cap)
        .map_err(|_| IpcError::BadCapability)?;

    serial::write_str("Capability derive accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" parent=");
    serial::write_u64_dec(parent_slot);
    serial::write_str(" new=");
    serial::write_u64_dec(new_slot);
    serial::write_str(" rights=");
    print_rights(rights_mask);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap.id);
    serial::write_str(" parent_cap_id=");
    serial::write_u64_dec(parent.id);
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

pub fn cap_revoke(slot: u64) -> Result<(), IpcError> {
    let cap = lookup_capability(slot, 0)?;
    let process_name = current_process_name();
    runtime().revoke_cap_id(cap.id)?;

    serial::write_str("Capability revoke accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" slot=");
    serial::write_u64_dec(slot);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap.id);
    serial::write_str("\n");
    Ok(())
}

pub fn cap_inspect(slot: u64) -> Result<u64, IpcError> {
    let cap = lookup_capability(slot, 0)?;
    serial::write_str("Capability inspect: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" slot=");
    serial::write_u64_dec(slot);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap.id);
    serial::write_str(" object_id=");
    serial::write_u64_dec(cap.object.raw());
    serial::write_str(" parent_cap_id=");
    serial::write_u64_dec(cap.parent_cap_id);
    serial::write_str(" owner_process=");
    serial::write_u64_dec(cap.owner_process.raw());
    serial::write_str(" delegated_by=");
    serial::write_u64_dec(cap.delegated_by.raw());
    serial::write_str(" generation=");
    serial::write_str(cap.generation_id);
    serial::write_str(" revoked=");
    serial::write_str(if cap.revoked { "yes" } else { "no" });
    serial::write_str("\n");
    Ok(cap.parent_cap_id)
}

pub fn cap_copy(source_slot: u64, target_slot: u64, rights_mask: u64) -> Result<(), IpcError> {
    let source = lookup_capability(source_slot, 0)?;
    if rights_mask == 0 || rights_mask & !source.rights != 0 {
        return Err(IpcError::BadCapability);
    }

    let runtime = runtime();
    let owner = runtime
        .processes
        .current_process()
        .map(|process| process.pid)
        .ok_or(IpcError::BadCapability)?;
    let copied = runtime.new_capability(source.object, rights_mask, owner, source.id, owner);
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    process
        .caps
        .grant(target_slot, copied)
        .map_err(|_| IpcError::BadCapability)?;

    serial::write_str("Capability copy accepted: proc=");
    serial::write_str(process.name);
    serial::write_str(" source=");
    serial::write_u64_dec(source_slot);
    serial::write_str(" target=");
    serial::write_u64_dec(target_slot);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(copied.id);
    serial::write_str(" parent_cap_id=");
    serial::write_u64_dec(source.id);
    serial::write_str("\n");
    Ok(())
}

pub fn cap_move(source_slot: u64, target_slot: u64) -> Result<(), IpcError> {
    let process_name = current_process_name();
    let runtime = runtime();
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    let cap = process.caps.clear(source_slot)?;
    process
        .caps
        .grant(target_slot, cap)
        .map_err(|_| IpcError::BadCapability)?;

    serial::write_str("Capability move accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" source=");
    serial::write_u64_dec(source_slot);
    serial::write_str(" target=");
    serial::write_u64_dec(target_slot);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap.id);
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

    let (caller, target, transferred_id, parent_cap_id) = {
        let runtime = runtime();
        let caller = runtime
            .processes
            .current_process()
            .map(|process| process.name)
            .unwrap_or("<none>");
        if target_process_index >= runtime.processes.count {
            return Err(IpcError::BadCapability);
        }
        let delegated_by = runtime
            .processes
            .current_process()
            .map(|process| process.pid)
            .ok_or(IpcError::BadCapability)?;
        let (target_pid, target_name, persist_for_restart) = {
            let Some(target_process) = runtime.processes.processes[target_process_index] else {
                return Err(IpcError::BadCapability);
            };
            (
                target_process.pid,
                target_process.name,
                target_process.state == ProcessState::Declared,
            )
        };
        let transferred =
            runtime.new_capability(cap.object, rights_mask, target_pid, cap.id, delegated_by);
        let transferred_id = transferred.id;
        let Some(target_process) = runtime.processes.processes[target_process_index].as_mut()
        else {
            return Err(IpcError::BadCapability);
        };
        let mut next_caps = target_process.caps;
        let mut next_initial_caps = target_process.initial_caps;
        next_caps
            .grant(target_slot, transferred)
            .map_err(|_| IpcError::BadCapability)?;
        if persist_for_restart {
            next_initial_caps
                .grant(target_slot, transferred)
                .map_err(|_| IpcError::BadCapability)?;
        }
        target_process.caps = next_caps;
        if persist_for_restart {
            target_process.initial_caps = next_initial_caps;
        }
        (caller, target_name, transferred_id, cap.id)
    };

    serial::write_str("Capability transfer accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" target=");
    serial::write_str(target);
    serial::write_str(" slot=");
    serial::write_u64_dec(target_slot);
    serial::write_str(" rights=");
    print_rights(rights_mask);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(transferred_id);
    serial::write_str(" parent_cap_id=");
    serial::write_u64_dec(parent_cap_id);
    serial::write_str("\n");
    Ok(())
}

pub fn endpoint_create(control_slot: u64, cap_slot: u64) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(control_slot, capability::RIGHT_ALLOCATE)?;
    let process_name = current_process_name();
    let runtime = runtime();
    let (owner, quota) = {
        let Some(process) = runtime.processes.current_process() else {
            return Err(IpcError::BadCapability);
        };
        (process.pid, process.quota)
    };
    if quota.used_endpoints >= quota.max_endpoints {
        serial::write_str("Endpoint create rejected: proc=");
        serial::write_str(process_name);
        serial::write_str(" quota=max_endpoints\n");
        return Err(IpcError::BadCapability);
    }

    let endpoint_id = runtime
        .objects
        .add_endpoint("dynamic-endpoint")
        .map_err(|_| {
            serial::write_str("Endpoint create rejected: object arena full\n");
            IpcError::BadCapability
        })?;
    let cap = runtime.new_capability(
        endpoint_id,
        capability::RIGHT_SEND | capability::RIGHT_RECEIVE,
        owner,
        0,
        owner,
    );
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    process
        .caps
        .grant(cap_slot, cap)
        .map_err(|_| IpcError::BadCapability)?;
    process.quota.used_endpoints = process.quota.used_endpoints.saturating_add(1);

    serial::write_str("Endpoint create accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" slot=");
    serial::write_u64_dec(cap_slot);
    serial::write_str(" endpoint_id=");
    serial::write_u64_dec(endpoint_id.raw());
    serial::write_str(" quota=");
    serial::write_u64_dec(process.quota.used_endpoints);
    serial::write_str("/");
    serial::write_u64_dec(process.quota.max_endpoints);
    serial::write_str("\n");
    Ok(())
}

pub fn quota_delegate(
    control_slot: u64,
    target_process_index: u64,
    max_endpoints: u64,
) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(control_slot, capability::RIGHT_DELEGATE)?;
    let Ok(target_process_index) = usize::try_from(target_process_index) else {
        return Err(IpcError::BadCapability);
    };
    let runtime = runtime();
    let (caller_name, caller_quota) = runtime
        .processes
        .current_process()
        .map(|process| (process.name, process.quota))
        .ok_or(IpcError::BadCapability)?;
    if max_endpoints > caller_quota.max_endpoints {
        serial::write_str("Quota delegate rejected: requested exceeds parent quota\n");
        return Err(IpcError::BadCapability);
    }
    if target_process_index >= runtime.processes.count {
        return Err(IpcError::BadCapability);
    }
    let Some(target) = runtime.processes.processes[target_process_index].as_mut() else {
        return Err(IpcError::BadCapability);
    };
    target.quota.max_endpoints = max_endpoints;

    serial::write_str("Quota delegate accepted: proc=");
    serial::write_str(caller_name);
    serial::write_str(" target=");
    serial::write_str(target.name);
    serial::write_str(" max_endpoints=");
    serial::write_u64_dec(max_endpoints);
    serial::write_str("\n");
    Ok(())
}

pub fn object_read(cap_slot: u64, destination: *mut u8, max_len: usize) -> Result<usize, IpcError> {
    let object = store_object_from_cap(cap_slot, capability::RIGHT_READ)?;
    let Ok(object_len) = usize::try_from(object.length) else {
        return Err(IpcError::MessageTooLarge);
    };
    let bytes = unsafe { core::slice::from_raw_parts(object.base as *const u8, object_len) };
    if !store_hash_matches(bytes, object.hash) {
        serial::write_str("Krust native store hash mismatch: object=");
        serial::write_str(object.name);
        serial::write_str("\n");
        serial::write_str("vertex-inspect security event: store hash mismatch object=");
        serial::write_str(object.name);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }
    let copy_len = min(object_len, max_len);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &bytes[..copy_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Krust native store verified object: object=");
    serial::write_str(object.name);
    serial::write_str(" identity=store:blake3:");
    serial::write_str(object.hash);
    serial::write_str("\n");
    serial::write_str("Object read accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" object=");
    serial::write_str(object.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");
    Ok(copy_len)
}

pub fn io_read(cap_slot: u64, port: u64) -> Result<u64, IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_READ)?;
    if !port_span_in_range(range, port, 1) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    let value = unsafe { serial::inb_raw(port as u16) };
    Ok(value as u64)
}

pub fn io_write(cap_slot: u64, port: u64, value: u64) -> Result<(), IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_WRITE)?;
    if !port_span_in_range(range, port, 1) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    unsafe {
        serial::outb_raw(port as u16, value as u8);
    }
    Ok(())
}

pub fn io_read16(cap_slot: u64, port: u64) -> Result<u64, IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_READ)?;
    if !port_span_in_range(range, port, 2) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    let value = unsafe { serial::inw_raw(port as u16) };
    Ok(value as u64)
}

pub fn io_write16(cap_slot: u64, port: u64, value: u64) -> Result<(), IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_WRITE)?;
    if !port_span_in_range(range, port, 2) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    unsafe {
        serial::outw_raw(port as u16, value as u16);
    }
    Ok(())
}

pub fn io_read32(cap_slot: u64, port: u64) -> Result<u64, IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_READ)?;
    if !port_span_in_range(range, port, 4) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    let value = unsafe { serial::inl_raw(port as u16) };
    Ok(value as u64)
}

pub fn io_write32(cap_slot: u64, port: u64, value: u64) -> Result<(), IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_WRITE)?;
    if !port_span_in_range(range, port, 4) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    unsafe {
        serial::outl_raw(port as u16, value as u32);
    }
    Ok(())
}

pub fn irq_wait(cap_slot: u64, _timeout_ms: u64) -> Result<(), IpcError> {
    let line = interrupt_line_from_cap(cap_slot, capability::RIGHT_LISTEN)?;
    serial::write_str("IRQ wait accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" interrupt-line=");
    serial::write_str(line.name);
    serial::write_str(" line=");
    serial::write_u64_dec(line.line);
    serial::write_str("\n");
    Ok(())
}

pub fn mmio_map(cap_slot: u64) -> Result<u64, IpcError> {
    let region = mmio_region_from_cap(cap_slot, capability::RIGHT_MAP)?;
    map_current_process_physical_range(
        align_down(region.base, memory::FRAME_SIZE),
        align_down(region.base, memory::FRAME_SIZE),
        region
            .length
            .checked_add(region.base - align_down(region.base, memory::FRAME_SIZE))
            .ok_or(IpcError::BadCapability)?,
        paging::PageFlags::user_device(),
    )?;
    serial::write_str("MMIO map accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" mmio-region=");
    serial::write_str(region.name);
    serial::write_str(" base=");
    serial::write_u64_hex(region.base);
    serial::write_str(" length=");
    serial::write_u64_hex(region.length);
    serial::write_str("\n");
    Ok(region.base)
}

pub fn dma_map(cap_slot: u64, destination: *mut u8, max_len: usize) -> Result<(), IpcError> {
    if max_len < DMA_MAPPING_INFO_BYTES {
        return Err(IpcError::InvalidUserBuffer);
    }

    let region = dma_region_from_cap(
        cap_slot,
        capability::RIGHT_READ | capability::RIGHT_WRITE | capability::RIGHT_MAP,
    )?;
    let virtual_base = USER_DMA_MAPPING_BASE + (region.id.raw() << 20);
    map_current_process_physical_range(
        virtual_base,
        region.base,
        region.length,
        paging::PageFlags::user(true, false),
    )?;

    let mut info = [0u8; DMA_MAPPING_INFO_BYTES];
    write_u64(&mut info, 0, virtual_base);
    write_u64(&mut info, 8, region.base);
    write_u64(&mut info, 16, region.length);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &info)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("DMA map accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" dma-region=");
    serial::write_str(region.name);
    serial::write_str(" phys=");
    serial::write_u64_hex(region.base);
    serial::write_str(" virt=");
    serial::write_u64_hex(virtual_base);
    serial::write_str(" length=");
    serial::write_u64_hex(region.length);
    serial::write_str("\n");
    Ok(())
}

pub fn runtime_inspect(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
) -> Result<usize, IpcError> {
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_INSPECT)?;
    let mut report = InspectReport::new();
    let caller = current_process_name();

    {
        let runtime = runtime();
        build_inspect_report(runtime, &mut report);
    }

    if report.truncated || report.len > max_len {
        return Err(IpcError::MessageTooLarge);
    }

    usercopy::copy_to_user(UserPtr::new(destination as u64), report.as_slice())
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Runtime inspect accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" bytes=");
    serial::write_u64_dec(report.len as u64);
    serial::write_str("\n");
    Ok(report.len)
}

pub fn sleep_ms(
    cap_slot: u64,
    milliseconds: u64,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let timer = timer_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    serial::write_str("Timer sleep accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" timer=");
    serial::write_str(timer.name);
    serial::write_str(" ms=");
    serial::write_u64_dec(milliseconds);
    serial::write_str("\n");

    if milliseconds == 0 {
        frame.rax = STATUS_OK;
        return Ok(());
    }

    let wake_tsc = deadline_after_ms(milliseconds);
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::BadCapability);
        };

        process.saved_frame = *frame;
        process.saved_frame.rax = STATUS_OK;
        process.has_saved_frame = true;
        process.state = ProcessState::Sleeping { wake_tsc };
        process.name
    };

    serial::write_str("Timer sleep blocked: proc=");
    serial::write_str(current);
    serial::write_str("\n");

    if schedule_next_ready(frame) {
        return Ok(());
    }

    let runtime = runtime();
    if let Some(process) = runtime.processes.current_process_mut() {
        process.state = ProcessState::Running;
    }

    Err(IpcError::Empty)
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

fn deadline_after_ms(milliseconds: u64) -> u64 {
    read_tsc().saturating_add(milliseconds.saturating_mul(tsc_ticks_per_ms()))
}

fn tsc_ticks_per_ms() -> u64 {
    let leaf15 = __cpuid_count(0x15, 0);
    if leaf15.eax != 0 && leaf15.ebx != 0 && leaf15.ecx != 0 {
        let hz = (leaf15.ecx as u64)
            .saturating_mul(leaf15.ebx as u64)
            .saturating_div(leaf15.eax as u64);
        if hz != 0 {
            return hz / 1_000;
        }
    }

    let leaf16 = __cpuid_count(0x16, 0);
    if leaf16.eax != 0 {
        return (leaf16.eax as u64).saturating_mul(1_000);
    }

    FALLBACK_TSC_TICKS_PER_MS
}

fn block_current_on_endpoint(
    endpoint: KernelObjectId,
    destination: u64,
    max_len: usize,
    timeout_tsc: Option<u64>,
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
            timeout_tsc,
        };
        process.name
    };

    serial::write_str("IPC receive blocked: proc=");
    serial::write_str(current);
    serial::write_str(" endpoint=");
    serial::write_u64_dec(endpoint.raw());
    if timeout_tsc.is_some() {
        serial::write_str(" timeout=yes");
    }
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
    wake_timed_processes(read_tsc());

    let Some(waiter_index) = blocked_receiver_index(endpoint) else {
        return;
    };

    let (name, receiver_pid, receiver_cr3, destination, max_len, current_cr3) = {
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
            waiter.pid,
            waiter.context.cr3,
            destination,
            max_len,
            current_cr3,
        )
    };

    let Some(message) = ({
        let runtime = runtime();
        let Some(endpoint_object) = runtime.objects.get_endpoint_mut(endpoint) else {
            return;
        };
        endpoint_object.dequeue_for(receiver_pid)
    }) else {
        return;
    };

    let copy_len = min(message.len, max_len);
    let copy_result = unsafe {
        gdt::switch_address_space(receiver_cr3);
        let result = usercopy::copy_to_user(UserPtr::new(destination), &message.bytes[..copy_len]);
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
            && runtime
                .objects
                .get_endpoint(endpoint)
                .map(|endpoint_object| endpoint_object.has_message_for(process.pid))
                .unwrap_or(false)
        {
            return Some(index);
        }
        index += 1;
    }

    None
}

fn wake_timed_processes(now: u64) -> usize {
    let runtime = runtime();
    let mut woke = 0;
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index].as_mut() {
            match process.state {
                ProcessState::Sleeping { wake_tsc } if deadline_reached(now, wake_tsc) => {
                    process.saved_frame.rax = STATUS_OK;
                    process.state = ProcessState::Ready;
                    woke += 1;
                    serial::write_str("Timer wake: proc=");
                    serial::write_str(process.name);
                    serial::write_str("\n");
                }
                ProcessState::BlockedOnEndpoint {
                    timeout_tsc: Some(timeout_tsc),
                    ..
                } if deadline_reached(now, timeout_tsc) => {
                    process.saved_frame.rax = STATUS_TIMEOUT;
                    process.state = ProcessState::Ready;
                    woke += 1;
                    serial::write_str("IPC receive timeout: proc=");
                    serial::write_str(process.name);
                    serial::write_str("\n");
                }
                _ => {}
            }
        }
        index += 1;
    }
    woke
}

fn next_deadline_tsc() -> Option<u64> {
    let runtime = runtime();
    let mut next = None;
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index] {
            let deadline = match process.state {
                ProcessState::Sleeping { wake_tsc } => Some(wake_tsc),
                ProcessState::BlockedOnEndpoint {
                    timeout_tsc: Some(timeout_tsc),
                    ..
                } => Some(timeout_tsc),
                _ => None,
            };
            if let Some(deadline) = deadline
                && next
                    .map(|current| deadline_before(deadline, current))
                    .unwrap_or(true)
            {
                next = Some(deadline);
            }
        }
        index += 1;
    }
    next
}

fn wait_until_deadline(deadline: u64) {
    while !deadline_reached(read_tsc(), deadline) && wake_timed_processes(read_tsc()) == 0 {
        crate::timer::wait_for_interrupt();
    }
}

fn deadline_reached(now: u64, deadline: u64) -> bool {
    (now as i64).wrapping_sub(deadline as i64) >= 0
}

fn deadline_before(left: u64, right: u64) -> bool {
    (left as i64).wrapping_sub(right as i64) < 0
}

fn schedule_next_ready(frame: &mut SyscallFrame) -> bool {
    schedule_next_ready_inner(frame, true, true)
}

fn schedule_next_ready_excluding_current(frame: &mut SyscallFrame) -> bool {
    schedule_next_ready_inner(frame, false, true)
}

fn schedule_next_ready_no_wait_excluding_current(frame: &mut SyscallFrame) -> bool {
    schedule_next_ready_inner(frame, false, false)
}

fn schedule_next_ready_inner(
    frame: &mut SyscallFrame,
    include_current: bool,
    wait_for_deadline: bool,
) -> bool {
    wake_timed_processes(read_tsc());
    if wait_for_deadline
        && runtime()
            .processes
            .next_ready_index_round_robin(include_current)
            .is_none()
        && let Some(deadline) = next_deadline_tsc()
    {
        wait_until_deadline(deadline);
        wake_timed_processes(read_tsc());
    }

    if !wait_for_deadline
        && runtime()
            .processes
            .next_ready_index_round_robin(include_current)
            .is_none()
    {
        return false;
    }

    let (from, to, next_frame, next_cr3) = {
        let runtime = runtime();
        let from = runtime
            .processes
            .current_process()
            .map(|process| process.name)
            .unwrap_or("<none>");
        let Some(next_index) = runtime
            .processes
            .next_ready_index_round_robin(include_current)
        else {
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

fn timer_from_cap(cap_slot: u64, required_right: u64) -> Result<TimerObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_timer(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn io_port_from_cap(cap_slot: u64, required_right: u64) -> Result<IoPortRangeObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_io_port(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn mmio_region_from_cap(cap_slot: u64, required_right: u64) -> Result<MmioRegionObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_mmio_region(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn interrupt_line_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<InterruptLineObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_interrupt_line(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn dma_region_from_cap(cap_slot: u64, required_right: u64) -> Result<DmaRegionObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_dma_region(cap.object)
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
    let runtime = runtime();
    let process = runtime
        .processes
        .current_process()
        .ok_or(IpcError::BadCapability)?;
    let cap = process
        .caps
        .lookup(cap_slot)
        .ok_or(IpcError::BadCapability)?;

    if cap.revoked
        || runtime.cap_id_revoked(cap.id)
        || capability_has_revoked_ancestor(runtime, cap)
    {
        return Err(IpcError::BadCapability);
    }

    if required_right != 0 && cap.rights & required_right != required_right {
        return Err(IpcError::BadCapability);
    }

    Ok(cap)
}

fn port_in_range(range: IoPortRangeObject, port: u64) -> bool {
    port >= range.base
        && port
            .checked_sub(range.base)
            .map(|offset| offset < range.length)
            .unwrap_or(false)
}

fn port_span_in_range(range: IoPortRangeObject, port: u64, width: u64) -> bool {
    if width == 0 {
        return false;
    }
    let Some(last_port) = port.checked_add(width - 1) else {
        return false;
    };
    if last_port > u16::MAX as u64 {
        return false;
    }
    port_in_range(range, port) && port_in_range(range, last_port)
}

fn capability_has_revoked_ancestor(runtime: &RuntimeState, cap: Capability) -> bool {
    let mut parent = cap.parent_cap_id;
    while parent != 0 {
        if runtime.cap_id_revoked(parent) {
            return true;
        }
        parent = find_cap_parent(runtime, parent).unwrap_or(0);
    }
    false
}

fn find_cap_parent(runtime: &RuntimeState, cap_id: u64) -> Option<u64> {
    let mut process_index = 0;
    while process_index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[process_index] {
            if let Some(parent) = find_cap_parent_in_space(process.caps, cap_id) {
                return Some(parent);
            }
            if let Some(parent) = find_cap_parent_in_space(process.initial_caps, cap_id) {
                return Some(parent);
            }
        }
        process_index += 1;
    }
    None
}

fn find_cap_parent_in_space(space: CapabilitySpace, cap_id: u64) -> Option<u64> {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = space.caps[slot]
            && cap.id == cap_id
        {
            return Some(cap.parent_cap_id);
        }
        slot += 1;
    }
    None
}

fn build_inspect_report(runtime: &RuntimeState, report: &mut InspectReport) {
    report.push_str("native-runtime-report v=1\n");
    report.push_str("generation=");
    report.push_str(runtime.generation_id);
    report.push_byte(b'\n');
    report.push_str("processes=");
    report.push_u64_dec(runtime.processes.count as u64);
    report.push_byte(b'\n');
    report.push_str("objects=");
    report.push_u64_dec(runtime.objects.count as u64);
    report.push_byte(b'\n');

    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index] {
            report.push_str("process[");
            report.push_u64_dec(index as u64);
            report.push_str("] name=");
            report.push_str(process.name);
            report.push_str(" pid=");
            report.push_u64_dec(process.pid.raw());
            report.push_str(" state=");
            report.push_str(process.state.label());
            report.push_str(" generation=");
            report.push_str(runtime.generation_id);
            report.push_byte(b'\n');

            write_capability_space_report(runtime, report, process, "current", process.caps);
            write_capability_space_report(
                runtime,
                report,
                process,
                "initial",
                process.initial_caps,
            );
        }
        index += 1;
    }
}

fn write_capability_space_report(
    runtime: &RuntimeState,
    report: &mut InspectReport,
    process: Process,
    space_name: &str,
    space: CapabilitySpace,
) {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = space.caps[slot] {
            report.push_str("space=");
            report.push_str(space_name);
            report.push_str(" proc=");
            report.push_str(process.name);
            report.push_str(" cap[");
            report.push_u64_dec(slot as u64);
            report.push_str("] ");
            write_capability_object_report(runtime, report, cap.object);
            report.push_str(" rights=");
            write_rights_report(report, cap.rights);
            report.push_str(" cap_id=");
            report.push_u64_dec(cap.id);
            report.push_str(" parent_cap_id=");
            report.push_u64_dec(cap.parent_cap_id);
            report.push_str(" generation=");
            report.push_str(cap.generation_id);
            report.push_str(" owner_pid=");
            report.push_u64_dec(cap.owner_process.raw());
            report.push_str(" owner=");
            report.push_str(process_name_by_pid(runtime, cap.owner_process));
            report.push_str(" delegated_by_pid=");
            report.push_u64_dec(cap.delegated_by.raw());
            report.push_str(" delegated_by=");
            report.push_str(process_name_by_pid(runtime, cap.delegated_by));
            report.push_str(" revoked=");
            report.push_str(if cap.revoked || runtime.cap_id_revoked(cap.id) {
                "yes"
            } else {
                "no"
            });
            report.push_byte(b'\n');
        }
        slot += 1;
    }
}

fn process_name_by_pid(runtime: &RuntimeState, pid: ProcessId) -> &'static str {
    if pid == ProcessId::empty() {
        return "kernel";
    }

    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && process.pid == pid
        {
            return process.name;
        }
        index += 1;
    }

    "<unknown>"
}

fn write_capability_object_report(
    runtime: &RuntimeState,
    report: &mut InspectReport,
    object: KernelObjectId,
) {
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(entry) = runtime.objects.objects[index] {
            match entry {
                KernelObject::IpcEndpoint(endpoint) if endpoint.id == object => {
                    report.push_str("endpoint=");
                    report.push_str(endpoint.name);
                    return;
                }
                KernelObject::BootModule(module) if module.id == object => {
                    report.push_str("boot-module=");
                    report.push_str(module.name);
                    return;
                }
                KernelObject::StoreObject(store) if store.id == object => {
                    report.push_str("store-object=");
                    report.push_str(store.name);
                    return;
                }
                KernelObject::Timer(timer) if timer.id == object => {
                    report.push_str("timer=");
                    report.push_str(timer.name);
                    return;
                }
                KernelObject::NetworkPort(port) if port.id == object => {
                    report.push_str("network-port=");
                    report.push_str(port.name);
                    return;
                }
                KernelObject::IoPortRange(port) if port.id == object => {
                    report.push_str("io-port=");
                    report.push_str(port.name);
                    return;
                }
                KernelObject::MmioRegion(region) if region.id == object => {
                    report.push_str("mmio-region=");
                    report.push_str(region.name);
                    return;
                }
                KernelObject::InterruptLine(line) if line.id == object => {
                    report.push_str("interrupt-line=");
                    report.push_str(line.name);
                    return;
                }
                KernelObject::DmaRegion(region) if region.id == object => {
                    report.push_str("dma-region=");
                    report.push_str(region.name);
                    return;
                }
                KernelObject::ProcessControl(process_control) if process_control.id == object => {
                    report.push_str("process-control=");
                    report.push_str(process_control.name);
                    return;
                }
                _ => {}
            }
        }
        index += 1;
    }

    report.push_str("object=");
    report.push_u64_dec(object.raw());
}

fn write_rights_report(report: &mut InspectReport, rights: u64) {
    let mut wrote = false;
    wrote = write_right_report(report, rights, capability::RIGHT_READ, "read", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_WRITE, "write", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_SEND, "send", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_RECEIVE, "receive", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_CONTROL, "control", wrote);
    wrote = write_right_report(
        report,
        rights,
        capability::RIGHT_SNAPSHOT,
        "snapshot",
        wrote,
    );
    wrote = write_right_report(report, rights, capability::RIGHT_RESTORE, "restore", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_MAP, "map", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_BIND, "bind", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_LISTEN, "listen", wrote);
    wrote = write_right_report(
        report,
        rights,
        capability::RIGHT_ALLOCATE,
        "allocate",
        wrote,
    );
    wrote = write_right_report(
        report,
        rights,
        capability::RIGHT_DELEGATE,
        "delegate",
        wrote,
    );
    wrote = write_right_report(report, rights, capability::RIGHT_REVOKE, "revoke", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_INSPECT, "inspect", wrote);

    if !wrote {
        report.push_str("none");
    }
}

fn write_right_report(
    report: &mut InspectReport,
    rights: u64,
    right: u64,
    label: &str,
    wrote: bool,
) -> bool {
    if rights & right == 0 {
        return wrote;
    }

    if wrote {
        report.push_byte(b'|');
    }
    report.push_str(label);
    true
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
                KernelObject::IoPortRange(port) if port.id == object => {
                    serial::write_str("io-port=");
                    serial::write_str(port.name);
                    return;
                }
                KernelObject::MmioRegion(region) if region.id == object => {
                    serial::write_str("mmio-region=");
                    serial::write_str(region.name);
                    return;
                }
                KernelObject::InterruptLine(line) if line.id == object => {
                    serial::write_str("interrupt-line=");
                    serial::write_str(line.name);
                    return;
                }
                KernelObject::DmaRegion(region) if region.id == object => {
                    serial::write_str("dma-region=");
                    serial::write_str(region.name);
                    serial::write_str(" base=");
                    serial::write_u64_hex(region.base);
                    serial::write_str(" length=");
                    serial::write_u64_hex(region.length);
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
    serial::write_str(" quota_caps=");
    serial::write_u64_dec(process.quota.max_caps);
    serial::write_str(" quota_endpoints=");
    serial::write_u64_dec(process.quota.max_endpoints);
    serial::write_str(" quota_memory_pages=");
    serial::write_u64_dec(process.quota.max_memory_pages);
    serial::write_str(" quota_child_processes=");
    serial::write_u64_dec(process.quota.max_child_processes);
    serial::write_str(" quota_ipc_bytes=");
    serial::write_u64_dec(process.quota.max_ipc_bytes);
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
    wrote = print_right(rights, capability::RIGHT_MAP, "map", wrote);
    wrote = print_right(rights, capability::RIGHT_BIND, "bind", wrote);
    wrote = print_right(rights, capability::RIGHT_LISTEN, "listen", wrote);
    wrote = print_right(rights, capability::RIGHT_ALLOCATE, "allocate", wrote);
    wrote = print_right(rights, capability::RIGHT_DELEGATE, "delegate", wrote);
    wrote = print_right(rights, capability::RIGHT_REVOKE, "revoke", wrote);
    wrote = print_right(rights, capability::RIGHT_INSPECT, "inspect", wrote);

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

fn generation_runtimes() -> &'static mut GenerationRuntimeTable {
    unsafe { &mut *GENERATION_RUNTIMES.0.get() }
}

fn set_rollback_runtime(runtime: GenerationRuntime) {
    unsafe {
        *ROLLBACK_RUNTIME.0.get() = Some(runtime);
    }
}

fn set_failed_generation(generation_id: &'static str) {
    unsafe {
        *FAILED_GENERATION.0.get() = Some(generation_id);
    }
}

fn failed_generation_is(generation_id: &'static str) -> bool {
    unsafe { *FAILED_GENERATION.0.get() == Some(generation_id) }
}

fn boot_manager() -> &'static mut BootManagerState {
    unsafe { &mut *BOOT_MANAGER.0.get() }
}

fn store_hash_matches(bytes: &[u8], expected: &str) -> bool {
    if expected.len() != 64 {
        return false;
    }
    let mut actual = [0u8; 64];
    store_hash_hex(blake3::hash(bytes).as_bytes(), &mut actual);
    actual == expected.as_bytes()
}

fn store_hash_hex(bytes: &[u8; 32], out: &mut [u8; 64]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut index = 0;
    while index < bytes.len() {
        out[index * 2] = HEX[(bytes[index] >> 4) as usize];
        out[index * 2 + 1] = HEX[(bytes[index] & 0xf) as usize];
        index += 1;
    }
}

fn runtime() -> &'static mut RuntimeState {
    unsafe { &mut *RUNTIME.0.get() }
}

fn frame_allocator() -> Result<&'static mut memory::FrameAllocator, IpcError> {
    let allocator = unsafe { *FRAME_ALLOCATOR.0.get() }.ok_or(IpcError::BadCapability)?;
    unsafe { allocator.as_mut().ok_or(IpcError::BadCapability) }
}

fn map_current_process_physical_range(
    virtual_base: u64,
    physical_base: u64,
    length: u64,
    flags: paging::PageFlags,
) -> Result<(), IpcError> {
    if length == 0
        || virtual_base % memory::FRAME_SIZE != 0
        || physical_base % memory::FRAME_SIZE != 0
    {
        return Err(IpcError::BadCapability);
    }

    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    let root_table_physical = runtime()
        .processes
        .current_process()
        .map(|process| process.context.cr3)
        .ok_or(IpcError::BadCapability)?;
    let allocator = frame_allocator()?;

    let mut offset = 0;
    while offset < length {
        let frame = memory::PhysicalFrame::from_start(
            physical_base
                .checked_add(offset)
                .ok_or(IpcError::BadCapability)?,
        )
        .ok_or(IpcError::BadCapability)?;
        let virtual_address = virtual_base
            .checked_add(offset)
            .ok_or(IpcError::BadCapability)?;
        match paging::map_page_in_root(
            hhdm_offset,
            root_table_physical,
            virtual_address,
            frame,
            flags,
            allocator,
        ) {
            Ok(()) | Err(paging::MapError::AlreadyMapped) => {}
            Err(_) => return Err(IpcError::BadCapability),
        }
        offset = offset
            .checked_add(memory::FRAME_SIZE)
            .ok_or(IpcError::BadCapability)?;
    }

    Ok(())
}

fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        buffer[offset + index] = bytes[index];
        index += 1;
    }
}

fn min(left: usize, right: usize) -> usize {
    if left < right { left } else { right }
}
