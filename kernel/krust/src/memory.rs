use crate::limine;

pub const FRAME_SIZE: u64 = 4096;

const MAX_RANGES: usize = 64;
const FREE_STACK_CAPACITY: usize = 8192;
const MAX_FRAME_RECORDS: usize = 16384;

#[derive(Clone, Copy)]
pub struct PhysicalFrame {
    start: u64,
}

impl PhysicalFrame {
    pub const fn from_start(start: u64) -> Option<Self> {
        if start % FRAME_SIZE == 0 {
            Some(Self { start })
        } else {
            None
        }
    }

    pub fn start(&self) -> u64 {
        self.start
    }
}

#[derive(Clone, Copy)]
struct FrameRange {
    start: u64,
    next: u64,
    end: u64,
}

impl FrameRange {
    const fn empty() -> Self {
        Self {
            start: 0,
            next: 0,
            end: 0,
        }
    }

    fn new(start: u64, end: u64) -> Self {
        Self {
            start,
            next: start,
            end,
        }
    }

    fn frame_count(&self) -> u64 {
        (self.end - self.start) / FRAME_SIZE
    }

    fn contains(&self, frame: PhysicalFrame) -> bool {
        let Some(frame_end) = frame.start.checked_add(FRAME_SIZE) else {
            return false;
        };

        frame.start >= self.start && frame_end <= self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitError {
    TooManyRanges,
    NoUsableFrames,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreeError {
    UnalignedFrame,
    OutsideUsableRanges,
    FreeStackFull,
    UnallocatedFrame,
    WrongOwner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOwnerKind {
    Kernel,
    PageTable,
    ProcessMemory,
    Dma,
    Scratch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameOwner {
    kind: FrameOwnerKind,
    id: u64,
}

impl FrameOwner {
    pub const fn kernel(id: u64) -> Self {
        Self {
            kind: FrameOwnerKind::Kernel,
            id,
        }
    }

    pub const fn page_table(root_table: u64) -> Self {
        Self {
            kind: FrameOwnerKind::PageTable,
            id: root_table,
        }
    }

    pub const fn process_memory(root_table: u64) -> Self {
        Self {
            kind: FrameOwnerKind::ProcessMemory,
            id: root_table,
        }
    }

    pub const fn dma(id: u64) -> Self {
        Self {
            kind: FrameOwnerKind::Dma,
            id,
        }
    }

    pub const fn scratch() -> Self {
        Self {
            kind: FrameOwnerKind::Scratch,
            id: 0,
        }
    }

    pub fn kind(self) -> FrameOwnerKind {
        self.kind
    }
}

pub struct AllocatorStats {
    pub range_count: usize,
    pub total_frames: u64,
    pub allocated_frames: u64,
    pub free_frames: u64,
    pub recycled_frames: usize,
    pub reclaimed_frames: u64,
    pub high_water_frames: u64,
    pub failed_allocations: u64,
    pub kernel_frames: u64,
    pub page_table_frames: u64,
    pub process_memory_frames: u64,
    pub dma_frames: u64,
    pub scratch_frames: u64,
    pub ledger_entries: usize,
}

#[derive(Clone, Copy)]
struct FrameRecord {
    frame: PhysicalFrame,
    owner: FrameOwner,
}

pub struct FrameAllocator {
    ranges: [FrameRange; MAX_RANGES],
    range_count: usize,
    next_range: usize,
    free_stack: [PhysicalFrame; FREE_STACK_CAPACITY],
    free_stack_len: usize,
    total_frames: u64,
    allocated_frames: u64,
    reclaimed_frames: u64,
    high_water_frames: u64,
    failed_allocations: u64,
    records: [Option<FrameRecord>; MAX_FRAME_RECORDS],
    record_count: usize,
}

impl FrameAllocator {
    pub const fn new() -> Self {
        Self {
            ranges: [FrameRange::empty(); MAX_RANGES],
            range_count: 0,
            next_range: 0,
            free_stack: [PhysicalFrame { start: 0 }; FREE_STACK_CAPACITY],
            free_stack_len: 0,
            total_frames: 0,
            allocated_frames: 0,
            reclaimed_frames: 0,
            high_water_frames: 0,
            failed_allocations: 0,
            records: [None; MAX_FRAME_RECORDS],
            record_count: 0,
        }
    }

    pub fn init_from_limine(&mut self, memory_map: &limine::MemoryMap) -> Result<(), InitError> {
        self.range_count = 0;
        self.next_range = 0;
        self.free_stack_len = 0;
        self.total_frames = 0;
        self.allocated_frames = 0;
        self.reclaimed_frames = 0;
        self.high_water_frames = 0;
        self.failed_allocations = 0;
        self.record_count = 0;

        let mut index = 0;
        while index < memory_map.entry_count() {
            if let Some(entry) = memory_map.entry(index)
                && entry.entry_type == limine::MEMMAP_USABLE
            {
                let start = align_up(entry.base, FRAME_SIZE);
                let end = align_down(entry.base.saturating_add(entry.length), FRAME_SIZE);

                if start < end {
                    if self.range_count == self.ranges.len() {
                        return Err(InitError::TooManyRanges);
                    }

                    let range = FrameRange::new(start, end);
                    self.total_frames += range.frame_count();
                    self.ranges[self.range_count] = range;
                    self.range_count += 1;
                }
            }

            index += 1;
        }

        if self.total_frames == 0 {
            return Err(InitError::NoUsableFrames);
        }

        Ok(())
    }

    pub fn allocate(&mut self) -> Option<PhysicalFrame> {
        self.allocate_owned(FrameOwner::scratch())
    }

    pub fn allocate_owned(&mut self, owner: FrameOwner) -> Option<PhysicalFrame> {
        if self.record_count == self.records.len() {
            self.failed_allocations = self.failed_allocations.saturating_add(1);
            return None;
        }

        if self.free_stack_len > 0 {
            self.free_stack_len -= 1;
            self.allocated_frames += 1;
            let frame = self.free_stack[self.free_stack_len];
            self.record_allocated(frame, owner);
            self.update_high_water();
            return Some(frame);
        }

        while self.next_range < self.range_count {
            let range = &mut self.ranges[self.next_range];
            if range.next < range.end {
                let frame = PhysicalFrame { start: range.next };
                range.next += FRAME_SIZE;
                self.allocated_frames += 1;
                self.record_allocated(frame, owner);
                self.update_high_water();
                return Some(frame);
            }

            self.next_range += 1;
        }

        self.failed_allocations = self.failed_allocations.saturating_add(1);
        None
    }

    pub fn allocate_contiguous_owned(
        &mut self,
        frame_count: u64,
        owner: FrameOwner,
    ) -> Option<PhysicalFrame> {
        if frame_count == 0 {
            self.failed_allocations = self.failed_allocations.saturating_add(1);
            return None;
        }
        let Ok(frame_count_usize) = usize::try_from(frame_count) else {
            self.failed_allocations = self.failed_allocations.saturating_add(1);
            return None;
        };
        if self.records.len().saturating_sub(self.record_count) < frame_count_usize {
            self.failed_allocations = self.failed_allocations.saturating_add(1);
            return None;
        }

        while self.next_range < self.range_count {
            let range = &mut self.ranges[self.next_range];
            let Some(bytes) = frame_count.checked_mul(FRAME_SIZE) else {
                self.failed_allocations = self.failed_allocations.saturating_add(1);
                return None;
            };
            let Some(end) = range.next.checked_add(bytes) else {
                self.failed_allocations = self.failed_allocations.saturating_add(1);
                return None;
            };

            if end <= range.end {
                let frame = PhysicalFrame { start: range.next };
                range.next = end;
                self.allocated_frames += frame_count;
                let mut index = 0;
                while index < frame_count_usize {
                    self.record_allocated(
                        PhysicalFrame {
                            start: frame.start + index as u64 * FRAME_SIZE,
                        },
                        owner,
                    );
                    index += 1;
                }
                self.update_high_water();
                return Some(frame);
            }

            self.next_range += 1;
        }

        self.failed_allocations = self.failed_allocations.saturating_add(1);
        None
    }

    pub fn free(&mut self, frame: PhysicalFrame) -> Result<(), FreeError> {
        self.free_with_owner(frame, None)
    }

    pub fn free_owned(
        &mut self,
        frame: PhysicalFrame,
        expected_owner: FrameOwner,
    ) -> Result<(), FreeError> {
        self.free_with_owner(frame, Some(expected_owner))
    }

    pub fn set_owner(&mut self, frame: PhysicalFrame, owner: FrameOwner) -> Result<(), FreeError> {
        if frame.start % FRAME_SIZE != 0 {
            return Err(FreeError::UnalignedFrame);
        }

        if !self.contains(frame) {
            return Err(FreeError::OutsideUsableRanges);
        }

        let Some(record_index) = self.record_index(frame) else {
            return Err(FreeError::UnallocatedFrame);
        };
        if let Some(record) = &mut self.records[record_index] {
            record.owner = owner;
        }
        Ok(())
    }

    pub fn owner_of(&self, frame: PhysicalFrame) -> Option<FrameOwner> {
        self.record_index(frame)
            .and_then(|index| self.records[index].map(|record| record.owner))
    }

    fn free_with_owner(
        &mut self,
        frame: PhysicalFrame,
        expected_owner: Option<FrameOwner>,
    ) -> Result<(), FreeError> {
        if frame.start % FRAME_SIZE != 0 {
            return Err(FreeError::UnalignedFrame);
        }

        if !self.contains(frame) {
            return Err(FreeError::OutsideUsableRanges);
        }

        let Some(record_index) = self.record_index(frame) else {
            return Err(FreeError::UnallocatedFrame);
        };
        if let Some(expected_owner) = expected_owner
            && self.records[record_index]
                .map(|record| record.owner != expected_owner)
                .unwrap_or(true)
        {
            return Err(FreeError::WrongOwner);
        }

        if self.free_stack_len == self.free_stack.len() {
            return Err(FreeError::FreeStackFull);
        }

        self.free_stack[self.free_stack_len] = frame;
        self.free_stack_len += 1;
        self.remove_record(record_index);
        self.allocated_frames = self.allocated_frames.saturating_sub(1);
        self.reclaimed_frames = self.reclaimed_frames.saturating_add(1);
        Ok(())
    }

    pub fn stats(&self) -> AllocatorStats {
        let mut kernel_frames = 0;
        let mut page_table_frames = 0;
        let mut process_memory_frames = 0;
        let mut dma_frames = 0;
        let mut scratch_frames = 0;
        let mut index = 0;
        while index < self.record_count {
            if let Some(record) = self.records[index] {
                match record.owner.kind() {
                    FrameOwnerKind::Kernel => kernel_frames += 1,
                    FrameOwnerKind::PageTable => page_table_frames += 1,
                    FrameOwnerKind::ProcessMemory => process_memory_frames += 1,
                    FrameOwnerKind::Dma => dma_frames += 1,
                    FrameOwnerKind::Scratch => scratch_frames += 1,
                }
            }
            index += 1;
        }

        AllocatorStats {
            range_count: self.range_count,
            total_frames: self.total_frames,
            allocated_frames: self.allocated_frames,
            free_frames: self.total_frames.saturating_sub(self.allocated_frames),
            recycled_frames: self.free_stack_len,
            reclaimed_frames: self.reclaimed_frames,
            high_water_frames: self.high_water_frames,
            failed_allocations: self.failed_allocations,
            kernel_frames,
            page_table_frames,
            process_memory_frames,
            dma_frames,
            scratch_frames,
            ledger_entries: self.record_count,
        }
    }

    fn contains(&self, frame: PhysicalFrame) -> bool {
        let mut index = 0;
        while index < self.range_count {
            if self.ranges[index].contains(frame) {
                return true;
            }

            index += 1;
        }

        false
    }

    fn record_allocated(&mut self, frame: PhysicalFrame, owner: FrameOwner) {
        self.records[self.record_count] = Some(FrameRecord { frame, owner });
        self.record_count += 1;
    }

    fn record_index(&self, frame: PhysicalFrame) -> Option<usize> {
        let mut index = 0;
        while index < self.record_count {
            if let Some(record) = self.records[index]
                && record.frame.start() == frame.start()
            {
                return Some(index);
            }
            index += 1;
        }

        None
    }

    fn remove_record(&mut self, index: usize) {
        self.record_count -= 1;
        self.records[index] = self.records[self.record_count];
        self.records[self.record_count] = None;
    }

    fn update_high_water(&mut self) {
        if self.allocated_frames > self.high_water_frames {
            self.high_water_frames = self.allocated_frames;
        }
    }
}

fn align_up(value: u64, align: u64) -> u64 {
    value.saturating_add(align - 1) & !(align - 1)
}

fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}
