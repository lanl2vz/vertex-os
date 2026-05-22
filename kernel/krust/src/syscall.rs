use core::arch::{asm, global_asm};

use crate::{gdt, serial};

const IA32_EFER: u32 = 0xc000_0080;
const IA32_STAR: u32 = 0xc000_0081;
const IA32_LSTAR: u32 = 0xc000_0082;
const IA32_FMASK: u32 = 0xc000_0084;

const EFER_SYSCALL_ENABLE: u64 = 1;
const RFLAGS_INTERRUPT_ENABLE: u64 = 1 << 9;

const SYSCALL_STACK_SIZE: usize = 16 * 1024;
const SYS_WRITE_SERIAL: u64 = 1;

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
    lea rsp, [rip + KRUST_SYSCALL_STACK + 16384]
    mov r12, rdi
    mov r13, rsi
    mov r14, rdx
    mov rdi, rax
    mov rsi, r12
    mov rdx, r13
    mov rcx, r14
    call krust_syscall_dispatch
1:
    hlt
    jmp 1b
"#
);

pub fn init() {
    let entry = krust_syscall_entry as *const () as usize as u64;
    let star = (gdt::KERNEL_CODE_SELECTOR as u64) << 32;

    unsafe {
        write_msr(IA32_STAR, star);
        write_msr(IA32_LSTAR, entry);
        write_msr(IA32_FMASK, RFLAGS_INTERRUPT_ENABLE);
        write_msr(IA32_EFER, read_msr(IA32_EFER) | EFER_SYSCALL_ENABLE);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn krust_syscall_dispatch(number: u64, arg0: u64, arg1: u64, _arg2: u64) -> ! {
    match number {
        SYS_WRITE_SERIAL => {
            serial::write_str("Userspace sys_write_serial: ");

            let bytes = unsafe { core::slice::from_raw_parts(arg0 as *const u8, arg1 as usize) };
            serial::write_ascii_bytes(bytes);
            serial::write_str("\nUserspace syscall demo ok\n");
        }
        _ => {
            serial::write_str("Unknown userspace syscall: ");
            serial::write_u64_dec(number);
            serial::write_str("\n");
        }
    }

    halt_loop()
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
