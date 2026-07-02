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
pub(crate) enum VfsMountSourceKind {
    Root,
    Store,
    State,
    Device,
    Proc,
    VertexFs,
    ServiceFs,
    Volatile,
    Unknown,
}

#[derive(Clone, Copy)]
pub(crate) struct VfsMountSource {
    pub(crate) kind: VfsMountSourceKind,
    pub(crate) id: u64,
}

impl VfsMountSource {
    pub(crate) fn from_label(source: &'static str, root_node: VfsNodeId) -> Self {
        Self {
            kind: vfs_mount_source_kind(source),
            id: root_node.raw(),
        }
    }
}

pub(crate) fn vfs_mount_source_kind(source: &'static str) -> VfsMountSourceKind {
    if source == "rootfs" {
        VfsMountSourceKind::Root
    } else if source == "storefs" {
        VfsMountSourceKind::Store
    } else if source == "devfs" {
        VfsMountSourceKind::Device
    } else if source == "procfs" {
        VfsMountSourceKind::Proc
    } else if source == "vertexfs" {
        VfsMountSourceKind::VertexFs
    } else if source == "servicefs" {
        VfsMountSourceKind::ServiceFs
    } else if source == "volatilefs" || source == "state:volatile" {
        VfsMountSourceKind::Volatile
    } else if source.starts_with("state:") {
        VfsMountSourceKind::State
    } else {
        VfsMountSourceKind::Unknown
    }
}

pub(crate) fn vfs_mount_source_kind_label(kind: VfsMountSourceKind) -> &'static str {
    match kind {
        VfsMountSourceKind::Root => "root",
        VfsMountSourceKind::Store => "store",
        VfsMountSourceKind::State => "state",
        VfsMountSourceKind::Device => "device",
        VfsMountSourceKind::Proc => "proc",
        VfsMountSourceKind::VertexFs => "vertexfs",
        VfsMountSourceKind::ServiceFs => "servicefs",
        VfsMountSourceKind::Volatile => "volatile",
        VfsMountSourceKind::Unknown => "unknown",
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
    pub(crate) source_info: VfsMountSource,
    pub(crate) flags: u64,
    pub(crate) dynamic: bool,
    pub(crate) owner: ProcessId,
}

impl VfsMountObject {
    pub(crate) fn new(
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
            source_info: VfsMountSource::from_label(source, root_node),
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
