use crate::kernel::{KernelObjectId, ProcessId};

use super::{VfsNodeId, VfsPath};

pub(crate) const MAX_NAMESPACE_ENTRIES: usize = 4;

#[derive(Clone, Copy)]
pub(crate) struct NamespaceEntry {
    pub(crate) path: &'static str,
    pub(crate) object: KernelObjectId,
    pub(crate) rights: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct NamespaceObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    entries: [Option<NamespaceEntry>; MAX_NAMESPACE_ENTRIES],
    entry_count: usize,
}

impl NamespaceObject {
    pub(crate) const fn new(
        id: KernelObjectId,
        name: &'static str,
        entries: [Option<NamespaceEntry>; MAX_NAMESPACE_ENTRIES],
        entry_count: usize,
    ) -> Self {
        Self {
            id,
            name,
            entries,
            entry_count,
        }
    }

    pub(crate) fn resolve(&self, path: &[u8]) -> Option<NamespaceEntry> {
        let mut index = 0;
        while index < self.entry_count {
            if let Some(entry) = self.entries[index]
                && entry.path.as_bytes() == path
            {
                return Some(entry);
            }
            index += 1;
        }
        None
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VfsRootObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) root_path: VfsPath,
    pub(crate) derived: bool,
}

impl VfsRootObject {
    pub(crate) const fn new(
        id: KernelObjectId,
        name: &'static str,
        root_path: VfsPath,
        derived: bool,
    ) -> Self {
        Self {
            id,
            name,
            root_path,
            derived,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VfsMountObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) root_node: VfsNodeId,
    pub(crate) root_path: VfsPath,
    pub(crate) source: &'static str,
    pub(crate) flags: u64,
    pub(crate) dynamic: bool,
    pub(crate) owner: ProcessId,
}

impl VfsMountObject {
    pub(crate) const fn new(
        id: KernelObjectId,
        name: &'static str,
        root_node: VfsNodeId,
        root_path: VfsPath,
        source: &'static str,
        flags: u64,
        dynamic: bool,
        owner: ProcessId,
    ) -> Self {
        Self {
            id,
            name,
            root_node,
            root_path,
            source,
            flags,
            dynamic,
            owner,
        }
    }
}

pub(crate) fn vfs_authority_path_covers(authority: &[u8], path: &[u8]) -> bool {
    if authority == b"/" {
        return !path.is_empty() && path[0] == b'/';
    }
    if authority == path {
        return true;
    }
    path.len() > authority.len() && path.starts_with(authority) && path[authority.len()] == b'/'
}
