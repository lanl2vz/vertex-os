use super::vfs_paths::{resolve_process_vfs_path, split_vfs_parent_child};
use super::vfs_transactions::wake_blocked_vfs_pipe_read;
use super::*;

pub(super) fn current_file_handle(handle: u64) -> Result<FileHandle, IpcError> {
    let runtime = runtime();
    let process = runtime
        .processes
        .current_process()
        .ok_or(IpcError::VfsPermission)?;
    let (_, file) = process.file_handle(handle)?;
    Ok(file)
}

pub(super) fn current_open_file(handle: u64) -> Result<(OpenFileDescription, VfsNode), IpcError> {
    let file = current_file_handle(handle)?;
    let description = runtime()
        .file_description(file.description)
        .ok_or(IpcError::VfsBadHandle)?;
    let node = runtime()
        .vfs_node(description.node)
        .ok_or(IpcError::VfsBadHandle)?;
    Ok((description, node))
}

pub(super) fn resolve_vfs_root_authority(cap: Capability, path: &[u8]) -> Result<u64, IpcError> {
    let root = runtime()
        .objects
        .get_vfs_root(cap.object)
        .ok_or(IpcError::VfsPermission)?;
    if !vfs_authority_path_covers(root.root_path.as_bytes(), path) {
        return Err(IpcError::VfsPermission);
    }
    let available = cap.rights & vfs_file_right_mask();
    if available & capability::RIGHT_RESOLVE == 0 {
        return Err(IpcError::VfsPermission);
    }
    Ok(available)
}

pub(super) fn vfs_create_memory_file_node(
    cap: Capability,
    path: &[u8],
    required_file_rights: u64,
) -> Result<(VfsNode, u64), IpcError> {
    let canonical_path = resolve_process_vfs_path(path)?;
    let path = canonical_path.as_bytes();
    let (parent_path, child_name) = split_vfs_parent_child(path)?;
    let child_name = VfsName::from_user_component(child_name).map_err(|_| IpcError::VfsBadPath)?;
    let available = resolve_vfs_root_authority(cap, parent_path)?;
    if available & capability::RIGHT_CREATE == 0 || required_file_rights & !available != 0 {
        return Err(IpcError::VfsPermission);
    }

    let parent = runtime()
        .vfs_node_by_path(parent_path)
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(parent.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    if runtime().vfs_node_by_path(path).is_some() {
        return Err(IpcError::VfsExists);
    }

    let runtime = runtime();
    let node_id = if parent.mount_source == "vertexfs" {
        let parent_inode = vertexfs_directory_inode_for_node(parent)?;
        let backing = runtime.add_empty_vertexfs_file(child_name, parent_inode)?;
        match runtime.add_vfs_node_with_name(
            child_name,
            Some(parent.id),
            VfsNodeKind::RegularFile,
            VfsBacking::VertexFsFile(backing),
            parent.mount_source,
        ) {
            Ok(node_id) => node_id,
            Err(_) => {
                let _ = runtime.release_vertexfs_file(backing);
                return Err(IpcError::VfsNoSpace);
            }
        }
    } else {
        let backing = runtime
            .add_vfs_empty_memory_file()
            .map_err(|_| IpcError::VfsNoSpace)?;
        match runtime.add_vfs_node_with_name(
            child_name,
            Some(parent.id),
            VfsNodeKind::RegularFile,
            VfsBacking::MemoryFile(backing),
            parent.mount_source,
        ) {
            Ok(node_id) => node_id,
            Err(_) => {
                let _ = runtime.release_vfs_memory_file(backing);
                return Err(IpcError::VfsNoSpace);
            }
        }
    };
    let node = runtime.vfs_node(node_id).ok_or(IpcError::VfsBadHandle)?;
    Ok((node, available))
}

pub(super) fn vfs_create_directory_node(cap: Capability, path: &[u8]) -> Result<VfsNode, IpcError> {
    let canonical_path = resolve_process_vfs_path(path)?;
    let path = canonical_path.as_bytes();
    let (parent_path, child_name) = split_vfs_parent_child(path)?;
    let child_name = VfsName::from_user_component(child_name).map_err(|_| IpcError::VfsBadPath)?;
    let available = resolve_vfs_root_authority(cap, parent_path)?;
    if available & capability::RIGHT_CREATE == 0 {
        return Err(IpcError::VfsPermission);
    }

    let parent = runtime()
        .vfs_node_by_path(parent_path)
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(parent.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    if runtime().vfs_node_by_path(path).is_some() {
        return Err(IpcError::VfsExists);
    }

    let runtime = runtime();
    let node_id = runtime
        .add_vfs_node_with_name(
            child_name,
            Some(parent.id),
            VfsNodeKind::Directory,
            VfsBacking::None,
            parent.mount_source,
        )
        .map_err(|_| IpcError::VfsNoSpace)?;
    runtime.vfs_node(node_id).ok_or(IpcError::VfsBadHandle)
}

fn vertexfs_directory_inode_for_node(node: VfsNode) -> Result<u32, IpcError> {
    if node.mount_source != "vertexfs" {
        return Err(IpcError::VfsUnsupported);
    }
    if node.name.as_bytes() == b"app" {
        return Ok(VERTEXFS_INODE_APP_DIR);
    }
    Err(IpcError::VfsUnsupported)
}

pub(super) fn release_created_vfs_memory_node(
    runtime: &mut RuntimeState,
    created_node: Option<VfsNode>,
) {
    let Some(node) = created_node else {
        return;
    };
    if runtime.vfs_node_has_open_description(node.id) {
        return;
    }
    match node.backing {
        VfsBacking::MemoryFile(backing) => {
            let _ = runtime.remove_vfs_node(node.id);
            let _ = runtime.release_vfs_memory_file(backing);
        }
        VfsBacking::VertexFsFile(backing) => {
            let _ = runtime.remove_vfs_node(node.id);
            let _ = runtime.release_vertexfs_file(backing);
        }
        _ => {}
    }
}

pub(super) fn resolve_vfs_node_from_cap(
    cap: Capability,
    path: &[u8],
) -> Result<(VfsNode, u64), IpcError> {
    if let Some(store_node) = runtime().vfs_node_for_store_object(cap.object) {
        if !path.is_empty() && path != b"." {
            return Err(IpcError::VfsBadPath);
        }
        return Ok((store_node, cap.rights & vfs_file_right_mask()));
    }
    if let Some(root) = runtime().objects.get_vfs_root(cap.object) {
        let canonical_path = if path.is_empty() {
            resolve_process_vfs_path(b"/")?
        } else {
            resolve_process_vfs_path(path)?
        };
        let path = canonical_path.as_bytes();
        if cap.rights & capability::RIGHT_RESOLVE == 0
            || !vfs_authority_path_covers(root.root_path.as_bytes(), path)
        {
            return Err(IpcError::VfsPermission);
        }
        let node = runtime()
            .vfs_node_by_path(path)
            .ok_or(IpcError::VfsNotFound)?;
        let available = cap.rights & vfs_file_right_mask();
        if available & capability::RIGHT_RESOLVE == 0 {
            return Err(IpcError::VfsPermission);
        }
        return Ok((node, available));
    }
    Err(IpcError::VfsPermission)
}

pub(super) fn vfs_open_rights(flags: u64, node: VfsNode) -> Result<u64, IpcError> {
    match node.kind {
        VfsNodeKind::Directory => {
            if flags & (VFS_OPEN_WRITE | VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND) != 0 {
                return Err(IpcError::VfsNotFile);
            }
            if flags & VFS_OPEN_READ == 0 {
                return Err(IpcError::VfsUnsupported);
            }
            Ok(capability::RIGHT_RESOLVE)
        }
        VfsNodeKind::DeviceNode => {
            if flags & (VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND) != 0 {
                return Err(IpcError::VfsUnsupported);
            }
            if flags & (VFS_OPEN_READ | VFS_OPEN_WRITE) == 0 {
                return Err(IpcError::VfsUnsupported);
            }
            Ok(capability::RIGHT_CONTROL)
        }
        _ if matches!(node.backing, VfsBacking::StateVolumeControl(_)) => {
            if flags != VFS_OPEN_WRITE {
                return Err(IpcError::VfsUnsupported);
            }
            Ok(capability::RIGHT_CONTROL)
        }
        _ => vfs_regular_file_open_rights(flags),
    }
}

pub(super) fn validate_vfs_device_open(
    flags: u64,
    available_rights: u64,
    device_object: KernelObjectId,
) -> Result<(), IpcError> {
    let mut path_rights = 0;
    if flags & VFS_OPEN_READ != 0 {
        path_rights |= capability::RIGHT_READ;
    }
    if flags & VFS_OPEN_WRITE != 0 {
        path_rights |= capability::RIGHT_WRITE;
    }
    if path_rights == 0 {
        return Err(IpcError::VfsUnsupported);
    }
    if path_rights & !available_rights != 0 {
        return Err(IpcError::VfsPermission);
    }
    if !current_process_has_live_cap_for_object(device_object, capability::RIGHT_CONTROL) {
        return Err(IpcError::VfsPermission);
    }
    Ok(())
}

fn current_process_has_live_cap_for_object(object: KernelObjectId, required_rights: u64) -> bool {
    let runtime = runtime();
    let Some(process) = runtime.processes.current_process() else {
        return false;
    };
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = process.caps.caps[slot]
            && cap.object == object
            && cap.rights & required_rights == required_rights
            && !cap.revoked
            && !runtime.cap_id_revoked(cap.id)
            && !capability_has_revoked_ancestor(runtime, cap)
            && cap.generation_id == runtime.generation_id
        {
            return true;
        }
        slot += 1;
    }
    false
}

pub(super) fn vfs_regular_file_open_rights(flags: u64) -> Result<u64, IpcError> {
    let mut rights = 0;
    if flags & VFS_OPEN_READ != 0 {
        rights |= capability::RIGHT_READ;
    }
    if flags & VFS_OPEN_WRITE != 0 {
        rights |= capability::RIGHT_WRITE;
    }
    if rights == 0 {
        return Err(IpcError::VfsUnsupported);
    }
    Ok(rights)
}

pub(super) fn vfs_file_right_mask() -> u64 {
    capability::RIGHT_READ
        | capability::RIGHT_WRITE
        | capability::RIGHT_CONTROL
        | capability::RIGHT_CREATE
        | capability::RIGHT_UNLINK
        | capability::RIGHT_RENAME
        | capability::RIGHT_MOUNT
        | capability::RIGHT_RESOLVE
        | capability::RIGHT_EXECUTE
        | capability::RIGHT_INSPECT_METADATA
}

pub(super) fn vfs_poll_ready(
    description: OpenFileDescription,
    node: VfsNode,
    events: u64,
) -> Result<u64, IpcError> {
    let mut ready = 0;
    if events & VFS_POLL_READABLE != 0 {
        let readable = match node.kind {
            VfsNodeKind::Directory => runtime().vfs_node_has_children(node.id),
            _ if matches!(node.backing, VfsBacking::Pipe) => !runtime().vfs_pipe.is_empty(),
            _ => vfs_node_len(node)? > description.offset,
        };
        if readable {
            ready |= VFS_POLL_READABLE;
        }
    }
    if events & VFS_POLL_WRITABLE != 0 {
        let writable = match node.backing {
            VfsBacking::MemoryFile(_)
            | VfsBacking::VertexFsFile(_)
            | VfsBacking::StateVolumeValue(_)
            | VfsBacking::StateVolumeControl(_) => true,
            VfsBacking::Pipe => runtime().vfs_pipe.is_empty(),
            _ => false,
        };
        if writable {
            ready |= VFS_POLL_WRITABLE;
        }
    }
    if events & VFS_POLL_METADATA != 0 {
        let mut index = min(description.watch_cursor, runtime().vfs_event_count);
        while index < runtime().vfs_event_count {
            if let Some(event) = runtime().vfs_events[index]
                && event.parent == node.id
            {
                ready |= VFS_POLL_METADATA;
                break;
            }
            index += 1;
        }
    }
    Ok(ready)
}

pub(super) fn vfs_read_node(
    node: VfsNode,
    offset: u64,
    destination: *mut u8,
    max_len: usize,
) -> Result<(usize, u64), IpcError> {
    match node.backing {
        VfsBacking::StoreObject(object_id) => {
            let object = runtime()
                .objects
                .get_store_object(object_id)
                .ok_or(IpcError::VfsBadHandle)?;
            let object_len = store_object_len(object)?;
            let start = min(usize::try_from(offset).unwrap_or(usize::MAX), object_len);
            let remaining = object_len - start;
            let copy_len = min(remaining, max_len);
            let bytes = store_object_bytes(object)?;
            usercopy::copy_to_user(
                UserPtr::new(destination as u64),
                &bytes[start..start + copy_len],
            )
            .map_err(|_| IpcError::InvalidUserBuffer)?;
            Ok((
                copy_len,
                offset
                    .checked_add(copy_len as u64)
                    .ok_or(IpcError::VfsUnsupported)?,
            ))
        }
        VfsBacking::MemoryFile(index) => {
            let runtime = runtime();
            if index >= runtime.vfs_mem_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            let file = runtime.vfs_mem_files[index];
            let start = min(usize::try_from(offset).unwrap_or(usize::MAX), file.len);
            let remaining = file.len - start;
            let copy_len = min(remaining, max_len);
            usercopy::copy_to_user(
                UserPtr::new(destination as u64),
                &file.bytes[start..start + copy_len],
            )
            .map_err(|_| IpcError::InvalidUserBuffer)?;
            Ok((
                copy_len,
                offset
                    .checked_add(copy_len as u64)
                    .ok_or(IpcError::VfsUnsupported)?,
            ))
        }
        VfsBacking::VertexFsFile(index) => {
            runtime().read_vertexfs_page_cache(index, offset, destination, max_len)
        }
        VfsBacking::Synthetic(bytes) => {
            let start = min(usize::try_from(offset).unwrap_or(usize::MAX), bytes.len());
            let remaining = bytes.len() - start;
            let copy_len = min(remaining, max_len);
            usercopy::copy_to_user(
                UserPtr::new(destination as u64),
                &bytes[start..start + copy_len],
            )
            .map_err(|_| IpcError::InvalidUserBuffer)?;
            Ok((
                copy_len,
                offset
                    .checked_add(copy_len as u64)
                    .ok_or(IpcError::VfsUnsupported)?,
            ))
        }
        VfsBacking::None
        | VfsBacking::StateVolume(_)
        | VfsBacking::StateVolumeValue(_)
        | VfsBacking::StateVolumeControl(_)
        | VfsBacking::Device(_)
        | VfsBacking::FsServiceReport
        | VfsBacking::Pipe => Err(IpcError::VfsNotFile),
    }
}

pub(super) fn vfs_write_node(
    node: VfsNode,
    offset: u64,
    bytes: &[u8],
) -> Result<(usize, u64), IpcError> {
    match node.backing {
        VfsBacking::MemoryFile(index) => {
            if bytes.len() > MAX_VFS_MEM_FILE_BYTES {
                return Err(IpcError::VfsNoSpace);
            }
            let start = usize::try_from(offset).map_err(|_| IpcError::VfsNoSpace)?;
            let end = start.checked_add(bytes.len()).ok_or(IpcError::VfsNoSpace)?;
            if end > MAX_VFS_MEM_FILE_BYTES {
                return Err(IpcError::VfsNoSpace);
            }
            let runtime = runtime();
            if index >= runtime.vfs_mem_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            {
                let file = &mut runtime.vfs_mem_files[index];
                let mut cursor = 0;
                while cursor < bytes.len() {
                    file.bytes[start + cursor] = bytes[cursor];
                    cursor += 1;
                }
                if end > file.len {
                    file.len = end;
                }
            }
            runtime.touch_vfs_memory_file_nodes(index)?;
            Ok((bytes.len(), end as u64))
        }
        VfsBacking::VertexFsFile(index) => {
            let runtime = runtime();
            let result = runtime.write_vertexfs_page_cache(index, offset, bytes)?;
            runtime.touch_vertexfs_file_nodes(index)?;
            Ok(result)
        }
        VfsBacking::Pipe => {
            if bytes.len() > MAX_VFS_PIPE_BYTES {
                return Err(IpcError::VfsNoSpace);
            }
            if wake_blocked_vfs_pipe_read(bytes) {
                return Ok((bytes.len(), 0));
            }
            runtime().vfs_pipe.enqueue(bytes)?;
            Ok((bytes.len(), 0))
        }
        _ => Err(IpcError::VfsUnsupported),
    }
}

pub(super) fn vfs_truncate_node(node: VfsNode, len: usize) -> Result<(), IpcError> {
    match node.backing {
        VfsBacking::MemoryFile(index) => {
            if len > MAX_VFS_MEM_FILE_BYTES {
                return Err(IpcError::VfsNoSpace);
            }
            let runtime = runtime();
            if index >= runtime.vfs_mem_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            runtime.vfs_mem_files[index].len = len;
            runtime.touch_vfs_memory_file_nodes(index)?;
            Ok(())
        }
        VfsBacking::VertexFsFile(index) => {
            let runtime = runtime();
            runtime.truncate_vertexfs_page_cache(index, len)?;
            runtime.touch_vertexfs_file_nodes(index)?;
            Ok(())
        }
        _ => Err(IpcError::VfsUnsupported),
    }
}

pub(super) fn vfs_node_len(node: VfsNode) -> Result<u64, IpcError> {
    match node.backing {
        VfsBacking::StoreObject(object_id) => runtime()
            .objects
            .get_store_object(object_id)
            .map(|object| object.length)
            .ok_or(IpcError::VfsBadHandle),
        VfsBacking::MemoryFile(index) => {
            let runtime = runtime();
            if index >= runtime.vfs_mem_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            Ok(runtime.vfs_mem_files[index].len as u64)
        }
        VfsBacking::VertexFsFile(index) => {
            let runtime = runtime();
            if index >= runtime.vertexfs_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            Ok(runtime.vertexfs_files[index].len as u64)
        }
        VfsBacking::Synthetic(bytes) => Ok(bytes.len() as u64),
        VfsBacking::FsServiceReport => Ok(VFS_SERVICE_REPORT_BYTES.len() as u64),
        VfsBacking::Pipe => Ok(runtime().vfs_pipe.len as u64),
        VfsBacking::None
        | VfsBacking::StateVolume(_)
        | VfsBacking::StateVolumeValue(_)
        | VfsBacking::StateVolumeControl(_)
        | VfsBacking::Device(_) => Ok(0),
    }
}

fn store_object_len(object: StoreObject) -> Result<usize, IpcError> {
    usize::try_from(object.length).map_err(|_| IpcError::MessageTooLarge)
}

fn store_object_bytes(object: StoreObject) -> Result<&'static [u8], IpcError> {
    let object_len = store_object_len(object)?;
    let bytes = unsafe { core::slice::from_raw_parts(object.base as *const u8, object_len) };
    if !store_hash_matches(bytes, object.hash) {
        if object.name.starts_with("config:") {
            serial::write_str("Krust native config hash mismatch: config=");
        } else {
            serial::write_str("Krust native store hash mismatch: object=");
        }
        serial::write_str(object.name);
        serial::write_str("\n");
        serial::write_str("vertex-inspect security event: store hash mismatch object=");
        serial::write_str(object.name);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }
    if object.name.starts_with("config:") {
        serial::write_str("Krust native config hash verified: config=");
        serial::write_str(object.name);
        serial::write_str("\n");
    }
    Ok(bytes)
}
