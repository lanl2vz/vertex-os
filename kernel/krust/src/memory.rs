use crate::limine;

pub const FRAME_SIZE: u64 = 4096;

const MAX_RANGES: usize = 64;
const FREE_STACK_CAPACITY: usize = 128;

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
}

pub struct AllocatorStats {
    pub range_count: usize,
    pub total_frames: u64,
    pub allocated_frames: u64,
    pub free_frames: u64,
    pub recycled_frames: usize,
}

pub struct FrameAllocator {
    ranges: [FrameRange; MAX_RANGES],
    range_count: usize,
    next_range: usize,
    free_stack: [PhysicalFrame; FREE_STACK_CAPACITY],
    free_stack_len: usize,
    total_frames: u64,
    allocated_frames: u64,
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
        }
    }

    pub fn init_from_limine(&mut self, memory_map: &limine::MemoryMap) -> Result<(), InitError> {
        *self = Self::new();

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
        if self.free_stack_len > 0 {
            self.free_stack_len -= 1;
            self.allocated_frames += 1;
            return Some(self.free_stack[self.free_stack_len]);
        }

        while self.next_range < self.range_count {
            let range = &mut self.ranges[self.next_range];
            if range.next < range.end {
                let frame = PhysicalFrame { start: range.next };
                range.next += FRAME_SIZE;
                self.allocated_frames += 1;
                return Some(frame);
            }

            self.next_range += 1;
        }

        None
    }

    pub fn allocate_contiguous(&mut self, frame_count: u64) -> Option<PhysicalFrame> {
        if frame_count == 0 {
            return None;
        }

        while self.next_range < self.range_count {
            let range = &mut self.ranges[self.next_range];
            let Some(bytes) = frame_count.checked_mul(FRAME_SIZE) else {
                return None;
            };
            let Some(end) = range.next.checked_add(bytes) else {
                return None;
            };

            if end <= range.end {
                let frame = PhysicalFrame { start: range.next };
                range.next = end;
                self.allocated_frames += frame_count;
                return Some(frame);
            }

            self.next_range += 1;
        }

        None
    }

    pub fn free(&mut self, frame: PhysicalFrame) -> Result<(), FreeError> {
        if frame.start % FRAME_SIZE != 0 {
            return Err(FreeError::UnalignedFrame);
        }

        if !self.contains(frame) {
            return Err(FreeError::OutsideUsableRanges);
        }

        if self.free_stack_len == self.free_stack.len() {
            return Err(FreeError::FreeStackFull);
        }

        self.free_stack[self.free_stack_len] = frame;
        self.free_stack_len += 1;
        self.allocated_frames = self.allocated_frames.saturating_sub(1);
        Ok(())
    }

    pub fn stats(&self) -> AllocatorStats {
        AllocatorStats {
            range_count: self.range_count,
            total_frames: self.total_frames,
            allocated_frames: self.allocated_frames,
            free_frames: self.total_frames.saturating_sub(self.allocated_frames),
            recycled_frames: self.free_stack_len,
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
}

fn align_up(value: u64, align: u64) -> u64 {
    value.saturating_add(align - 1) & !(align - 1)
}

fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}
