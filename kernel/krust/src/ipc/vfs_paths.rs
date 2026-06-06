use super::*;

fn validate_vfs_absolute_path(path: &[u8]) -> Result<(), IpcError> {
    if path.len() < 2 || path[0] != b'/' || path[path.len() - 1] == b'/' {
        return Err(IpcError::VfsBadPath);
    }
    let mut start = 1;
    while start < path.len() {
        let mut end = start;
        while end < path.len() && path[end] != b'/' {
            if path[end] == 0 {
                return Err(IpcError::VfsBadPath);
            }
            end += 1;
        }
        if end == start
            || end - start > MAX_VFS_NAME_BYTES
            || &path[start..end] == b"."
            || &path[start..end] == b".."
        {
            return Err(IpcError::VfsBadPath);
        }
        start = end + 1;
    }
    Ok(())
}

pub(super) fn split_vfs_parent_child(path: &[u8]) -> Result<(&[u8], &[u8]), IpcError> {
    validate_vfs_absolute_path(path)?;
    let mut slash = path.len();
    while slash > 0 {
        slash -= 1;
        if path[slash] == b'/' {
            break;
        }
    }
    if slash == 0 {
        Ok((&path[..1], &path[1..]))
    } else {
        Ok((&path[..slash], &path[slash + 1..]))
    }
}

fn current_process_mount_root() -> Result<VfsPath, IpcError> {
    runtime()
        .processes
        .current_process()
        .map(|process| process.mount_root)
        .ok_or(IpcError::VfsPermission)
}

pub(super) fn resolve_process_vfs_path(path: &[u8]) -> Result<VfsPath, IpcError> {
    if path == b"/" {
        let root = current_process_mount_root()?;
        if root.as_bytes() != b"/" {
            serial::write_str("VFS namespace root resolved: proc=");
            serial::write_str(current_process_name());
            serial::write_str(" root=");
            serial::write_ascii_bytes(root.as_bytes());
            serial::write_str("\n");
        }
        return Ok(root);
    }
    validate_vfs_absolute_path(path)?;
    let root = current_process_mount_root()?;
    if root.as_bytes() == b"/" {
        return VfsPath::from_root_path(path);
    }
    let combined_len = root
        .len
        .checked_add(path.len())
        .ok_or(IpcError::VfsBadPath)?;
    if combined_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let mut resolved = VfsPath::empty();
    let mut index = 0;
    while index < root.len {
        resolved.bytes[index] = root.bytes[index];
        index += 1;
    }
    let mut path_index = 0;
    while path_index < path.len() {
        resolved.bytes[index] = path[path_index];
        index += 1;
        path_index += 1;
    }
    resolved.len = combined_len;
    Ok(resolved)
}

pub(super) fn resolve_vfs_path_under_root(root: VfsPath, path: &[u8]) -> Result<VfsPath, IpcError> {
    if path == b"/" {
        return Ok(root);
    }
    validate_vfs_absolute_path(path)?;
    if root.as_bytes() == b"/" {
        return VfsPath::from_root_path(path);
    }
    let combined_len = root
        .len
        .checked_add(path.len())
        .ok_or(IpcError::VfsBadPath)?;
    if combined_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let mut resolved = VfsPath::empty();
    let mut index = 0;
    while index < root.len {
        resolved.bytes[index] = root.bytes[index];
        index += 1;
    }
    let mut path_index = 0;
    while path_index < path.len() {
        resolved.bytes[index] = path[path_index];
        index += 1;
        path_index += 1;
    }
    resolved.len = combined_len;
    Ok(resolved)
}

pub(super) fn vfs_request_path_is_read_only(path: &[u8]) -> Result<bool, IpcError> {
    let canonical_path = if path.is_empty() {
        resolve_process_vfs_path(b"/")?
    } else {
        resolve_process_vfs_path(path)?
    };
    Ok(vfs_path_is_read_only(canonical_path.as_bytes()))
}

pub(super) fn vfs_path_is_read_only(path: &[u8]) -> bool {
    runtime()
        .objects
        .get_vfs_mount_by_path(path)
        .is_some_and(|mount| mount.flags & VFS_MOUNT_READ_ONLY != 0)
}
