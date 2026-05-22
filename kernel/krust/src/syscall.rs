use core::arch::{asm, global_asm};

use crate::{gdt, ipc, serial};

const IA32_EFER: u32 = 0xc000_0080;
const IA32_STAR: u32 = 0xc000_0081;
const IA32_LSTAR: u32 = 0xc000_0082;
const IA32_FMASK: u32 = 0xc000_0084;

const EFER_SYSCALL_ENABLE: u64 = 1;
const RFLAGS_INTERRUPT_ENABLE: u64 = 1 << 9;

const SYSCALL_STACK_SIZE: usize = 16 * 1024;
const SYS_WRITE_SERIAL: u64 = 1;
const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_IPC_RECV: u64 = 4;

const STATUS_OK: u64 = 0;
const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;
const STATUS_TOO_LARGE: u64 = u64::MAX - 3;
const STATUS_EMPTY: u64 = u64::MAX - 4;

#[repr(C, align(16))]
pub struct SyscallStack([u8; SYSCALL_STACK_SIZE]);

#[unsafe(no_mangle)]
static mut KRUST_SYSCALL_STACK: SyscallStack = SyscallStack([0; SYSCALL_STACK_SIZE]);

unsafe extern "C" {
    fn krust_syscall_entry();
}

global_asm!(
    r#"
    .global krust_syscall_entry
krust_syscall_entry:
    mov r10, rsp
    lea rsp, [rip + KRUST_SYSCALL_STACK + 16384]
    push r11
    push rcx
    push r10
    sub rsp, 8
    mov r12, rdi
    mov r13, rsi
    mov r14, rdx
    mov r15, r8
    mov rdi, rax
    mov rsi, r12
    mov rdx, r13
    mov rcx, r14
    mov r8, r15
    call krust_syscall_dispatch
    add rsp, 8
    pop r10
    pop rcx
    pop r11
    mov rsp, r10
    sysretq
"#
);

pub fn init() {
    let entry = krust_syscall_entry as *const () as usize as u64;
    let star =
        ((gdt::USER_SELECTOR_BASE as u64) << 48) | ((gdt::KERNEL_CODE_SELECTOR as u64) << 32);

    unsafe {
        write_msr(IA32_STAR, star);
        write_msr(IA32_LSTAR, entry);
        write_msr(IA32_FMASK, RFLAGS_INTERRUPT_ENABLE);
        write_msr(IA32_EFER, read_msr(IA32_EFER) | EFER_SYSCALL_ENABLE);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn krust_syscall_dispatch(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    match number {
        SYS_WRITE_SERIAL => match user_bytes(arg0, arg1) {
            Some(bytes) => {
                serial::write_str("Userspace sys_write_serial: ");
                serial::write_ascii_bytes(bytes);
                serial::write_str("\n");
                STATUS_OK
            }
            None => STATUS_BAD_BUFFER,
        },
        SYS_EXIT => exit_current_process(arg0),
        SYS_IPC_SEND => match ipc::send(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(()) => STATUS_OK,
            Err(error) => ipc_error_status(error),
        },
        SYS_IPC_RECV => {
            match ipc::receive(
                arg0,
                arg1 as *mut u8,
                usize::try_from(arg2).unwrap_or(usize::MAX),
            ) {
                Ok(len) => len as u64,
                Err(error) => ipc_error_status(error),
            }
        }
        _ => {
            serial::write_str("Unknown userspace syscall: ");
            serial::write_u64_dec(number);
            serial::write_str("\n");
            u64::MAX
        }
    }
}

fn exit_current_process(status: u64) -> ! {
    serial::write_str("Process exited: proc=");
    serial::write_str(ipc::current_process_name());
    serial::write_str(" status=");
    serial::write_u64_dec(status);
    serial::write_str("\n");

    match ipc::exit_current_process(status) {
        ipc::ExitAction::Switch { name, context } => {
            serial::write_str("Switching to process: ");
            serial::write_str(name);
            serial::write_str("\n");
            unsafe {
                gdt::enter_user_mode(context.cr3, context.entry, context.stack_top);
            }
        }
        ipc::ExitAction::Halt { ok } => {
            if ok {
                serial::write_str("IPC demo ok\n");
            } else {
                serial::write_str("IPC demo failed\n");
            }
            halt_loop()
        }
    }
}

fn user_bytes(pointer: u64, len: u64) -> Option<&'static [u8]> {
    const USER_CANONICAL_LIMIT: u64 = 0x0000_8000_0000_0000;

    let end = pointer.checked_add(len)?;
    if pointer >= USER_CANONICAL_LIMIT || end > USER_CANONICAL_LIMIT {
        return None;
    }

    Some(unsafe { core::slice::from_raw_parts(pointer as *const u8, len as usize) })
}

fn ipc_error_status(error: ipc::IpcError) -> u64 {
    match error {
        ipc::IpcError::BadCapability => {
            serial::write_str("IPC syscall rejected: bad capability\n");
            STATUS_BAD_CAPABILITY
        }
        ipc::IpcError::InvalidUserBuffer => {
            serial::write_str("IPC syscall rejected: bad user buffer\n");
            STATUS_BAD_BUFFER
        }
        ipc::IpcError::MessageTooLarge => {
            serial::write_str("IPC syscall rejected: message too large\n");
            STATUS_TOO_LARGE
        }
        ipc::IpcError::Empty => {
            serial::write_str("IPC syscall rejected: endpoint empty\n");
            STATUS_EMPTY
        }
    }
}

fn halt_loop() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

unsafe fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;

    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags)
        );
    }

    ((high as u64) << 32) | low as u64
}

unsafe fn write_msr(msr: u32, value: u64) {
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
            options(nomem, nostack, preserves_flags)
        );
    }
}
