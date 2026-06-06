use crate::kernel::ProcessId;

#[derive(Clone, Copy)]
pub(crate) struct VirtioQueueState {
    pub(crate) dma_physical: u64,
    pub(crate) dma_virtual: u64,
    pub(crate) queue_size: u16,
    pub(crate) avail_offset: usize,
    pub(crate) used_offset: usize,
    pub(crate) data_offset: usize,
    pub(crate) avail_idx: u16,
    pub(crate) used_idx: u16,
    pub(crate) submissions: u64,
    pub(crate) completions: u64,
    pub(crate) interrupt_waits: u64,
    pub(crate) timeouts: u64,
    pub(crate) last_error: &'static str,
}

impl VirtioQueueState {
    pub(crate) const fn empty() -> Self {
        Self {
            dma_physical: 0,
            dma_virtual: 0,
            queue_size: 0,
            avail_offset: 0,
            used_offset: 0,
            data_offset: 0,
            avail_idx: 0,
            used_idx: 0,
            submissions: 0,
            completions: 0,
            interrupt_waits: 0,
            timeouts: 0,
            last_error: "none",
        }
    }

    pub(crate) const fn new(dma_physical: u64, dma_virtual: u64) -> Self {
        Self {
            dma_physical,
            dma_virtual,
            queue_size: 0,
            avail_offset: 0,
            used_offset: 0,
            data_offset: 0,
            avail_idx: 0,
            used_idx: 0,
            submissions: 0,
            completions: 0,
            interrupt_waits: 0,
            timeouts: 0,
            last_error: "none",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VirtioRngState {
    pub(crate) initialized: bool,
    pub(crate) io_base: u16,
    pub(crate) queue: VirtioQueueState,
    pub(crate) owner: ProcessId,
    pub(crate) reset_count: u64,
    pub(crate) last_error: &'static str,
}

impl VirtioRngState {
    pub(crate) const fn new() -> Self {
        Self {
            initialized: false,
            io_base: 0,
            queue: VirtioQueueState::empty(),
            owner: ProcessId::empty(),
            reset_count: 0,
            last_error: "none",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VirtioNetState {
    pub(crate) initialized: bool,
    pub(crate) io_base: u16,
    pub(crate) rx: VirtioQueueState,
    pub(crate) tx: VirtioQueueState,
    pub(crate) rx_posted: bool,
    pub(crate) owner: ProcessId,
    pub(crate) reset_count: u64,
    pub(crate) last_error: &'static str,
}

impl VirtioNetState {
    pub(crate) const fn new() -> Self {
        Self {
            initialized: false,
            io_base: 0,
            rx: VirtioQueueState::empty(),
            tx: VirtioQueueState::empty(),
            rx_posted: false,
            owner: ProcessId::empty(),
            reset_count: 0,
            last_error: "none",
        }
    }
}
