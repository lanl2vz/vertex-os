use core::arch::asm;

pub const KERNEL_CODE_SELECTOR: u16 = 0x08;
pub const USER_SELECTOR_BASE: u16 = 0x10;
const USER_DATA_SELECTOR: u16 = 0x18 | 3;
const USER_CODE_SELECTOR: u16 = 0x20 | 3;

const GDT: [u64; 5] = [
    0,
    0x00af_9b00_0000_ffff,
    0x00cf_9300_0000_ffff,
    0x00cf_f300_0000_ffff,
    0x00af_fb00_0000_ffff,
];

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

pub fn init() {
    let pointer = DescriptorTablePointer {
        limit: (core::mem::size_of_val(&GDT) - 1) as u16,
        base: GDT.as_ptr() as u64,
    };

    unsafe {
        asm!("lgdt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags));
    }
}

pub unsafe fn enter_user_mode(cr3: u64, entry: u64, stack_top: u64) -> ! {
    unsafe {
        asm!(
            "mov ds, ax",
            "mov es, ax",
            in("ax") USER_DATA_SELECTOR,
            options(nostack, preserves_flags)
        );

        asm!(
            "mov cr3, {cr3}",
            "push {user_data}",
            "push {stack_top}",
            "push {rflags}",
            "push {user_code}",
            "push {entry}",
            "iretq",
            cr3 = in(reg) cr3,
            user_data = in(reg) USER_DATA_SELECTOR as u64,
            stack_top = in(reg) stack_top,
            rflags = in(reg) 0x2u64,
            user_code = in(reg) USER_CODE_SELECTOR as u64,
            entry = in(reg) entry,
            options(noreturn)
        );
    }
}

pub unsafe fn switch_address_space(cr3: u64) {
    unsafe {
        asm!(
            "mov cr3, {cr3}",
            cr3 = in(reg) cr3,
            options(nostack, preserves_flags)
        );
    }
}
