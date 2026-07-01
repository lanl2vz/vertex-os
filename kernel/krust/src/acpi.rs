use core::{ptr, slice};

use crate::{limine, serial};

const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";
const RSDT_SIGNATURE: &[u8; 4] = b"RSDT";
const XSDT_SIGNATURE: &[u8; 4] = b"XSDT";
const FADT_SIGNATURE: &[u8; 4] = b"FACP";
const DSDT_SIGNATURE: &[u8; 4] = b"DSDT";
const HEADER_LEN: usize = 36;
const MAX_TABLE_LEN: usize = 1024 * 1024;
const SLP_EN: u16 = 1 << 13;
const SCI_EN: u16 = 1;
const SYSTEM_IO_SPACE: u8 = 1;

#[repr(C, packed)]
struct RsdpV1 {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

#[derive(Clone, Copy)]
struct FadtInfo {
    dsdt_physical: u64,
    pm1a_control: IoRegister,
    pm1b_control: Option<IoRegister>,
    smi_command: u32,
    acpi_enable: u8,
}

#[derive(Clone, Copy)]
struct IoRegister {
    port: u16,
    width_bytes: u8,
}

#[derive(Clone, Copy)]
struct S5SleepType {
    a: u16,
    b: u16,
}

pub fn poweroff() -> ! {
    match acpi_poweroff() {
        Ok(()) => {}
        Err(reason) => {
            serial::write_str("ACPI poweroff unavailable: ");
            serial::write_str(reason);
            serial::write_str("\n");
        }
    }
    halt_loop()
}

fn acpi_poweroff() -> Result<(), &'static str> {
    let rsdp = limine::rsdp_address().ok_or("missing RSDP")?;
    if rsdp.is_null() {
        return Err("null RSDP");
    }
    if !valid_rsdp_v1(rsdp) {
        return Err("invalid RSDP");
    }
    let rsdp_v1 = rsdp as *const RsdpV1;
    let revision = unsafe { ptr::addr_of!((*rsdp_v1).revision).read_unaligned() };
    let rsdt_address = unsafe { ptr::addr_of!((*rsdp_v1).rsdt_address).read_unaligned() } as u64;
    let xsdt_address = if revision >= 2 {
        read_rsdp_xsdt_address(rsdp)
    } else {
        0
    };

    let fadt = if xsdt_address != 0 {
        find_table(xsdt_address, XSDT_SIGNATURE, true, FADT_SIGNATURE)
            .or_else(|| find_table(rsdt_address, RSDT_SIGNATURE, false, FADT_SIGNATURE))
    } else {
        find_table(rsdt_address, RSDT_SIGNATURE, false, FADT_SIGNATURE)
    }
    .ok_or("missing FADT")?;

    let fadt_info = parse_fadt(fadt)?;
    let dsdt = acpi_table(fadt_info.dsdt_physical).ok_or("missing DSDT")?;
    if &dsdt[0..4] != DSDT_SIGNATURE || !valid_checksum(dsdt) {
        return Err("invalid DSDT");
    }
    let sleep_type = find_s5_sleep_type(dsdt).ok_or("missing _S5_")?;

    enable_acpi_if_needed(fadt_info);
    write_sleep_control(fadt_info.pm1a_control, sleep_type.a);
    if let Some(pm1b_control) = fadt_info.pm1b_control {
        write_sleep_control(pm1b_control, sleep_type.b);
    }
    serial::write_str("ACPI S5 poweroff requested\n");
    Ok(())
}

fn valid_rsdp_v1(rsdp: *const u8) -> bool {
    let bytes = unsafe { slice::from_raw_parts(rsdp, 20) };
    &bytes[0..8] == RSDP_SIGNATURE && checksum(bytes) == 0
}

fn read_rsdp_xsdt_address(rsdp: *const u8) -> u64 {
    let bytes = unsafe { slice::from_raw_parts(rsdp, 36) };
    let length = read_u32(bytes, 20) as usize;
    if length < 36 || checksum(&bytes[..36]) != 0 {
        return 0;
    }
    read_u64(bytes, 24)
}

fn find_table(
    root_physical: u64,
    root_signature: &[u8; 4],
    entries_are_64_bit: bool,
    target_signature: &[u8; 4],
) -> Option<&'static [u8]> {
    let root = acpi_table(root_physical)?;
    if &root[0..4] != root_signature || !valid_checksum(root) {
        return None;
    }
    let entry_width = if entries_are_64_bit { 8 } else { 4 };
    let mut offset = HEADER_LEN;
    while offset + entry_width <= root.len() {
        let table_physical = if entries_are_64_bit {
            read_u64(root, offset)
        } else {
            read_u32(root, offset) as u64
        };
        if let Some(table) = acpi_table(table_physical)
            && &table[0..4] == target_signature
            && valid_checksum(table)
        {
            return Some(table);
        }
        offset += entry_width;
    }
    None
}

fn acpi_table(physical: u64) -> Option<&'static [u8]> {
    let offset = limine::hhdm_offset()?;
    let virtual_address = physical.checked_add(offset)?;
    let header = unsafe { slice::from_raw_parts(virtual_address as *const u8, HEADER_LEN) };
    let length = read_u32(header, 4) as usize;
    if !(HEADER_LEN..=MAX_TABLE_LEN).contains(&length) {
        return None;
    }
    Some(unsafe { slice::from_raw_parts(virtual_address as *const u8, length) })
}

fn parse_fadt(table: &[u8]) -> Result<FadtInfo, &'static str> {
    if &table[0..4] != FADT_SIGNATURE {
        return Err("invalid FADT signature");
    }
    let dsdt_physical = preferred_dsdt_address(table).ok_or("missing DSDT address")?;
    let pm1a_control = preferred_pm1_control(table, 64, 172).ok_or("missing PM1a control")?;
    let pm1b_control = preferred_pm1_control(table, 68, 184);
    Ok(FadtInfo {
        dsdt_physical,
        pm1a_control,
        pm1b_control,
        smi_command: read_u32_if_present(table, 48),
        acpi_enable: read_u8_if_present(table, 52),
    })
}

fn preferred_dsdt_address(table: &[u8]) -> Option<u64> {
    if table.len() >= 148 {
        let x_dsdt = read_u64(table, 140);
        if x_dsdt != 0 {
            return Some(x_dsdt);
        }
    }
    let dsdt = read_u32_if_present(table, 40) as u64;
    (dsdt != 0).then_some(dsdt)
}

fn preferred_pm1_control(
    table: &[u8],
    legacy_offset: usize,
    gas_offset: usize,
) -> Option<IoRegister> {
    if table.len() >= gas_offset + 12
        && let Some(register) = parse_gas_io_register(table, gas_offset)
    {
        return Some(register);
    }
    let port = read_u32_if_present(table, legacy_offset);
    if port == 0 || port > u16::MAX as u32 {
        return None;
    }
    Some(IoRegister {
        port: port as u16,
        width_bytes: read_u8_if_present(table, 89).max(2),
    })
}

fn parse_gas_io_register(table: &[u8], offset: usize) -> Option<IoRegister> {
    if table[offset] != SYSTEM_IO_SPACE {
        return None;
    }
    let address = read_u64(table, offset + 4);
    if address == 0 || address > u16::MAX as u64 {
        return None;
    }
    let width_bits = table[offset + 1];
    Some(IoRegister {
        port: address as u16,
        width_bytes: width_bits.saturating_add(7) / 8,
    })
}

fn enable_acpi_if_needed(fadt: FadtInfo) {
    if read_control(fadt.pm1a_control) & SCI_EN != 0 {
        return;
    }
    if fadt.smi_command == 0 || fadt.smi_command > u16::MAX as u32 || fadt.acpi_enable == 0 {
        return;
    }
    unsafe {
        serial::outb_raw(fadt.smi_command as u16, fadt.acpi_enable);
    }
    let mut attempts = 0;
    while attempts < 100_000 {
        if read_control(fadt.pm1a_control) & SCI_EN != 0 {
            return;
        }
        core::hint::spin_loop();
        attempts += 1;
    }
}

fn write_sleep_control(register: IoRegister, sleep_type: u16) {
    let value = (sleep_type << 10) | SLP_EN;
    unsafe {
        if register.width_bytes >= 4 {
            serial::outl_raw(register.port, value as u32);
        } else {
            serial::outw_raw(register.port, value);
        }
    }
}

fn read_control(register: IoRegister) -> u16 {
    unsafe {
        if register.width_bytes >= 4 {
            serial::inl_raw(register.port) as u16
        } else {
            serial::inw_raw(register.port)
        }
    }
}

fn find_s5_sleep_type(dsdt: &[u8]) -> Option<S5SleepType> {
    let mut index = HEADER_LEN;
    while index + 5 < dsdt.len() {
        if &dsdt[index..index + 4] == b"_S5_" {
            let mut cursor = index + 4;
            if cursor >= dsdt.len() || dsdt[cursor] != 0x12 {
                index += 1;
                continue;
            }
            cursor += 1;
            let (_, consumed) = aml_pkg_length(&dsdt[cursor..])?;
            cursor += consumed;
            if cursor >= dsdt.len() {
                return None;
            }
            cursor += 1;
            let (a, consumed) = aml_integer(&dsdt[cursor..])?;
            cursor += consumed;
            let (b, _) = aml_integer(&dsdt[cursor..])?;
            return Some(S5SleepType { a, b });
        }
        index += 1;
    }
    None
}

fn aml_pkg_length(bytes: &[u8]) -> Option<(usize, usize)> {
    let lead = *bytes.first()?;
    let extra = (lead >> 6) as usize;
    if extra == 0 {
        return Some(((lead & 0x3f) as usize, 1));
    }
    if bytes.len() < extra + 1 {
        return None;
    }
    let mut length = (lead & 0x0f) as usize;
    let mut shift = 4;
    let mut index = 0;
    while index < extra {
        length |= (bytes[index + 1] as usize) << shift;
        shift += 8;
        index += 1;
    }
    Some((length, extra + 1))
}

fn aml_integer(bytes: &[u8]) -> Option<(u16, usize)> {
    match *bytes.first()? {
        0x00 => Some((0, 1)),
        0x01 => Some((1, 1)),
        0x0a if bytes.len() >= 2 => Some((bytes[1] as u16, 2)),
        0x0b if bytes.len() >= 3 => Some((read_u16(bytes, 1), 3)),
        0x0c if bytes.len() >= 5 => Some((read_u32(bytes, 1) as u16, 5)),
        value if value <= 0x0f => Some((value as u16, 1)),
        _ => None,
    }
}

fn valid_checksum(bytes: &[u8]) -> bool {
    checksum(bytes) == 0
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte))
}

fn read_u8_if_present(bytes: &[u8], offset: usize) -> u8 {
    bytes.get(offset).copied().unwrap_or(0)
}

fn read_u32_if_present(bytes: &[u8], offset: usize) -> u32 {
    if offset + 4 <= bytes.len() {
        read_u32(bytes, offset)
    } else {
        0
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

fn halt_loop() -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}
