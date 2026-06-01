use core::arch::asm;

pub const STATUS_OK: u64 = 0;
pub const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
pub const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;
pub const STATUS_VFS_PERMISSION: u64 = u64::MAX - 32;
pub const STATUS_VFS_BAD_HANDLE: u64 = u64::MAX - 38;
pub const STATUS_VFS_NO_SPACE: u64 = u64::MAX - 40;

const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_IPC_RECV: u64 = 4;
const SYS_LOG_WRITE: u64 = 7;
const SYS_PROCESS_CREATE: u64 = 9;
const SYS_LEGACY_OBJECT_READ: u64 = 13;
const SYS_IO_WRITE: u64 = 28;
const SYS_SECRET_READ: u64 = 39;
const SYS_VFS_OPEN: u64 = 48;
const SYS_VFS_READ: u64 = 49;
const SYS_VFS_CLOSE: u64 = 50;
const SYS_VFS_STAT: u64 = 51;
const SYS_VFS_SEEK: u64 = 52;
const SYS_VFS_PREAD: u64 = 53;
const SYS_VFS_WRITE: u64 = 54;
const SYS_VFS_SYNC: u64 = 56;
const SYS_VFS_DUP: u64 = 57;

const VFS_OPEN_READ: u64 = 1;
const VFS_DUP_SHARE_OFFSET: u64 = 1;
const VFS_SEEK_SET: u64 = 0;

pub fn ipc_send(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
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

pub fn ipc_recv_raw(cap_slot: u64, destination: u64, len: u64) -> u64 {
    syscall3(SYS_IPC_RECV, cap_slot, destination, len)
}

pub fn log(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_LOG_WRITE,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn process_create(cap_slot: u64, process_index: u64) -> u64 {
    syscall3(SYS_PROCESS_CREATE, cap_slot, process_index, 0)
}

pub fn io_write(cap_slot: u64, port: u64, value: u8) -> u64 {
    syscall3(SYS_IO_WRITE, cap_slot, port, value as u64)
}

pub fn legacy_object_read_raw(cap_slot: u64, destination: u64, len: u64) -> u64 {
    syscall3(SYS_LEGACY_OBJECT_READ, cap_slot, destination, len)
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

pub fn vfs_read(handle: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_VFS_READ,
        handle,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn vfs_read_raw(handle: u64, destination: u64, len: u64) -> u64 {
    syscall3(SYS_VFS_READ, handle, destination, len)
}

pub fn vfs_pread(handle: u64, buffer: &mut [u8], offset: u64) -> u64 {
    syscall3(
        SYS_VFS_PREAD,
        handle,
        buffer.as_mut_ptr() as u64,
        (offset << 32) | buffer.len() as u64,
    )
}

pub fn vfs_stat(handle: u64, buffer: &mut [u8; 32]) -> u64 {
    syscall3(
        SYS_VFS_STAT,
        handle,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn vfs_seek_set(handle: u64, offset: u64) -> u64 {
    syscall3(SYS_VFS_SEEK, handle, offset, VFS_SEEK_SET)
}

pub fn vfs_write(handle: u64, buffer: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_WRITE,
        handle,
        buffer.as_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn vfs_sync(handle: u64) -> u64 {
    syscall3(SYS_VFS_SYNC, handle, 0, 0)
}

pub fn vfs_dup(handle: u64) -> u64 {
    syscall3(SYS_VFS_DUP, handle, 0, 0)
}

pub fn vfs_dup_shared(handle: u64) -> u64 {
    syscall3(SYS_VFS_DUP, handle, VFS_DUP_SHARE_OFFSET, 0)
}

pub fn vfs_close(handle: u64) -> u64 {
    syscall3(SYS_VFS_CLOSE, handle, 0, 0)
}

pub fn secret_read(cap_slot: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_SECRET_READ,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn exit(status: u64) -> ! {
    syscall3(SYS_EXIT, status, 0, 0);
    loop {
        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
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
