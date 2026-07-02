use super::*;

pub(super) fn write_vfs_stat_record(
    stat: &mut [u8; VFS_STAT_BYTES],
    node: VfsNode,
    size: u64,
    rights: u64,
) {
    write_u64_le(stat, 0, vfs_node_kind_value(node.kind));
    write_u64_le(stat, 8, size);
    write_u64_le(stat, 16, runtime().vfs_node_stat_identity(node));
    write_u64_le(stat, 24, rights);
    write_u64_le(stat, 32, node.metadata_version);
    write_u64_le(stat, 40, runtime().vfs_node_link_count(node));
    write_u64_le(stat, 48, 0);
    write_u64_le(stat, 56, 0);
}

pub(super) fn write_vfs_dirent_record(dirent: &mut [u8; VFS_DIRENT_BYTES], child: VfsNode) {
    write_u64_le(dirent, 0, vfs_node_kind_value(child.kind));
    write_u64_le(dirent, 8, child.id.raw());
    write_u64_le(dirent, 16, child.name.len as u64);
    let name = child.name.as_bytes();
    let mut index = 0;
    while index < name.len() {
        dirent[24 + index] = name[index];
        index += 1;
    }
}

pub(super) fn write_vfs_watch_event_record(
    record: &mut [u8; VFS_WATCH_EVENT_BYTES],
    event: VfsEvent,
) {
    write_u64_le(record, 0, event.kind);
    write_u64_le(record, 8, event.metadata_version);
    write_u64_le(record, 16, event.name.len as u64);
    let name = event.name.as_bytes();
    let mut index = 0;
    while index < name.len() && 24 + index < record.len() {
        record[24 + index] = name[index];
        index += 1;
    }
}

fn vfs_node_kind_value(kind: VfsNodeKind) -> u64 {
    match kind {
        VfsNodeKind::RegularFile => VFS_NODE_KIND_REGULAR,
        VfsNodeKind::Directory => VFS_NODE_KIND_DIRECTORY,
        VfsNodeKind::DeviceNode => VFS_NODE_KIND_DEVICE,
        VfsNodeKind::Pipe => VFS_NODE_KIND_PIPE,
        VfsNodeKind::SyntheticNode => VFS_NODE_KIND_SYNTHETIC,
    }
}

pub(super) fn serial_write_vfs_name(name: VfsName) {
    serial::write_ascii_bytes(name.as_bytes());
}

pub(super) fn serial_write_vfs_mount_flags(flags: u64) {
    let mut wrote = false;
    if flags & VFS_MOUNT_VOLATILE != 0 {
        serial::write_str("volatile");
        wrote = true;
    }
    if flags & VFS_MOUNT_BIND != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("bind");
        wrote = true;
    }
    if flags & VFS_MOUNT_READ_ONLY != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("read-only");
        wrote = true;
    }
    if !wrote {
        serial::write_str("none");
    }
}

pub(super) fn write_u64_le(destination: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        destination[offset + index] = bytes[index];
        index += 1;
    }
}

pub(super) fn write_u16_le(destination: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    destination[offset] = bytes[0];
    destination[offset + 1] = bytes[1];
}

pub(super) fn read_u16_le(source: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([source[offset], source[offset + 1]])
}

pub(super) fn read_u64_le(source: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    let mut index = 0;
    while index < bytes.len() {
        bytes[index] = source[offset + index];
        index += 1;
    }
    u64::from_le_bytes(bytes)
}
