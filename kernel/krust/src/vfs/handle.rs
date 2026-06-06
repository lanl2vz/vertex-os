use crate::kernel::ProcessId;

use super::{VfsName, VfsNodeId};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileDescriptionId(u64);

impl FileDescriptionId {
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub(crate) const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OpenFileDescription {
    pub(crate) id: FileDescriptionId,
    pub(crate) node: VfsNodeId,
    pub(crate) rights: u64,
    pub(crate) flags: u64,
    pub(crate) offset: u64,
    pub(crate) ref_count: u64,
    pub(crate) owner: ProcessId,
    pub(crate) authority_cap_id: u64,
    pub(crate) watch_cursor: usize,
}

impl OpenFileDescription {
    pub(crate) const fn new(
        id: FileDescriptionId,
        node: VfsNodeId,
        rights: u64,
        flags: u64,
        owner: ProcessId,
        authority_cap_id: u64,
        watch_cursor: usize,
    ) -> Self {
        Self {
            id,
            node,
            rights,
            flags,
            offset: 0,
            ref_count: 1,
            owner,
            authority_cap_id,
            watch_cursor,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VfsLockMode {
    Shared,
    Exclusive,
}

#[derive(Clone, Copy)]
pub(crate) struct VfsLock {
    pub(crate) node: VfsNodeId,
    pub(crate) owner: ProcessId,
    pub(crate) description: FileDescriptionId,
    pub(crate) mode: VfsLockMode,
    pub(crate) start: u64,
    pub(crate) len: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct VfsEvent {
    pub(crate) parent: VfsNodeId,
    pub(crate) kind: u64,
    pub(crate) name: VfsName,
    pub(crate) metadata_version: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct FileHandle {
    pub(crate) description: FileDescriptionId,
}

#[derive(Clone, Copy)]
pub(crate) struct FileHandleSlot {
    pub(crate) generation: u64,
    pub(crate) handle: Option<FileHandle>,
}

impl FileHandleSlot {
    pub(crate) const fn empty() -> Self {
        Self {
            generation: 0,
            handle: None,
        }
    }
}
