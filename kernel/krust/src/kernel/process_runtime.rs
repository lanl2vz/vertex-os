use crate::vfs::{
    FileDescriptionId, FileHandle, FileHandleSlot, VfsNodeId, VfsPath, VfsStateOperation,
};

use super::{
    InitError, IpcError, KernelObjectId, MAX_MESSAGE_BYTES, MAX_OBJECTS, ProcessContext, ProcessId,
    SyscallFrame,
};

pub(crate) const MAX_PROCESSES: usize = 16;
pub(crate) const MAX_CAPS: usize = 32;
pub(crate) const MAX_FILE_HANDLES: usize = 16;
pub(crate) const MAX_OPEN_FILE_DESCRIPTIONS: usize = MAX_PROCESSES * MAX_FILE_HANDLES;

const FILE_HANDLE_SLOT_BITS: u64 = 8;
const FILE_HANDLE_SLOT_MASK: u64 = (1 << FILE_HANDLE_SLOT_BITS) - 1;

#[derive(Clone, Copy)]
pub(crate) struct DmaUserMapping {
    pub(crate) region: KernelObjectId,
    pub(crate) virtual_base: u64,
    pub(crate) physical_base: u64,
    pub(crate) length: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessState {
    Empty,
    Declared,
    Ready,
    Running,
    BlockedOnEndpoint {
        endpoint: KernelObjectId,
        cap_id: u64,
        destination: u64,
        max_len: usize,
        timeout_tsc: Option<u64>,
    },
    BlockedOnInterrupt {
        interrupt: KernelObjectId,
        timeout_tsc: Option<u64>,
    },
    BlockedOnVfsRead {
        node: VfsNodeId,
        description: FileDescriptionId,
        destination: u64,
        max_len: usize,
    },
    BlockedOnVfsState {
        reply_endpoint: KernelObjectId,
        node: VfsNodeId,
        description: FileDescriptionId,
        operation: VfsStateOperation,
        transaction_id: u64,
        offset: u64,
        destination: u64,
        max_len: usize,
        write_len: usize,
        update_offset: bool,
    },
    BlockedOnVertexFsSync {
        request_endpoint: KernelObjectId,
        reply_endpoint: KernelObjectId,
        backing: usize,
        inode_id: u32,
        checksum: u32,
        write_count: usize,
        next_write: usize,
        expected_sector: u64,
    },
    BlockedOnNetworkPort {
        port: KernelObjectId,
        destination: u64,
        max_len: usize,
    },
    Sleeping {
        wake_tsc: u64,
    },
    Exited,
}

impl ProcessState {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Declared => "declared",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::BlockedOnEndpoint { .. } => "blocked",
            Self::BlockedOnInterrupt { .. } => "blocked-irq",
            Self::BlockedOnVfsRead { .. } => "blocked-vfs",
            Self::BlockedOnVfsState { .. } => "blocked-vfs-state",
            Self::BlockedOnVertexFsSync { .. } => "blocked-vertexfs-sync",
            Self::BlockedOnNetworkPort { .. } => "blocked-net",
            Self::Sleeping { .. } => "sleeping",
            Self::Exited => "exited",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Capability {
    pub(crate) id: u64,
    pub(crate) object: KernelObjectId,
    pub(crate) rights: u64,
    pub(crate) owner_process: ProcessId,
    pub(crate) parent_cap_id: u64,
    pub(crate) generation_id: &'static str,
    pub(crate) delegated_by: ProcessId,
    pub(crate) revoked: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct CapabilitySpace {
    pub(crate) caps: [Option<Capability>; MAX_CAPS],
}

#[derive(Clone, Copy)]
pub(crate) struct Process {
    pub(crate) pid: ProcessId,
    pub(crate) name: &'static str,
    pub(crate) context: ProcessContext,
    pub(crate) image_base: u64,
    pub(crate) image_length: u64,
    pub(crate) context_reaped: bool,
    pub(crate) state: ProcessState,
    pub(crate) caps: CapabilitySpace,
    pub(crate) initial_caps: CapabilitySpace,
    pub(crate) saved_frame: SyscallFrame,
    pub(crate) has_saved_frame: bool,
    pub(crate) exit_status: u64,
    pub(crate) has_exited: bool,
    pub(crate) start_count: u64,
    pub(crate) quota: ProcessQuota,
    pub(crate) initial_quota: ProcessQuota,
    pub(crate) mount_root: VfsPath,
    pub(crate) dma_mappings: [Option<DmaUserMapping>; MAX_OBJECTS],
    pub(crate) file_handles: [FileHandleSlot; MAX_FILE_HANDLES],
}

#[derive(Clone, Copy)]
pub(crate) struct ProcessQuota {
    pub(crate) max_caps: u64,
    pub(crate) max_endpoints: u64,
    pub(crate) max_memory_pages: u64,
    pub(crate) max_child_processes: u64,
    pub(crate) max_ipc_bytes: u64,
    pub(crate) used_endpoints: u64,
}

pub(crate) struct ProcessTable {
    pub(crate) processes: [Option<Process>; MAX_PROCESSES],
    pub(crate) count: usize,
    pub(crate) current: Option<ProcessId>,
    pub(crate) next_id: u64,
}

impl CapabilitySpace {
    pub(crate) const fn new() -> Self {
        Self {
            caps: [None; MAX_CAPS],
        }
    }

    pub(crate) fn grant(&mut self, slot: u64, cap: Capability) -> Result<(), InitError> {
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

    pub(crate) fn lookup(&self, slot: u64) -> Option<Capability> {
        let slot = usize::try_from(slot).ok()?;
        self.caps.get(slot).copied().flatten()
    }

    pub(crate) fn clear(&mut self, slot: u64) -> Result<Capability, IpcError> {
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

    pub(crate) fn can_grant(&self, slot: u64) -> bool {
        let Ok(slot) = usize::try_from(slot) else {
            return false;
        };
        slot < self.caps.len() && self.caps[slot].is_none()
    }

    pub(crate) fn mark_revoked(&mut self, cap_id: u64) {
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
    pub(crate) const fn initial() -> Self {
        Self {
            max_caps: MAX_CAPS as u64,
            max_endpoints: 1,
            max_memory_pages: 0,
            max_child_processes: MAX_PROCESSES as u64,
            max_ipc_bytes: MAX_MESSAGE_BYTES as u64,
            used_endpoints: 0,
        }
    }

    pub(crate) const fn service() -> Self {
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
    pub(crate) const fn empty() -> Self {
        Self {
            pid: ProcessId::empty(),
            name: "",
            context: ProcessContext {
                cr3: 0,
                entry: 0,
                stack_top: 0,
            },
            image_base: 0,
            image_length: 0,
            context_reaped: true,
            state: ProcessState::Empty,
            caps: CapabilitySpace::new(),
            initial_caps: CapabilitySpace::new(),
            saved_frame: SyscallFrame::empty(),
            has_saved_frame: false,
            exit_status: 0,
            has_exited: false,
            start_count: 0,
            quota: ProcessQuota::service(),
            initial_quota: ProcessQuota::service(),
            mount_root: VfsPath::empty(),
            dma_mappings: [None; MAX_OBJECTS],
            file_handles: [FileHandleSlot::empty(); MAX_FILE_HANDLES],
        }
    }

    pub(crate) fn new(
        pid: ProcessId,
        name: &'static str,
        context: ProcessContext,
        image_base: u64,
        image_length: u64,
        state: ProcessState,
        caps: CapabilitySpace,
        mount_root: VfsPath,
    ) -> Self {
        let initial = state == ProcessState::Running;
        let start_count = if initial { 1 } else { 0 };
        let quota = if initial {
            ProcessQuota::initial()
        } else {
            ProcessQuota::service()
        };
        Self {
            pid,
            name,
            context,
            image_base,
            image_length,
            context_reaped: false,
            state,
            caps,
            initial_caps: caps,
            saved_frame: SyscallFrame::empty(),
            has_saved_frame: false,
            exit_status: 0,
            has_exited: false,
            start_count,
            quota,
            initial_quota: quota,
            mount_root,
            dma_mappings: [None; MAX_OBJECTS],
            file_handles: [FileHandleSlot::empty(); MAX_FILE_HANDLES],
        }
    }

    pub(crate) fn dma_mapping(&self, region: KernelObjectId) -> Option<DmaUserMapping> {
        let mut index = 0;
        while index < self.dma_mappings.len() {
            if let Some(mapping) = self.dma_mappings[index]
                && mapping.region == region
            {
                return Some(mapping);
            }
            index += 1;
        }
        None
    }

    pub(crate) fn add_dma_mapping(&mut self, mapping: DmaUserMapping) -> Result<(), IpcError> {
        if self.dma_mapping(mapping.region).is_some() {
            return Ok(());
        }
        let mut index = 0;
        while index < self.dma_mappings.len() {
            if self.dma_mappings[index].is_none() {
                self.dma_mappings[index] = Some(mapping);
                return Ok(());
            }
            index += 1;
        }
        Err(IpcError::VfsNoSpace)
    }

    pub(crate) fn clear_dma_mappings(&mut self) {
        let mut index = 0;
        while index < self.dma_mappings.len() {
            self.dma_mappings[index] = None;
            index += 1;
        }
    }

    pub(crate) fn take_dma_mapping(&mut self, index: usize) -> Option<DmaUserMapping> {
        if index >= self.dma_mappings.len() {
            return None;
        }
        let mapping = self.dma_mappings[index];
        self.dma_mappings[index] = None;
        mapping
    }

    pub(crate) fn open_file_handle(&mut self, handle: FileHandle) -> Result<u64, IpcError> {
        let mut index = 0;
        while index < self.file_handles.len() {
            if self.file_handles[index].handle.is_none() {
                let mut generation = self.file_handles[index].generation.saturating_add(1);
                if generation == 0 {
                    generation = 1;
                }
                self.file_handles[index] = FileHandleSlot {
                    generation,
                    handle: Some(handle),
                };
                return Ok((generation << FILE_HANDLE_SLOT_BITS) | ((index as u64) + 1));
            }
            index += 1;
        }
        Err(IpcError::VfsNoSpace)
    }

    pub(crate) fn file_handle(&self, raw: u64) -> Result<(usize, FileHandle), IpcError> {
        let (index, generation) = decode_file_handle(raw)?;
        if index >= self.file_handles.len() {
            return Err(IpcError::VfsBadHandle);
        }
        let slot = self.file_handles[index];
        if slot.generation != generation {
            return Err(IpcError::VfsBadHandle);
        }
        let Some(handle) = slot.handle else {
            return Err(IpcError::VfsBadHandle);
        };
        Ok((index, handle))
    }

    pub(crate) fn close_file_handle(&mut self, raw: u64) -> Result<FileHandle, IpcError> {
        let (index, handle) = self.file_handle(raw)?;
        self.file_handles[index].handle = None;
        Ok(handle)
    }

    pub(crate) fn clear_file_handles(&mut self) {
        let mut index = 0;
        while index < self.file_handles.len() {
            self.file_handles[index].handle = None;
            index += 1;
        }
    }
}

fn decode_file_handle(raw: u64) -> Result<(usize, u64), IpcError> {
    let slot = raw & FILE_HANDLE_SLOT_MASK;
    let generation = raw >> FILE_HANDLE_SLOT_BITS;
    if raw == 0 || slot == 0 || generation == 0 {
        return Err(IpcError::VfsBadHandle);
    }
    Ok(((slot - 1) as usize, generation))
}

impl ProcessTable {
    pub(crate) const fn new() -> Self {
        Self {
            processes: [Some(Process::empty()); MAX_PROCESSES],
            count: 0,
            current: None,
            next_id: 1,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.count = 0;
        self.current = None;
        self.next_id = 1;
    }

    pub(crate) fn add_process(
        &mut self,
        name: &'static str,
        context: ProcessContext,
        image_base: u64,
        image_length: u64,
        state: ProcessState,
        caps: CapabilitySpace,
        mount_root: VfsPath,
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
            image_base,
            image_length,
            state,
            caps,
            mount_root,
        ));
        self.count += 1;
        Ok(pid)
    }

    pub(crate) fn remove_last_process(&mut self, pid: ProcessId) -> Result<(), InitError> {
        if self.count == 0 {
            return Err(InitError::InvalidBootManifest);
        }

        let index = self.count - 1;
        let Some(process) = self.processes[index] else {
            return Err(InitError::InvalidBootManifest);
        };
        if process.pid != pid {
            return Err(InitError::InvalidBootManifest);
        }

        self.processes[index] = Some(Process::empty());
        self.count -= 1;
        if self.next_id == pid.raw() + 1 {
            self.next_id = pid.raw();
        }
        Ok(())
    }

    pub(crate) fn remove_process(&mut self, pid: ProcessId) -> Result<(), InitError> {
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

        let Some(mut index) = found else {
            return Err(InitError::InvalidBootManifest);
        };
        if self.current == Some(pid) {
            return Err(InitError::InvalidBootManifest);
        }

        while index + 1 < self.count {
            self.processes[index] = self.processes[index + 1];
            index += 1;
        }
        self.count -= 1;
        self.processes[self.count] = Some(Process::empty());
        Ok(())
    }

    pub(crate) fn set_current(&mut self, pid: ProcessId) {
        self.current = Some(pid);
    }

    pub(crate) fn current_process(&self) -> Option<Process> {
        let pid = self.current?;
        self.process(pid).copied()
    }

    pub(crate) fn current_process_mut(&mut self) -> Option<&mut Process> {
        let pid = self.current?;
        self.process_mut(pid)
    }

    pub(crate) fn process(&self, pid: ProcessId) -> Option<&Process> {
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

    pub(crate) fn process_mut(&mut self, pid: ProcessId) -> Option<&mut Process> {
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

    pub(crate) fn current_index(&self) -> Option<usize> {
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

    pub(crate) fn next_ready_index_round_robin(&self, include_current: bool) -> Option<usize> {
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

    pub(crate) fn all_exited_successfully(&self) -> bool {
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
