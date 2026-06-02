use core::arch::asm;

pub const STATUS_OK: u64 = 0;
pub const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
pub const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;
pub const STATUS_VFS_PERMISSION: u64 = u64::MAX - 32;
pub const STATUS_VFS_BAD_PATH: u64 = u64::MAX - 33;
pub const STATUS_VFS_NOT_FOUND: u64 = u64::MAX - 34;
pub const STATUS_VFS_BUSY: u64 = u64::MAX - 37;
pub const STATUS_VFS_UNSUPPORTED: u64 = u64::MAX - 39;
pub const STATUS_VFS_NO_SPACE: u64 = u64::MAX - 40;
pub const RIGHT_SEND: u64 = 1 << 4;
pub const RIGHT_RECEIVE: u64 = 1 << 5;
pub const RIGHT_READ: u64 = 1 << 0;
pub const RIGHT_WRITE: u64 = 1 << 1;
pub const RIGHT_BIND: u64 = 1 << 10;
pub const RIGHT_RESOLVE: u64 = 1 << 23;
pub const RIGHT_CREATE: u64 = 1 << 15;
pub const RIGHT_UNLINK: u64 = 1 << 24;
pub const RIGHT_MOUNT: u64 = 1 << 26;

const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_IPC_RECV: u64 = 4;
const SYS_LOG_WRITE: u64 = 7;
const SYS_CAP_DROP: u64 = 11;
const SYS_LEGACY_OBJECT_READ: u64 = 13;
const SYS_PROCESS_ATTEMPT: u64 = 20;
const SYS_CAP_REVOKE: u64 = 21;
const SYS_CAP_INSPECT: u64 = 22;
const SYS_CAP_MOVE: u64 = 23;
const SYS_CAP_COPY: u64 = 24;
const SYS_ENDPOINT_CREATE: u64 = 25;
const SYS_IO_READ: u64 = 27;
const SYS_IO_WRITE: u64 = 28;
const SYS_IRQ_WAIT: u64 = 29;
const SYS_MMIO_MAP: u64 = 30;
const SYS_DMA_MAP: u64 = 32;
const SYS_SECRET_READ: u64 = 39;
const SYS_VIRTIO_NET_TX: u64 = 42;
const SYS_NETWORK_SEND_UDP: u64 = 44;
const SYS_NAMESPACE_RESOLVE: u64 = 45;
const SYS_VFS_OPEN: u64 = 48;
const SYS_VFS_READ: u64 = 49;
const SYS_VFS_CLOSE: u64 = 50;
const SYS_VFS_STAT: u64 = 51;
const SYS_VFS_WRITE: u64 = 54;
const SYS_VFS_CREATE: u64 = 58;
const SYS_VFS_UNLINK: u64 = 59;
const SYS_VFS_DERIVE_ROOT: u64 = 60;
const SYS_VFS_LOCK: u64 = 61;
const SYS_VFS_UNLOCK: u64 = 62;
const SYS_VFS_READDIR: u64 = 63;
const SYS_VFS_MOUNT: u64 = 64;
const SYS_VFS_UNMOUNT: u64 = 65;
const SYS_VFS_RENAME: u64 = 66;
const SYS_VFS_MKDIR: u64 = 67;
const SYS_VFS_RMDIR: u64 = 68;
const SYS_VFS_LINK: u64 = 69;

const VFS_OPEN_READ: u64 = 1;
const VFS_OPEN_WRITE: u64 = 1 << 1;
const VFS_OPEN_CREATE: u64 = 1 << 2;
const VFS_OPEN_TRUNC: u64 = 1 << 3;
const VFS_OPEN_APPEND: u64 = 1 << 4;
const VFS_LOCK_SHARED: u64 = 1;
const VFS_LOCK_EXCLUSIVE: u64 = 2;
const VFS_MOUNT_VOLATILE: u64 = 1;
const MAX_VFS_PATH_BYTES: usize = 128;
const VFS_RENAME_REQUEST_HEADER_BYTES: usize = 16;
const VFS_RENAME_REQUEST_MAX_BYTES: usize =
    VFS_RENAME_REQUEST_HEADER_BYTES + (MAX_VFS_PATH_BYTES * 2);

pub fn ipc_send(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_IPC_SEND,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn ipc_send_raw(cap_slot: u64, source: u64, len: u64) -> u64 {
    syscall3(SYS_IPC_SEND, cap_slot, source, len)
}

pub fn ipc_send_with_direction_flag(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3_with_direction_flag(
        SYS_IPC_SEND,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn ipc_recv(cap_slot: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_IPC_RECV,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn log(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_LOG_WRITE,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn cap_drop(cap_slot: u64) -> u64 {
    syscall3(SYS_CAP_DROP, cap_slot, 0, 0)
}

pub fn cap_revoke(cap_slot: u64) -> u64 {
    syscall3(SYS_CAP_REVOKE, cap_slot, 0, 0)
}

pub fn cap_inspect(cap_slot: u64) -> u64 {
    syscall3(SYS_CAP_INSPECT, cap_slot, 0, 0)
}

pub fn cap_move(source_slot: u64, target_slot: u64) -> u64 {
    syscall3(SYS_CAP_MOVE, source_slot, target_slot, 0)
}

pub fn cap_copy(source_slot: u64, target_slot: u64, rights: u64) -> u64 {
    syscall3(SYS_CAP_COPY, source_slot, target_slot, rights)
}

pub fn endpoint_create(control_slot: u64, cap_slot: u64) -> u64 {
    syscall3(SYS_ENDPOINT_CREATE, control_slot, cap_slot, 0)
}

pub fn io_write(cap_slot: u64, port: u64, value: u8) -> u64 {
    syscall3(SYS_IO_WRITE, cap_slot, port, value as u64)
}

pub fn io_read(cap_slot: u64, port: u64) -> u64 {
    syscall3(SYS_IO_READ, cap_slot, port, 0)
}

pub fn irq_wait(cap_slot: u64, timeout_ms: u64) -> u64 {
    syscall3(SYS_IRQ_WAIT, cap_slot, timeout_ms, 0)
}

pub fn mmio_map(cap_slot: u64) -> u64 {
    syscall3(SYS_MMIO_MAP, cap_slot, 0, 0)
}

pub fn dma_map(cap_slot: u64, buffer: &mut [u8; 24]) -> u64 {
    syscall3(
        SYS_DMA_MAP,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn legacy_object_read(cap_slot: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_LEGACY_OBJECT_READ,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn vfs_open_read(cap_slot: u64) -> u64 {
    syscall3(SYS_VFS_OPEN, cap_slot, 0, VFS_OPEN_READ << 32)
}

pub fn vfs_open_path_read(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_OPEN,
        cap_slot,
        path.as_ptr() as u64,
        (VFS_OPEN_READ << 32) | path.len() as u64,
    )
}

pub fn vfs_open_path_readwrite(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_OPEN,
        cap_slot,
        path.as_ptr() as u64,
        ((VFS_OPEN_READ | VFS_OPEN_WRITE) << 32) | path.len() as u64,
    )
}

pub fn vfs_open_path_write(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_OPEN,
        cap_slot,
        path.as_ptr() as u64,
        (VFS_OPEN_WRITE << 32) | path.len() as u64,
    )
}

pub fn vfs_open_path_create_readwrite(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_OPEN,
        cap_slot,
        path.as_ptr() as u64,
        ((VFS_OPEN_READ | VFS_OPEN_WRITE | VFS_OPEN_CREATE) << 32) | path.len() as u64,
    )
}

pub fn vfs_open_path_create_trunc_readwrite(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_OPEN,
        cap_slot,
        path.as_ptr() as u64,
        ((VFS_OPEN_READ | VFS_OPEN_WRITE | VFS_OPEN_CREATE | VFS_OPEN_TRUNC) << 32)
            | path.len() as u64,
    )
}

pub fn vfs_open_path_append_write(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_OPEN,
        cap_slot,
        path.as_ptr() as u64,
        ((VFS_OPEN_WRITE | VFS_OPEN_APPEND) << 32) | path.len() as u64,
    )
}

pub fn vfs_read(handle: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_VFS_READ,
        handle,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn vfs_readdir(handle: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_VFS_READDIR,
        handle,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn vfs_stat(handle: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_VFS_STAT,
        handle,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn vfs_write(handle: u64, buffer: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_WRITE,
        handle,
        buffer.as_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn vfs_close(handle: u64) -> u64 {
    syscall3(SYS_VFS_CLOSE, handle, 0, 0)
}

pub fn vfs_lock_shared(handle: u64) -> u64 {
    syscall3(SYS_VFS_LOCK, handle, VFS_LOCK_SHARED, 0)
}

pub fn vfs_lock_exclusive(handle: u64) -> u64 {
    syscall3(SYS_VFS_LOCK, handle, VFS_LOCK_EXCLUSIVE, 0)
}

pub fn vfs_unlock(handle: u64) -> u64 {
    syscall3(SYS_VFS_UNLOCK, handle, 0, 0)
}

pub fn vfs_create(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_CREATE,
        cap_slot,
        path.as_ptr() as u64,
        path.len() as u64,
    )
}

pub fn vfs_unlink(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_UNLINK,
        cap_slot,
        path.as_ptr() as u64,
        path.len() as u64,
    )
}

pub fn vfs_mkdir(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_MKDIR,
        cap_slot,
        path.as_ptr() as u64,
        path.len() as u64,
    )
}

pub fn vfs_rmdir(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_RMDIR,
        cap_slot,
        path.as_ptr() as u64,
        path.len() as u64,
    )
}

pub fn vfs_rename(cap_slot: u64, old_path: &[u8], new_path: &[u8]) -> u64 {
    if old_path.len() > MAX_VFS_PATH_BYTES || new_path.len() > MAX_VFS_PATH_BYTES {
        return STATUS_VFS_BAD_PATH;
    }
    let mut request = [0u8; VFS_RENAME_REQUEST_MAX_BYTES];
    write_u64_le(&mut request, 0, old_path.len() as u64);
    write_u64_le(&mut request, 8, new_path.len() as u64);
    let mut cursor = VFS_RENAME_REQUEST_HEADER_BYTES;
    let mut index = 0;
    while index < old_path.len() {
        request[cursor] = old_path[index];
        cursor += 1;
        index += 1;
    }
    index = 0;
    while index < new_path.len() {
        request[cursor] = new_path[index];
        cursor += 1;
        index += 1;
    }
    syscall3(
        SYS_VFS_RENAME,
        cap_slot,
        request.as_ptr() as u64,
        cursor as u64,
    )
}

pub fn vfs_link(cap_slot: u64, old_path: &[u8], new_path: &[u8]) -> u64 {
    if old_path.len() > MAX_VFS_PATH_BYTES || new_path.len() > MAX_VFS_PATH_BYTES {
        return STATUS_VFS_BAD_PATH;
    }
    let mut request = [0u8; VFS_RENAME_REQUEST_MAX_BYTES];
    write_u64_le(&mut request, 0, old_path.len() as u64);
    write_u64_le(&mut request, 8, new_path.len() as u64);
    let mut cursor = VFS_RENAME_REQUEST_HEADER_BYTES;
    let mut index = 0;
    while index < old_path.len() {
        request[cursor] = old_path[index];
        cursor += 1;
        index += 1;
    }
    index = 0;
    while index < new_path.len() {
        request[cursor] = new_path[index];
        cursor += 1;
        index += 1;
    }
    syscall3(
        SYS_VFS_LINK,
        cap_slot,
        request.as_ptr() as u64,
        cursor as u64,
    )
}

pub fn vfs_derive_root(cap_slot: u64, path: &[u8], target_slot: u64) -> u64 {
    syscall3(
        SYS_VFS_DERIVE_ROOT,
        cap_slot,
        path.as_ptr() as u64,
        (target_slot << 32) | path.len() as u64,
    )
}

pub fn vfs_mount_volatile(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_MOUNT,
        cap_slot,
        path.as_ptr() as u64,
        (VFS_MOUNT_VOLATILE << 32) | path.len() as u64,
    )
}

pub fn vfs_unmount(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_UNMOUNT,
        cap_slot,
        path.as_ptr() as u64,
        path.len() as u64,
    )
}

pub fn secret_read(cap_slot: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_SECRET_READ,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn virtio_net_tx(cap_slot: u64, frame: &[u8]) -> u64 {
    syscall3(
        SYS_VIRTIO_NET_TX,
        cap_slot,
        frame.as_ptr() as u64,
        frame.len() as u64,
    )
}

pub fn network_send_udp(cap_slot: u64, payload: &[u8]) -> u64 {
    syscall3(
        SYS_NETWORK_SEND_UDP,
        cap_slot,
        payload.as_ptr() as u64,
        payload.len() as u64,
    )
}

pub fn namespace_resolve(cap_slot: u64, path: &[u8], target_slot: u64) -> u64 {
    syscall3(
        SYS_NAMESPACE_RESOLVE,
        cap_slot,
        path.as_ptr() as u64,
        (target_slot << 32) | path.len() as u64,
    )
}

pub fn process_attempt() -> u64 {
    syscall3(SYS_PROCESS_ATTEMPT, 0, 0, 0)
}

pub fn exit(status: u64) -> ! {
    syscall3(SYS_EXIT, status, 0, 0);
    loop {
        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
}

fn write_u64_le(destination: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        destination[offset + index] = bytes[index];
        index += 1;
    }
}

fn syscall3(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    let result: u64;

    unsafe {
        asm!(
            "syscall",
            inlateout("rax") number => result,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") arg2,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }

    result
}

fn syscall3_with_direction_flag(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    let result: u64;

    unsafe {
        asm!(
            "std",
            "syscall",
            "cld",
            inlateout("rax") number => result,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") arg2,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }

    result
}
