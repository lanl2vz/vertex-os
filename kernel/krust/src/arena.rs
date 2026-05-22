#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArenaError {
    Full,
    InvalidHandle,
    SlotEmpty,
}

#[derive(Clone, Copy)]
pub struct ArenaHandle {
    index: usize,
    generation: u64,
}

impl ArenaHandle {
    pub fn index(self) -> usize {
        self.index
    }
}

#[derive(Clone, Copy)]
struct ArenaSlot<T: Copy> {
    value: Option<T>,
    generation: u64,
}

impl<T: Copy> ArenaSlot<T> {
    const fn empty() -> Self {
        Self {
            value: None,
            generation: 1,
        }
    }
}

pub struct TypedArena<T: Copy, const N: usize> {
    slots: [ArenaSlot<T>; N],
    live: usize,
}

impl<T: Copy, const N: usize> TypedArena<T, N> {
    pub const fn new() -> Self {
        Self {
            slots: [ArenaSlot::empty(); N],
            live: 0,
        }
    }

    pub fn alloc(&mut self, value: T) -> Result<ArenaHandle, ArenaError> {
        let mut index = 0;
        while index < N {
            if self.slots[index].value.is_none() {
                self.slots[index].value = Some(value);
                self.live += 1;
                return Ok(ArenaHandle {
                    index,
                    generation: self.slots[index].generation,
                });
            }
            index += 1;
        }
        Err(ArenaError::Full)
    }

    pub fn free(&mut self, handle: ArenaHandle) -> Result<(), ArenaError> {
        if handle.index >= N || self.slots[handle.index].generation != handle.generation {
            return Err(ArenaError::InvalidHandle);
        }
        if self.slots[handle.index].value.is_none() {
            return Err(ArenaError::SlotEmpty);
        }
        self.slots[handle.index].value = None;
        self.slots[handle.index].generation = self.slots[handle.index].generation.saturating_add(1);
        self.live = self.live.saturating_sub(1);
        Ok(())
    }

    pub fn live(&self) -> usize {
        self.live
    }
}

#[derive(Clone, Copy)]
pub struct KernelHeap {
    base: u64,
    length: u64,
    next: u64,
}

impl KernelHeap {
    pub const fn new(base: u64, length: u64) -> Self {
        Self {
            base,
            length,
            next: base,
        }
    }

    pub fn alloc(&mut self, bytes: u64, align: u64) -> Option<u64> {
        let align = align.max(1);
        let start = align_up(self.next, align);
        let end = start.checked_add(bytes)?;
        if end > self.base.checked_add(self.length)? {
            return None;
        }
        self.next = end;
        Some(start)
    }
}

fn align_up(value: u64, align: u64) -> u64 {
    let remainder = value % align;
    if remainder == 0 {
        value
    } else {
        value.saturating_add(align - remainder)
    }
}
