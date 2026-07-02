use super::vfs_nodes::{
    current_file_handle, current_open_file, release_created_vfs_memory_node,
    resolve_vfs_node_from_cap, resolve_vfs_root_authority, validate_vfs_device_open,
    vfs_create_directory_node, vfs_create_memory_file_node, vfs_file_right_mask, vfs_node_len,
    vfs_open_rights, vfs_poll_ready, vfs_read_node, vfs_regular_file_open_rights,
    vfs_truncate_node, vfs_write_node,
};
use super::vfs_paths::{
    resolve_process_vfs_path, split_vfs_parent_child, vfs_path_is_read_only,
    vfs_request_path_is_read_only,
};
use super::vfs_transactions::{
    block_current_on_vfs_read, start_vertexfs_sync_transaction, start_vfs_service_read_transaction,
    start_vfs_state_transaction,
};
use super::vfs_wire::{
    read_u64_le, serial_write_vfs_mount_flags, serial_write_vfs_name, write_vfs_dirent_record,
    write_vfs_stat_record, write_vfs_watch_event_record,
};
use super::*;

pub fn legacy_object_read(
    _cap_slot: u64,
    _destination: *mut u8,
    _max_len: usize,
) -> Result<usize, IpcError> {
    serial::write_str("Legacy object-read syscall rejected: use VFS handles\n");
    Err(IpcError::BadCapability)
}

pub fn vfs_open(cap_slot: u64, path: *const u8, packed_len_flags: u64) -> Result<u64, IpcError> {
    let path_len = usize::try_from(packed_len_flags & 0xffff_ffff).unwrap_or(usize::MAX);
    let flags = packed_len_flags >> 32;
    if path_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    if flags & !VFS_OPEN_KNOWN_FLAGS != 0 {
        return Err(IpcError::VfsUnsupported);
    }
    if flags & (VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND) != 0
        && flags & VFS_OPEN_WRITE == 0
    {
        return Err(IpcError::VfsPermission);
    }

    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let cap = lookup_capability(cap_slot, 0).map_err(|_| IpcError::VfsPermission)?;
    let path = &path_bytes[..path_len];
    if flags & (VFS_OPEN_WRITE | VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND) != 0
        && runtime().objects.get_vfs_root(cap.object).is_some()
    {
        let canonical_path = if path.is_empty() {
            resolve_process_vfs_path(b"/")?
        } else {
            resolve_process_vfs_path(path)?
        };
        let canonical = canonical_path.as_bytes();
        if vfs_path_is_read_only(canonical) {
            if let Some(node) = runtime().vfs_node_by_path(canonical)
                && matches!(node.backing, VfsBacking::FsServiceReport)
            {
                serial::write_str(
                    "VFS filesystem service write-open denied before service request: proc=",
                );
                serial::write_str(current_process_name());
                serial::write_str(" file=");
                serial_write_vfs_name(node.name);
                serial::write_str(" source_kind=servicefs\n");
            }
            return Err(IpcError::VfsPermission);
        }
    }
    let mut created_node = None;
    let (node, available_rights) = match resolve_vfs_node_from_cap(cap, path) {
        Ok(resolved) => resolved,
        Err(IpcError::VfsNotFound) if flags & VFS_OPEN_CREATE != 0 => {
            let requested_rights = vfs_regular_file_open_rights(flags)?;
            let (node, available) = vfs_create_memory_file_node(cap, path, requested_rights)?;
            created_node = Some(node);
            (node, available)
        }
        Err(error) => return Err(error),
    };
    let requested_rights = match vfs_open_rights(flags, node) {
        Ok(rights) => rights,
        Err(error) => {
            release_created_vfs_memory_node(runtime(), created_node);
            return Err(error);
        }
    };
    if requested_rights & !available_rights != 0 {
        if !matches!(node.backing, VfsBacking::Device(_)) {
            release_created_vfs_memory_node(runtime(), created_node);
            return Err(IpcError::VfsPermission);
        }
    }
    if let VfsBacking::Device(device_object) = node.backing {
        if let Err(error) = validate_vfs_device_open(flags, available_rights, device_object) {
            release_created_vfs_memory_node(runtime(), created_node);
            return Err(error);
        }
    }
    if matches!(
        node.backing,
        VfsBacking::StateVolumeValue(_) | VfsBacking::StateVolumeControl(_)
    ) && flags & (VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND) != 0
    {
        release_created_vfs_memory_node(runtime(), created_node);
        return Err(IpcError::VfsUnsupported);
    }
    if matches!(node.backing, VfsBacking::FsServiceReport)
        && flags & (VFS_OPEN_WRITE | VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND) != 0
    {
        release_created_vfs_memory_node(runtime(), created_node);
        return Err(IpcError::VfsUnsupported);
    }
    if flags & VFS_OPEN_TRUNC != 0
        && !matches!(
            node.backing,
            VfsBacking::MemoryFile(_) | VfsBacking::VertexFsFile(_)
        )
    {
        release_created_vfs_memory_node(runtime(), created_node);
        return Err(IpcError::VfsUnsupported);
    }

    let owner = current_process_id();
    let (raw_handle, description) = {
        let runtime = runtime();
        let description =
            match runtime.open_file_description(node.id, requested_rights, flags, owner, cap.id) {
                Ok(description) => description,
                Err(error) => {
                    release_created_vfs_memory_node(runtime, created_node);
                    return Err(error);
                }
            };
        let Some(process) = runtime.processes.current_process_mut() else {
            let _ = runtime.release_file_description(description);
            release_created_vfs_memory_node(runtime, created_node);
            return Err(IpcError::VfsPermission);
        };
        let handle = FileHandle { description };
        match process.open_file_handle(handle) {
            Ok(raw) => (raw, description),
            Err(error) => {
                let _ = runtime.release_file_description(description);
                release_created_vfs_memory_node(runtime, created_node);
                return Err(error);
            }
        }
    };
    if flags & VFS_OPEN_TRUNC != 0 {
        if let Err(error) = vfs_truncate_node(node, 0) {
            let runtime = runtime();
            if let Some(process) = runtime.processes.current_process_mut() {
                let _ = process.close_file_handle(raw_handle);
            }
            let _ = runtime.release_file_description(description);
            release_created_vfs_memory_node(runtime, created_node);
            return Err(error);
        }
    }

    if created_node.is_some() {
        if let Some(parent) = node.parent {
            runtime().record_vfs_event(parent, VFS_EVENT_CREATE, node.name);
        }
        serial::write_str("VFS open-create accepted: proc=");
        serial::write_str(current_process_name());
        serial::write_str(" path=");
        serial::write_ascii_bytes(path);
        serial::write_str("\n");
    }
    serial::write_str("VFS open accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" handle=");
    serial::write_u64_dec(raw_handle);
    serial::write_str(" description=");
    serial::write_u64_dec(description.raw());
    serial::write_str("\n");
    Ok(raw_handle)
}

pub fn vfs_read(
    handle: u64,
    destination: *mut u8,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let (description, node) = current_open_file(handle)?;
    if description.rights & capability::RIGHT_READ == 0 {
        return Err(IpcError::VfsPermission);
    }
    if matches!(node.backing, VfsBacking::Pipe) {
        if max_len == 0 {
            frame.rax = 0;
            return Ok(());
        }
        usercopy::validate_user_buffer(
            UserPtr::new(destination as u64),
            max_len,
            paging::UserAccess::Write,
        )
        .map_err(|_| IpcError::InvalidUserBuffer)?;
        if !runtime().vfs_pipe.is_empty() {
            let copy_len = min(runtime().vfs_pipe.len, max_len);
            usercopy::copy_to_user(
                UserPtr::new(destination as u64),
                &runtime().vfs_pipe.bytes[..copy_len],
            )
            .map_err(|_| IpcError::InvalidUserBuffer)?;
            runtime().vfs_pipe.len = 0;
            serial::write_str("VFS pipe buffered read accepted: proc=");
            serial::write_str(current_process_name());
            serial::write_str(" file=");
            serial_write_vfs_name(node.name);
            serial::write_str(" bytes=");
            serial::write_u64_dec(copy_len as u64);
            serial::write_str("\n");
            frame.rax = copy_len as u64;
            return Ok(());
        }
        if block_current_on_vfs_read(node.id, description.id, destination as u64, max_len, frame) {
            return Ok(());
        }
        return Err(IpcError::Empty);
    }
    if let VfsBacking::StateVolumeValue(state) = node.backing {
        return vfs_state_value_read(
            state,
            node,
            description,
            description.offset,
            destination,
            max_len,
            true,
            frame,
        );
    }
    if matches!(node.backing, VfsBacking::FsServiceReport) {
        return start_vfs_service_read_transaction(
            node,
            description,
            description.offset,
            destination as u64,
            max_len,
            true,
            frame,
        );
    }
    let (copy_len, new_offset) = vfs_read_node(node, description.offset, destination, max_len)?;
    runtime()
        .file_description_mut(description.id)
        .ok_or(IpcError::VfsBadHandle)?
        .offset = new_offset;

    serial::write_str("VFS read accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");
    frame.rax = copy_len as u64;
    Ok(())
}

pub fn vfs_pread(
    handle: u64,
    destination: *mut u8,
    packed_len_offset: u64,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let max_len = usize::try_from(packed_len_offset & 0xffff_ffff).unwrap_or(usize::MAX);
    let offset = packed_len_offset >> 32;
    let (description, node) = current_open_file(handle)?;
    if description.rights & capability::RIGHT_READ == 0 {
        return Err(IpcError::VfsPermission);
    }
    if let VfsBacking::StateVolumeValue(state) = node.backing {
        return vfs_state_value_read(
            state,
            node,
            description,
            offset,
            destination,
            max_len,
            false,
            frame,
        );
    }
    if matches!(node.backing, VfsBacking::FsServiceReport) {
        return start_vfs_service_read_transaction(
            node,
            description,
            offset,
            destination as u64,
            max_len,
            false,
            frame,
        );
    }
    let (copy_len, _) = vfs_read_node(node, offset, destination, max_len)?;
    serial::write_str("VFS pread accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");
    frame.rax = copy_len as u64;
    Ok(())
}

pub fn vfs_write(
    handle: u64,
    source: *const u8,
    len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let (description, node) = current_open_file(handle)?;
    if let VfsBacking::StateVolumeControl(state) = node.backing {
        if description.rights & capability::RIGHT_CONTROL == 0 {
            return Err(IpcError::VfsPermission);
        }
        if description.flags & VFS_OPEN_APPEND != 0 {
            return Err(IpcError::VfsUnsupported);
        }
        return vfs_state_control_write(
            state,
            node,
            description,
            description.offset,
            source,
            len,
            true,
            frame,
        );
    }
    if description.rights & capability::RIGHT_WRITE == 0 {
        return Err(IpcError::VfsPermission);
    }
    if let VfsBacking::StateVolumeValue(state) = node.backing {
        if description.flags & VFS_OPEN_APPEND != 0 {
            return Err(IpcError::VfsUnsupported);
        }
        return vfs_state_value_write(
            state,
            node,
            description,
            description.offset,
            source,
            len,
            true,
            frame,
        );
    }
    if len > MAX_VFS_MEM_FILE_BYTES {
        return Err(IpcError::VfsNoSpace);
    }
    let mut bytes = [0u8; MAX_VFS_MEM_FILE_BYTES];
    usercopy::copy_from_user(&mut bytes, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let offset = if description.flags & VFS_OPEN_APPEND != 0 {
        vfs_node_len(node)?
    } else {
        description.offset
    };
    let (written, new_offset) = vfs_write_node(node, offset, &bytes[..len])?;
    runtime()
        .file_description_mut(description.id)
        .ok_or(IpcError::VfsBadHandle)?
        .offset = new_offset;
    serial::write_str("VFS write accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(written as u64);
    serial::write_str("\n");
    frame.rax = written as u64;
    Ok(())
}

pub fn vfs_pwrite(
    handle: u64,
    source: *const u8,
    packed_len_offset: u64,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let len = usize::try_from(packed_len_offset & 0xffff_ffff).unwrap_or(usize::MAX);
    let offset = packed_len_offset >> 32;
    let (description, node) = current_open_file(handle)?;
    if let VfsBacking::StateVolumeControl(state) = node.backing {
        if description.rights & capability::RIGHT_CONTROL == 0 {
            return Err(IpcError::VfsPermission);
        }
        return vfs_state_control_write(
            state,
            node,
            description,
            offset,
            source,
            len,
            false,
            frame,
        );
    }
    if description.rights & capability::RIGHT_WRITE == 0 {
        return Err(IpcError::VfsPermission);
    }
    if let VfsBacking::StateVolumeValue(state) = node.backing {
        return vfs_state_value_write(state, node, description, offset, source, len, false, frame);
    }
    if len > MAX_VFS_MEM_FILE_BYTES {
        return Err(IpcError::VfsNoSpace);
    }
    let mut bytes = [0u8; MAX_VFS_MEM_FILE_BYTES];
    usercopy::copy_from_user(&mut bytes, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let (written, _) = vfs_write_node(node, offset, &bytes[..len])?;
    serial::write_str("VFS pwrite accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(written as u64);
    serial::write_str("\n");
    frame.rax = written as u64;
    Ok(())
}

fn vfs_state_value_read(
    state: KernelObjectId,
    node: VfsNode,
    description: OpenFileDescription,
    offset: u64,
    destination: *mut u8,
    max_len: usize,
    update_offset: bool,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if max_len == 0 {
        frame.rax = 0;
        return Ok(());
    }
    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        max_len,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;

    let request = [0u8; MAX_MESSAGE_BYTES];
    start_vfs_state_transaction(
        state,
        node,
        description,
        VfsStateOperation::Read,
        offset,
        destination as u64,
        max_len,
        0,
        update_offset,
        &request,
        0,
        frame,
    )
}

fn vfs_state_value_write(
    state: KernelObjectId,
    node: VfsNode,
    description: OpenFileDescription,
    offset: u64,
    source: *const u8,
    len: usize,
    update_offset: bool,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if offset != 0 {
        return Err(IpcError::VfsUnsupported);
    }
    if len > MAX_STATE_VOLUME_VALUE_BYTES {
        return Err(IpcError::VfsNoSpace);
    }

    let mut request = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut request, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    start_vfs_state_transaction(
        state,
        node,
        description,
        VfsStateOperation::Write,
        offset,
        0,
        0,
        len,
        update_offset,
        &request,
        len,
        frame,
    )
}

fn vfs_state_control_write(
    state: KernelObjectId,
    node: VfsNode,
    description: OpenFileDescription,
    offset: u64,
    source: *const u8,
    len: usize,
    update_offset: bool,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if offset != 0 || len != 1 {
        return Err(IpcError::VfsUnsupported);
    }

    let mut request = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut request[..len], UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    if request[0] != b'Q' {
        return Err(IpcError::VfsUnsupported);
    }
    start_vfs_state_transaction(
        state,
        node,
        description,
        VfsStateOperation::Control,
        offset,
        0,
        0,
        len,
        update_offset,
        &request,
        1,
        frame,
    )
}

pub fn vfs_close(handle: u64) -> Result<(), IpcError> {
    let (process_name, file) = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::VfsPermission);
        };
        (process.name, process.close_file_handle(handle)?)
    };
    runtime().release_file_description(file.description)?;
    serial::write_str("VFS close accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" handle=");
    serial::write_u64_dec(handle);
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_seek(handle: u64, offset: u64, whence: u64) -> Result<u64, IpcError> {
    let (description, node) = current_open_file(handle)?;
    let size = vfs_node_len(node)?;
    let next = match whence {
        VFS_SEEK_SET => offset,
        VFS_SEEK_CURRENT => description
            .offset
            .checked_add(offset)
            .ok_or(IpcError::VfsUnsupported)?,
        VFS_SEEK_END => size.checked_add(offset).ok_or(IpcError::VfsUnsupported)?,
        _ => return Err(IpcError::VfsUnsupported),
    };
    if next > size {
        return Err(IpcError::VfsUnsupported);
    }
    runtime()
        .file_description_mut(description.id)
        .ok_or(IpcError::VfsBadHandle)?
        .offset = next;
    Ok(next)
}

pub fn vfs_stat(
    handle: u64,
    destination: *mut u8,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if max_len < VFS_STAT_BYTES {
        return Err(IpcError::InvalidUserBuffer);
    }
    let (description, node) = current_open_file(handle)?;
    if let VfsBacking::StateVolumeValue(state) = node.backing {
        return vfs_state_value_stat(state, node, description, destination, max_len, frame);
    }
    let mut stat = [0u8; VFS_STAT_BYTES];
    write_vfs_stat_record(&mut stat, node, vfs_node_len(node)?, description.rights);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &stat)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    serial::write_str("VFS stat accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str("\n");
    frame.rax = VFS_STAT_BYTES as u64;
    Ok(())
}

fn vfs_state_value_stat(
    state: KernelObjectId,
    node: VfsNode,
    description: OpenFileDescription,
    destination: *mut u8,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        VFS_STAT_BYTES,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;

    let request = [0u8; MAX_MESSAGE_BYTES];
    start_vfs_state_transaction(
        state,
        node,
        description,
        VfsStateOperation::Stat,
        0,
        destination as u64,
        max_len,
        0,
        false,
        &request,
        0,
        frame,
    )
}

pub fn vfs_readdir(handle: u64, destination: *mut u8, max_len: usize) -> Result<usize, IpcError> {
    if max_len < VFS_DIRENT_BYTES {
        return Err(IpcError::InvalidUserBuffer);
    }
    let (description, node) = current_open_file(handle)?;
    if !matches!(node.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    if description.rights & capability::RIGHT_RESOLVE == 0 {
        return Err(IpcError::VfsPermission);
    }
    let entry_index = usize::try_from(description.offset).map_err(|_| IpcError::VfsUnsupported)?;
    let Some(child) = runtime().vfs_child_by_entry_index(node.id, entry_index) else {
        return Ok(0);
    };

    let mut dirent = [0u8; VFS_DIRENT_BYTES];
    write_vfs_dirent_record(&mut dirent, child);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &dirent)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    runtime()
        .file_description_mut(description.id)
        .ok_or(IpcError::VfsBadHandle)?
        .offset = description
        .offset
        .checked_add(1)
        .ok_or(IpcError::VfsUnsupported)?;

    serial::write_str("VFS readdir accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" dir=");
    serial_write_vfs_name(node.name);
    serial::write_str(" entry=");
    serial_write_vfs_name(child.name);
    serial::write_str(" vnode=");
    serial::write_u64_dec(child.id.raw());
    serial::write_str("\n");
    Ok(VFS_DIRENT_BYTES)
}

pub fn vfs_mount(cap_slot: u64, path: *const u8, packed_len_flags: u64) -> Result<(), IpcError> {
    let path_len = usize::try_from(packed_len_flags & 0xffff_ffff).unwrap_or(usize::MAX);
    let flags = packed_len_flags >> 32;
    if path_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    if flags & !VFS_MOUNT_KNOWN_FLAGS != 0
        || flags == 0
        || (flags & VFS_MOUNT_VOLATILE != 0 && flags != VFS_MOUNT_VOLATILE)
        || (flags & VFS_MOUNT_BIND != 0 && flags & VFS_MOUNT_VOLATILE != 0)
        || (flags & VFS_MOUNT_READ_ONLY != 0 && flags & VFS_MOUNT_BIND == 0)
    {
        return Err(IpcError::VfsUnsupported);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let requested_path = &path_bytes[..path_len];
    let root_path = resolve_process_vfs_path(requested_path)?;
    let path = root_path.as_bytes();
    let (parent_path, child_name) = split_vfs_parent_child(path)?;
    let child_name = VfsName::from_user_component(child_name).map_err(|_| IpcError::VfsBadPath)?;
    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let available = resolve_vfs_root_authority(cap, parent_path)?;
    if available & capability::RIGHT_MOUNT == 0 {
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
    if flags & VFS_MOUNT_BIND != 0 {
        let source_root = runtime()
            .objects
            .get_vfs_root(cap.object)
            .ok_or(IpcError::VfsPermission)?;
        let source_node = runtime()
            .vfs_node_by_path(source_root.root_path.as_bytes())
            .ok_or(IpcError::VfsNotFound)?;
        if !matches!(source_node.kind, VfsNodeKind::Directory) {
            return Err(IpcError::VfsNotDirectory);
        }
        let source_mount_flags = runtime()
            .objects
            .get_vfs_mount_by_path(source_root.root_path.as_bytes())
            .map(|mount| mount.flags)
            .unwrap_or(0);
        let bind_flags = flags | (source_mount_flags & VFS_MOUNT_READ_ONLY);
        runtime()
            .add_vfs_mount(
                "mount:bind",
                source_node.id,
                root_path,
                source_node.mount_source,
                bind_flags,
                true,
                current_process_id(),
            )
            .map_err(|_| IpcError::VfsNoSpace)?;

        serial::write_str("VFS bind mount accepted: proc=");
        serial::write_str(current_process_name());
        serial::write_str(" path=");
        serial::write_ascii_bytes(requested_path);
        serial::write_str(" canonical=");
        serial::write_ascii_bytes(path);
        serial::write_str(" source=");
        serial::write_ascii_bytes(source_root.root_path.as_bytes());
        serial::write_str(" flags=");
        serial_write_vfs_mount_flags(bind_flags);
        serial::write_str("\n");
        return Ok(());
    }

    let runtime = runtime();
    let node_id = runtime
        .add_vfs_node_with_name(
            child_name,
            Some(parent.id),
            VfsNodeKind::Directory,
            VfsBacking::None,
            "volatilefs",
        )
        .map_err(|_| IpcError::VfsNoSpace)?;
    if runtime
        .add_vfs_mount(
            "mount:volatile",
            node_id,
            root_path,
            "volatilefs",
            flags,
            true,
            current_process_id(),
        )
        .is_err()
    {
        let _ = runtime.remove_vfs_node(node_id);
        return Err(IpcError::VfsNoSpace);
    }

    serial::write_str("VFS mount accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" path=");
    serial::write_ascii_bytes(requested_path);
    serial::write_str(" canonical=");
    serial::write_ascii_bytes(path);
    serial::write_str(" source=volatilefs\n");
    Ok(())
}

pub fn vfs_unmount(cap_slot: u64, path: *const u8, path_len: usize) -> Result<(), IpcError> {
    if path_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let requested_path = &path_bytes[..path_len];
    let canonical_path = resolve_process_vfs_path(requested_path)?;
    let path = canonical_path.as_bytes();
    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let available = resolve_vfs_root_authority(cap, path)?;
    if available & capability::RIGHT_MOUNT == 0 {
        return Err(IpcError::VfsPermission);
    }
    let exact_dynamic_bind = runtime()
        .objects
        .get_vfs_mount_by_exact_path(path)
        .filter(|mount| mount.dynamic && mount.flags & VFS_MOUNT_BIND != 0);
    let node = runtime().vfs_node_by_path(path);
    let mount = if let Some(mount) = exact_dynamic_bind {
        mount
    } else {
        let node = node.ok_or(IpcError::VfsNotFound)?;
        if !matches!(node.kind, VfsNodeKind::Directory) {
            return Err(IpcError::VfsNotDirectory);
        }
        runtime()
            .objects
            .get_vfs_mount_by_root_node(node.id)
            .ok_or(IpcError::VfsUnsupported)?
    };
    if !mount.dynamic {
        return Err(IpcError::VfsUnsupported);
    }
    if runtime().vfs_subtree_has_open_description(mount.root_node)
        || (mount.flags & VFS_MOUNT_BIND == 0
            && node.is_some_and(|node| runtime().vfs_node_has_children(node.id)))
    {
        return Err(IpcError::VfsBusy);
    }

    let runtime = runtime();
    if mount.flags & VFS_MOUNT_BIND == 0
        && let Some(node) = node
    {
        runtime.remove_vfs_node(node.id)?;
    }
    let removed_mount = if mount.flags & VFS_MOUNT_BIND != 0 {
        runtime.objects.remove_dynamic_vfs_mount_by_path(path)
    } else {
        runtime.objects.remove_dynamic_vfs_mount(mount.root_node)
    };
    if let Some(mount_id) = removed_mount {
        runtime.remove_vfs_mount_id(mount_id);
    }

    serial::write_str("VFS unmount accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" path=");
    serial::write_ascii_bytes(requested_path);
    serial::write_str(" canonical=");
    serial::write_ascii_bytes(path);
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_sync(handle: u64, frame: &mut SyscallFrame) -> Result<(), IpcError> {
    let (_description, node) = current_open_file(handle)?;
    match node.backing {
        VfsBacking::StoreObject(_)
        | VfsBacking::StateVolumeValue(_)
        | VfsBacking::MemoryFile(_)
        | VfsBacking::VertexFsFile(_)
        | VfsBacking::Synthetic(_) => {}
        _ => return Err(IpcError::VfsUnsupported),
    }
    if let VfsBacking::VertexFsFile(backing) = node.backing {
        match runtime().prepare_vertexfs_sync_file(backing)? {
            VertexFsSyncResult::Journaled {
                inode_id,
                checksum,
                write_count,
            } => {
                return start_vertexfs_sync_transaction(
                    backing,
                    inode_id,
                    checksum,
                    write_count,
                    frame,
                );
            }
            VertexFsSyncResult::Cached { checksum } => {
                let format = vertexfs_format_label(&runtime().vertexfs_image).unwrap_or("v1");
                serial::write_str("VertexFS ");
                serial::write_str(format);
                serial::write_str(" fsync cached runtime file=");
                serial_write_vfs_name(node.name);
                serial::write_str(" checksum=");
                serial::write_u64_dec(checksum as u64);
                serial::write_str("\n");
            }
        }
    }
    serial::write_str("VFS sync accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str("\n");
    frame.rax = STATUS_OK;
    Ok(())
}

pub fn vfs_dup(handle: u64, flags: u64) -> Result<u64, IpcError> {
    if flags & !VFS_DUP_SHARE_OFFSET != 0 {
        return Err(IpcError::VfsUnsupported);
    }
    let file = current_file_handle(handle)?;
    let description = runtime()
        .file_description(file.description)
        .ok_or(IpcError::VfsBadHandle)?;
    if runtime().cap_id_revoked_or_has_revoked_ancestor(description.authority_cap_id) {
        return Err(IpcError::VfsPermission);
    }
    let new_description = if flags & VFS_DUP_SHARE_OFFSET != 0 {
        runtime().retain_file_description(description.id)?;
        description.id
    } else {
        let runtime = runtime();
        let new_id = runtime.open_file_description(
            description.node,
            description.rights,
            description.flags,
            description.owner,
            description.authority_cap_id,
        )?;
        runtime
            .file_description_mut(new_id)
            .ok_or(IpcError::VfsBadHandle)?
            .offset = description.offset;
        new_id
    };

    let raw = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            runtime.release_file_description(new_description)?;
            return Err(IpcError::VfsPermission);
        };
        match process.open_file_handle(FileHandle {
            description: new_description,
        }) {
            Ok(raw) => raw,
            Err(error) => {
                runtime.release_file_description(new_description)?;
                return Err(error);
            }
        }
    };
    serial::write_str("VFS dup accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" handle=");
    serial::write_u64_dec(handle);
    serial::write_str(" new_handle=");
    serial::write_u64_dec(raw);
    serial::write_str(if flags & VFS_DUP_SHARE_OFFSET != 0 {
        " shared-offset=yes\n"
    } else {
        " shared-offset=no\n"
    });
    Ok(raw)
}

pub fn vfs_poll(handle: u64, events: u64, _timeout_ms: u64) -> Result<u64, IpcError> {
    if events == 0 || events & !VFS_POLL_KNOWN_EVENTS != 0 {
        return Err(IpcError::VfsUnsupported);
    }
    let (description, node) = current_open_file(handle)?;
    if events & VFS_POLL_READABLE != 0 && description.rights & capability::RIGHT_READ == 0 {
        return Err(IpcError::VfsPermission);
    }
    if events & VFS_POLL_WRITABLE != 0 && description.rights & capability::RIGHT_WRITE == 0 {
        return Err(IpcError::VfsPermission);
    }
    let ready = vfs_poll_ready(description, node, events)?;
    serial::write_str("VFS poll accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" events=");
    serial::write_u64_dec(events);
    serial::write_str(" ready=");
    serial::write_u64_dec(ready);
    serial::write_str("\n");
    Ok(ready)
}

pub fn vfs_watch(handle: u64, destination: *mut u8, max_len: usize) -> Result<usize, IpcError> {
    if max_len < VFS_WATCH_EVENT_BYTES {
        return Err(IpcError::InvalidUserBuffer);
    }
    let (description, node) = current_open_file(handle)?;
    if !matches!(node.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    if description.rights & capability::RIGHT_RESOLVE == 0 {
        return Err(IpcError::VfsPermission);
    }
    let start = min(description.watch_cursor, runtime().vfs_event_count);
    let mut event_index = start;
    let mut event = None;
    while event_index < runtime().vfs_event_count {
        if let Some(candidate) = runtime().vfs_events[event_index]
            && candidate.parent == node.id
        {
            event = Some(candidate);
            break;
        }
        event_index += 1;
    }
    let Some(event) = event else {
        return Ok(0);
    };
    let mut record = [0u8; VFS_WATCH_EVENT_BYTES];
    write_vfs_watch_event_record(&mut record, event);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &record)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    runtime()
        .file_description_mut(description.id)
        .ok_or(IpcError::VfsBadHandle)?
        .watch_cursor = event_index + 1;

    serial::write_str("VFS watch event delivered: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" dir=");
    serial_write_vfs_name(node.name);
    serial::write_str(" event=");
    serial::write_u64_dec(event.kind);
    serial::write_str(" name=");
    serial_write_vfs_name(event.name);
    serial::write_str("\n");
    Ok(VFS_WATCH_EVENT_BYTES)
}

pub fn vfs_lock(handle: u64, flags: u64, packed_range: u64) -> Result<(), IpcError> {
    let mode = match flags & VFS_LOCK_MODE_MASK {
        VFS_LOCK_SHARED => VfsLockMode::Shared,
        VFS_LOCK_EXCLUSIVE => VfsLockMode::Exclusive,
        _ => return Err(IpcError::VfsUnsupported),
    };
    if flags & !(VFS_LOCK_MODE_MASK | VFS_LOCK_RANGE) != 0 {
        return Err(IpcError::VfsUnsupported);
    }
    let (start, len) = if flags & VFS_LOCK_RANGE != 0 {
        let len = packed_range >> 32;
        let start = packed_range & 0xffff_ffff;
        if len == 0 {
            return Err(IpcError::VfsUnsupported);
        }
        (start, len)
    } else {
        (0, u64::MAX)
    };
    let (description, node) = current_open_file(handle)?;
    if !matches!(node.kind, VfsNodeKind::RegularFile) {
        return Err(IpcError::VfsNotFile);
    }
    match mode {
        VfsLockMode::Shared => {
            if description.rights & capability::RIGHT_READ == 0 {
                return Err(IpcError::VfsPermission);
            }
        }
        VfsLockMode::Exclusive => {
            if description.rights & capability::RIGHT_WRITE == 0 {
                return Err(IpcError::VfsPermission);
            }
        }
    }
    runtime().acquire_vfs_lock(description, mode, start, len)?;
    serial::write_str("VFS lock accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" description=");
    serial::write_u64_dec(description.id.raw());
    if flags & VFS_LOCK_RANGE != 0 {
        serial::write_str(" range=");
        serial::write_u64_dec(start);
        serial::write_str("+");
        serial::write_u64_dec(len);
    }
    serial::write_str(match mode {
        VfsLockMode::Shared => " mode=shared\n",
        VfsLockMode::Exclusive => " mode=exclusive\n",
    });
    Ok(())
}

pub fn vfs_unlock(handle: u64) -> Result<(), IpcError> {
    let (description, node) = current_open_file(handle)?;
    if !runtime().release_vfs_lock(description.id) {
        return Err(IpcError::VfsBadHandle);
    }
    serial::write_str("VFS unlock accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" description=");
    serial::write_u64_dec(description.id.raw());
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_create(cap_slot: u64, path: *const u8, packed_len_flags: u64) -> Result<(), IpcError> {
    let path_len = usize::try_from(packed_len_flags & 0xffff_ffff).unwrap_or(usize::MAX);
    let flags = packed_len_flags >> 32;
    if path_len > MAX_VFS_PATH_BYTES || flags != 0 {
        return Err(IpcError::VfsBadPath);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let path = &path_bytes[..path_len];
    if vfs_request_path_is_read_only(path)? {
        return Err(IpcError::VfsPermission);
    }
    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let (node, _) = vfs_create_memory_file_node(cap, path, 0)?;
    if let Some(parent) = node.parent {
        runtime().record_vfs_event(parent, VFS_EVENT_CREATE, node.name);
    }

    serial::write_str("VFS create accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" path=");
    serial::write_ascii_bytes(path);
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_mkdir(cap_slot: u64, path: *const u8, packed_len_flags: u64) -> Result<(), IpcError> {
    let path_len = usize::try_from(packed_len_flags & 0xffff_ffff).unwrap_or(usize::MAX);
    let flags = packed_len_flags >> 32;
    if path_len > MAX_VFS_PATH_BYTES || flags != 0 {
        return Err(IpcError::VfsBadPath);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let requested_path = &path_bytes[..path_len];
    if vfs_request_path_is_read_only(requested_path)? {
        return Err(IpcError::VfsPermission);
    }
    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let node = vfs_create_directory_node(cap, requested_path)?;
    if let Some(parent) = node.parent {
        runtime().record_vfs_event(parent, VFS_EVENT_CREATE, node.name);
    }

    serial::write_str("VFS mkdir accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" path=");
    serial::write_ascii_bytes(requested_path);
    serial::write_str(" vnode=");
    serial::write_u64_dec(node.id.raw());
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_unlink(cap_slot: u64, path: *const u8, path_len: usize) -> Result<(), IpcError> {
    if path_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let requested_path = &path_bytes[..path_len];
    let canonical_path = resolve_process_vfs_path(requested_path)?;
    let path = canonical_path.as_bytes();
    if vfs_path_is_read_only(path) {
        return Err(IpcError::VfsPermission);
    }
    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let available = resolve_vfs_root_authority(cap, path)?;
    if available & capability::RIGHT_UNLINK == 0 {
        return Err(IpcError::VfsPermission);
    }

    let node = runtime()
        .vfs_node_by_path(path)
        .ok_or(IpcError::VfsNotFound)?;
    let VfsBacking::MemoryFile(backing) = node.backing else {
        return Err(IpcError::VfsUnsupported);
    };
    if runtime().vfs_node_has_children(node.id) {
        return Err(IpcError::VfsBusy);
    }
    {
        let runtime = runtime();
        if runtime.vfs_node_has_open_description(node.id) {
            runtime.detach_vfs_node(node.id)?;
            runtime.touch_vfs_memory_file_nodes(backing)?;
        } else {
            runtime.remove_vfs_node(node.id)?;
            if runtime.vfs_memory_file_in_use(backing) {
                runtime.touch_vfs_memory_file_nodes(backing)?;
            } else {
                let _ = runtime.release_vfs_memory_file(backing);
            }
        }
    }
    if let Some(parent) = node.parent {
        runtime().record_vfs_event(parent, VFS_EVENT_UNLINK, node.name);
    }

    serial::write_str("VFS unlink accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" path=");
    serial::write_ascii_bytes(requested_path);
    serial::write_str(" canonical=");
    serial::write_ascii_bytes(path);
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_rmdir(cap_slot: u64, path: *const u8, path_len: usize) -> Result<(), IpcError> {
    if path_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let requested_path = &path_bytes[..path_len];
    let canonical_path = resolve_process_vfs_path(requested_path)?;
    let path = canonical_path.as_bytes();
    if vfs_path_is_read_only(path) {
        return Err(IpcError::VfsPermission);
    }
    let (parent_path, _) = split_vfs_parent_child(path)?;
    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let available = resolve_vfs_root_authority(cap, parent_path)?;
    if available & capability::RIGHT_UNLINK == 0 {
        return Err(IpcError::VfsPermission);
    }

    let node = runtime()
        .vfs_node_by_path(path)
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(node.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    if node.parent.is_none()
        || runtime()
            .objects
            .get_vfs_mount_by_root_node(node.id)
            .is_some()
    {
        return Err(IpcError::VfsUnsupported);
    }
    if runtime().vfs_node_has_children(node.id) || runtime().vfs_node_has_open_description(node.id)
    {
        return Err(IpcError::VfsBusy);
    }
    runtime().remove_vfs_node(node.id)?;
    if let Some(parent) = node.parent {
        runtime().record_vfs_event(parent, VFS_EVENT_UNLINK, node.name);
    }

    serial::write_str("VFS rmdir accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" path=");
    serial::write_ascii_bytes(requested_path);
    serial::write_str(" canonical=");
    serial::write_ascii_bytes(path);
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_rename(cap_slot: u64, request: *const u8, request_len: usize) -> Result<(), IpcError> {
    if !(VFS_RENAME_REQUEST_HEADER_BYTES..=VFS_RENAME_REQUEST_MAX_BYTES).contains(&request_len) {
        return Err(IpcError::VfsBadPath);
    }
    let mut request_bytes = [0u8; VFS_RENAME_REQUEST_MAX_BYTES];
    usercopy::copy_from_user(
        &mut request_bytes,
        UserPtr::new(request as u64),
        request_len,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;
    let old_len =
        usize::try_from(read_u64_le(&request_bytes, 0)).map_err(|_| IpcError::VfsBadPath)?;
    let new_len =
        usize::try_from(read_u64_le(&request_bytes, 8)).map_err(|_| IpcError::VfsBadPath)?;
    if old_len > MAX_VFS_PATH_BYTES || new_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let expected_len = VFS_RENAME_REQUEST_HEADER_BYTES
        .checked_add(old_len)
        .and_then(|len| len.checked_add(new_len))
        .ok_or(IpcError::VfsBadPath)?;
    if expected_len != request_len {
        return Err(IpcError::VfsBadPath);
    }

    let old_requested =
        &request_bytes[VFS_RENAME_REQUEST_HEADER_BYTES..VFS_RENAME_REQUEST_HEADER_BYTES + old_len];
    let new_requested = &request_bytes[VFS_RENAME_REQUEST_HEADER_BYTES + old_len..expected_len];
    let old_canonical = resolve_process_vfs_path(old_requested)?;
    let new_canonical = resolve_process_vfs_path(new_requested)?;
    let old_path = old_canonical.as_bytes();
    let new_path = new_canonical.as_bytes();
    if vfs_path_is_read_only(old_path) || vfs_path_is_read_only(new_path) {
        return Err(IpcError::VfsPermission);
    }
    let (old_parent_path, _) = split_vfs_parent_child(old_path)?;
    let (new_parent_path, new_child_name) = split_vfs_parent_child(new_path)?;
    let new_child_name =
        VfsName::from_user_component(new_child_name).map_err(|_| IpcError::VfsBadPath)?;

    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let old_available = resolve_vfs_root_authority(cap, old_parent_path)?;
    let new_available = resolve_vfs_root_authority(cap, new_parent_path)?;
    if old_available & capability::RIGHT_RENAME == 0
        || new_available & capability::RIGHT_RENAME == 0
    {
        return Err(IpcError::VfsPermission);
    }

    let old_parent = runtime()
        .vfs_node_by_path(old_parent_path)
        .ok_or(IpcError::VfsNotFound)?;
    let new_parent = runtime()
        .vfs_node_by_path(new_parent_path)
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(old_parent.kind, VfsNodeKind::Directory)
        || !matches!(new_parent.kind, VfsNodeKind::Directory)
    {
        return Err(IpcError::VfsNotDirectory);
    }
    let node = runtime()
        .vfs_node_by_path(old_path)
        .ok_or(IpcError::VfsNotFound)?;
    let VfsBacking::MemoryFile(_) = node.backing else {
        return Err(IpcError::VfsUnsupported);
    };
    let old_mount = runtime()
        .objects
        .get_vfs_mount_by_path(old_path)
        .ok_or(IpcError::VfsUnsupported)?;
    let new_mount = runtime()
        .objects
        .get_vfs_mount_by_path(new_parent_path)
        .ok_or(IpcError::VfsUnsupported)?;
    if old_mount.id != new_mount.id {
        return Err(IpcError::VfsUnsupported);
    }
    if runtime().vfs_node_by_path(new_path).is_some() {
        return Err(IpcError::VfsExists);
    }

    runtime().rename_vfs_node(node.id, new_parent.id, new_child_name)?;
    runtime().record_vfs_event(new_parent.id, VFS_EVENT_RENAME, new_child_name);

    serial::write_str("VFS rename accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" old=");
    serial::write_ascii_bytes(old_requested);
    serial::write_str(" new=");
    serial::write_ascii_bytes(new_requested);
    serial::write_str(" canonical_old=");
    serial::write_ascii_bytes(old_path);
    serial::write_str(" canonical_new=");
    serial::write_ascii_bytes(new_path);
    serial::write_str(" vnode=");
    serial::write_u64_dec(node.id.raw());
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_link(cap_slot: u64, request: *const u8, request_len: usize) -> Result<(), IpcError> {
    if !(VFS_RENAME_REQUEST_HEADER_BYTES..=VFS_RENAME_REQUEST_MAX_BYTES).contains(&request_len) {
        return Err(IpcError::VfsBadPath);
    }
    let mut request_bytes = [0u8; VFS_RENAME_REQUEST_MAX_BYTES];
    usercopy::copy_from_user(
        &mut request_bytes,
        UserPtr::new(request as u64),
        request_len,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;
    let old_len =
        usize::try_from(read_u64_le(&request_bytes, 0)).map_err(|_| IpcError::VfsBadPath)?;
    let new_len =
        usize::try_from(read_u64_le(&request_bytes, 8)).map_err(|_| IpcError::VfsBadPath)?;
    if old_len > MAX_VFS_PATH_BYTES || new_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let expected_len = VFS_RENAME_REQUEST_HEADER_BYTES
        .checked_add(old_len)
        .and_then(|len| len.checked_add(new_len))
        .ok_or(IpcError::VfsBadPath)?;
    if expected_len != request_len {
        return Err(IpcError::VfsBadPath);
    }

    let old_requested =
        &request_bytes[VFS_RENAME_REQUEST_HEADER_BYTES..VFS_RENAME_REQUEST_HEADER_BYTES + old_len];
    let new_requested = &request_bytes[VFS_RENAME_REQUEST_HEADER_BYTES + old_len..expected_len];
    let old_canonical = resolve_process_vfs_path(old_requested)?;
    let new_canonical = resolve_process_vfs_path(new_requested)?;
    let old_path = old_canonical.as_bytes();
    let new_path = new_canonical.as_bytes();
    if vfs_path_is_read_only(new_path) {
        return Err(IpcError::VfsPermission);
    }
    let (old_parent_path, _) = split_vfs_parent_child(old_path)?;
    let (new_parent_path, new_child_name) = split_vfs_parent_child(new_path)?;
    let new_child_name =
        VfsName::from_user_component(new_child_name).map_err(|_| IpcError::VfsBadPath)?;

    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    resolve_vfs_root_authority(cap, old_parent_path)?;
    let new_available = resolve_vfs_root_authority(cap, new_parent_path)?;
    if new_available & capability::RIGHT_CREATE == 0 {
        return Err(IpcError::VfsPermission);
    }

    let node = runtime()
        .vfs_node_by_path(old_path)
        .ok_or(IpcError::VfsNotFound)?;
    let VfsBacking::MemoryFile(backing) = node.backing else {
        return Err(IpcError::VfsUnsupported);
    };
    let new_parent = runtime()
        .vfs_node_by_path(new_parent_path)
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(new_parent.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    let old_mount = runtime()
        .objects
        .get_vfs_mount_by_path(old_path)
        .ok_or(IpcError::VfsUnsupported)?;
    let new_mount = runtime()
        .objects
        .get_vfs_mount_by_path(new_parent_path)
        .ok_or(IpcError::VfsUnsupported)?;
    if old_mount.id != new_mount.id {
        return Err(IpcError::VfsUnsupported);
    }
    if runtime().vfs_node_by_path(new_path).is_some() {
        return Err(IpcError::VfsExists);
    }

    let runtime = runtime();
    let new_node = runtime
        .add_vfs_node_with_name(
            new_child_name,
            Some(new_parent.id),
            VfsNodeKind::RegularFile,
            VfsBacking::MemoryFile(backing),
            node.mount_source,
        )
        .map_err(|_| IpcError::VfsNoSpace)?;
    runtime.touch_vfs_memory_file_nodes(backing)?;
    runtime.record_vfs_event(new_parent.id, VFS_EVENT_CREATE, new_child_name);

    serial::write_str("VFS link accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" old=");
    serial::write_ascii_bytes(old_requested);
    serial::write_str(" new=");
    serial::write_ascii_bytes(new_requested);
    serial::write_str(" vnode=");
    serial::write_u64_dec(node.id.raw());
    serial::write_str(" link_vnode=");
    serial::write_u64_dec(new_node.raw());
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_derive_root(
    cap_slot: u64,
    path: *const u8,
    packed_len_target: u64,
) -> Result<(), IpcError> {
    let path_len = usize::try_from(packed_len_target & 0xffff_ffff).unwrap_or(usize::MAX);
    let target_slot = packed_len_target >> 32;
    if path_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let requested_path = &path_bytes[..path_len];
    let root_path = resolve_process_vfs_path(requested_path)?;
    let source = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let source_root = runtime()
        .objects
        .get_vfs_root(source.object)
        .ok_or(IpcError::VfsPermission)?;
    if !vfs_authority_path_covers(source_root.root_path.as_bytes(), root_path.as_bytes()) {
        return Err(IpcError::VfsPermission);
    }
    let node = runtime()
        .vfs_node_by_path(root_path.as_bytes())
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(node.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }

    let process_name = current_process_name();
    let runtime = runtime();
    let owner = runtime
        .processes
        .current_process()
        .map(|process| process.pid)
        .ok_or(IpcError::VfsPermission)?;
    {
        let Some(process) = runtime.processes.current_process() else {
            return Err(IpcError::VfsPermission);
        };
        if !process.caps.can_grant(target_slot) {
            return Err(IpcError::VfsBadHandle);
        }
    }
    if runtime.next_cap_id == 0
        || runtime.next_cap_id == u64::MAX
        || runtime.cap_lineage_count == runtime.cap_lineage.len()
    {
        return Err(IpcError::VfsNoSpace);
    }
    let object = runtime.objects.add_derived_vfs_root(root_path)?;
    let cap = runtime.new_capability(
        object,
        source.rights & vfs_file_right_mask(),
        owner,
        source.id,
        owner,
    )?;
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::VfsPermission);
    };
    process
        .caps
        .grant(target_slot, cap)
        .map_err(|_| IpcError::VfsBadHandle)?;

    serial::write_str("VFS root derive accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" source=");
    serial::write_u64_dec(cap_slot);
    serial::write_str(" target=");
    serial::write_u64_dec(target_slot);
    serial::write_str(" root=");
    serial::write_ascii_bytes(root_path.as_bytes());
    serial::write_str(" rights=");
    print_rights(cap.rights);
    serial::write_str("\n");
    Ok(())
}
