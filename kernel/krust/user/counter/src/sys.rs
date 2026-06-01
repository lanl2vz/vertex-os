use core::arch::asm;

pub const STATUS_OK: u64 = 0;
pub const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
pub const STATUS_EMPTY: u64 = u64::MAX - 4;

const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_IPC_RECV: u64 = 4;
const SYS_YIELD: u64 = 5;
const SYS_LOG_WRITE: u64 = 7;
const SYS_VFS_OPEN: u64 = 48;
const SYS_VFS_WRITE: u64 = 54;
const SYS_VFS_CLOSE: u64 = 50;

const VFS_OPEN_READ: u64 = 1;
const VFS_OPEN_WRITE: u64 = 1 << 1;

pub fn log(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_LOG_WRITE,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

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

pub fn yield_now() -> u64 {
    syscall3(SYS_YIELD, 0, 0, 0)
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
