use crate::kernel::{KernelObjectId, ProcessId};

#[derive(Clone, Copy)]
pub(crate) struct TimerObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
}

impl TimerObject {
    pub(crate) const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self { id, name }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct IoPortRangeObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) base: u64,
    pub(crate) length: u64,
}

impl IoPortRangeObject {
    pub(crate) const fn new(
        id: KernelObjectId,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Self {
        Self {
            id,
            name,
            base,
            length,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MmioRegionObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) base: u64,
    pub(crate) length: u64,
}

impl MmioRegionObject {
    pub(crate) const fn new(
        id: KernelObjectId,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Self {
        Self {
            id,
            name,
            base,
            length,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FramebufferObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) physical_base: u64,
    pub(crate) length: u64,
    pub(crate) width: u64,
    pub(crate) height: u64,
    pub(crate) pitch: u64,
    pub(crate) bpp: u16,
    pub(crate) red_mask_size: u8,
    pub(crate) red_mask_shift: u8,
    pub(crate) green_mask_size: u8,
    pub(crate) green_mask_shift: u8,
    pub(crate) blue_mask_size: u8,
    pub(crate) blue_mask_shift: u8,
}

impl FramebufferObject {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        id: KernelObjectId,
        name: &'static str,
        physical_base: u64,
        length: u64,
        width: u64,
        height: u64,
        pitch: u64,
        bpp: u16,
        red_mask_size: u8,
        red_mask_shift: u8,
        green_mask_size: u8,
        green_mask_shift: u8,
        blue_mask_size: u8,
        blue_mask_shift: u8,
    ) -> Self {
        Self {
            id,
            name,
            physical_base,
            length,
            width,
            height,
            pitch,
            bpp,
            red_mask_size,
            red_mask_shift,
            green_mask_size,
            green_mask_shift,
            blue_mask_size,
            blue_mask_shift,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct InterruptLineObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) line: u64,
    pub(crate) pending_count: u64,
    pub(crate) delivered_count: u64,
    pub(crate) spurious_count: u64,
}

impl InterruptLineObject {
    pub(crate) const fn new(id: KernelObjectId, name: &'static str, line: u64) -> Self {
        Self {
            id,
            name,
            line,
            pending_count: 0,
            delivered_count: 0,
            spurious_count: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct DmaRegionObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) base: u64,
    pub(crate) length: u64,
    pub(crate) mapped_by: ProcessId,
    pub(crate) map_count: u64,
    pub(crate) release_count: u64,
}

impl DmaRegionObject {
    pub(crate) const fn new(
        id: KernelObjectId,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Self {
        Self {
            id,
            name,
            base,
            length,
            mapped_by: ProcessId::empty(),
            map_count: 0,
            release_count: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PciDeviceObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) kind: &'static str,
}

impl PciDeviceObject {
    pub(crate) const fn new(id: KernelObjectId, name: &'static str, kind: &'static str) -> Self {
        Self { id, name, kind }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VirtioDeviceObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) transport: &'static str,
    pub(crate) owner: ProcessId,
    pub(crate) queue_size: u16,
    pub(crate) avail_idx: u16,
    pub(crate) used_idx: u16,
    pub(crate) submissions: u64,
    pub(crate) completions: u64,
    pub(crate) timeouts: u64,
    pub(crate) reset_count: u64,
    pub(crate) last_error: &'static str,
}

impl VirtioDeviceObject {
    pub(crate) const fn new(
        id: KernelObjectId,
        name: &'static str,
        transport: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            transport,
            owner: ProcessId::empty(),
            queue_size: 0,
            avail_idx: 0,
            used_idx: 0,
            submissions: 0,
            completions: 0,
            timeouts: 0,
            reset_count: 0,
            last_error: "none",
        }
    }
}
