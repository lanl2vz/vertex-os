use super::*;

pub(super) fn reap_process_context(pid: ProcessId) -> Result<(), IpcError> {
    let _ = cancel_blocked_receivers_for_endpoint_owner(pid, STATUS_BAD_CAPABILITY);
    let removed_endpoints = runtime().objects.remove_owned_endpoints(pid);
    if removed_endpoints > 0 {
        serial::write_str("Krust process owned endpoints reaped: pid=");
        serial::write_u64_dec(pid.raw());
        serial::write_str(" endpoints=");
        serial::write_u64_dec(removed_endpoints);
        serial::write_str("\n");
    }

    let (name, cr3, already_reaped) = {
        let runtime = runtime();
        let Some(process) = runtime.processes.process(pid) else {
            return Err(IpcError::BadCapability);
        };
        (process.name, process.context.cr3, process.context_reaped)
    };
    release_process_virtio_ownership(pid);
    release_process_dma_mappings(pid);
    let runtime = runtime();
    if let Some(process) = runtime.processes.process_mut(pid) {
        process.clear_file_handles();
    }
    runtime.release_process_file_descriptions(pid);
    let removed_dynamic_bind_mounts = runtime.remove_owned_dynamic_bind_mounts(pid);
    let removed_declared_bind_mounts = runtime.remove_owned_declared_bind_mounts(pid);
    if removed_dynamic_bind_mounts > 0 {
        serial::write_str("Krust process dynamic bind mounts reaped: proc=");
        serial::write_str(name);
        serial::write_str(" mounts=");
        serial::write_u64_dec(removed_dynamic_bind_mounts);
        serial::write_str("\n");
    }
    if removed_declared_bind_mounts > 0 {
        serial::write_str("Krust process declared mount snapshot reaped: proc=");
        serial::write_str(name);
        serial::write_str(" mounts=");
        serial::write_u64_dec(removed_declared_bind_mounts);
        serial::write_str("\n");
    }
    release_unreferenced_derived_vfs_roots(runtime);
    if already_reaped || cr3 == 0 {
        return Ok(());
    }

    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    let stats = paging::reclaim_user_address_space(hhdm_offset, cr3, frame_allocator()?)
        .map_err(|_| IpcError::BadCapability)?;
    if let Some(process) = runtime.processes.process_mut(pid) {
        process.context = ProcessContext {
            cr3: 0,
            entry: 0,
            stack_top: 0,
        };
        process.context_reaped = true;
    }

    serial::write_str("Krust process address space reaped: proc=");
    serial::write_str(name);
    serial::write_str(" pid=");
    serial::write_u64_dec(pid.raw());
    serial::write_str(" user_frames=");
    serial::write_u64_dec(stats.user_leaf_frames);
    serial::write_str(" page_tables=");
    serial::write_u64_dec(stats.page_table_frames);
    serial::write_str(" device_mappings=");
    serial::write_u64_dec(stats.device_mappings);
    serial::write_str("\n");
    Ok(())
}

pub(super) fn map_current_process_physical_range(
    virtual_base: u64,
    physical_base: u64,
    length: u64,
    flags: paging::PageFlags,
) -> Result<(), IpcError> {
    if length == 0
        || length % memory::FRAME_SIZE != 0
        || virtual_base % memory::FRAME_SIZE != 0
        || physical_base % memory::FRAME_SIZE != 0
    {
        return Err(IpcError::BadCapability);
    }
    let virtual_end = virtual_base
        .checked_add(length)
        .ok_or(IpcError::BadCapability)?;
    if virtual_base >= paging::USER_CANONICAL_LIMIT || virtual_end > paging::USER_CANONICAL_LIMIT {
        return Err(IpcError::BadCapability);
    }
    physical_base
        .checked_add(length)
        .ok_or(IpcError::BadCapability)?;

    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    let root_table_physical = runtime()
        .processes
        .current_process()
        .map(|process| process.context.cr3)
        .ok_or(IpcError::BadCapability)?;
    if !paging::user_range_is_unmapped(hhdm_offset, root_table_physical, virtual_base, length)
        .map_err(|_| IpcError::BadCapability)?
    {
        return Err(IpcError::BadCapability);
    }
    let allocator = frame_allocator()?;

    let mut offset = 0;
    let mut mapped_length = 0;
    while offset < length {
        let frame = memory::PhysicalFrame::from_start(
            physical_base
                .checked_add(offset)
                .ok_or(IpcError::BadCapability)?,
        )
        .ok_or(IpcError::BadCapability)?;
        let virtual_address = virtual_base
            .checked_add(offset)
            .ok_or(IpcError::BadCapability)?;
        let next_offset = offset
            .checked_add(memory::FRAME_SIZE)
            .ok_or(IpcError::BadCapability)?;
        match paging::map_page_in_root(
            hhdm_offset,
            root_table_physical,
            virtual_address,
            frame,
            flags,
            allocator,
        ) {
            Ok(()) => {}
            Err(_) => {
                rollback_current_process_physical_range(
                    hhdm_offset,
                    root_table_physical,
                    virtual_base,
                    mapped_length,
                    allocator,
                );
                return Err(IpcError::BadCapability);
            }
        }
        mapped_length = next_offset;
        offset = next_offset;
    }

    Ok(())
}

fn rollback_current_process_physical_range(
    hhdm_offset: u64,
    root_table_physical: u64,
    virtual_base: u64,
    length: u64,
    allocator: &mut memory::FrameAllocator,
) {
    let mut offset = 0;
    while offset < length {
        if let Some(virtual_address) = virtual_base.checked_add(offset) {
            let _ = paging::unmap_page_in_root(hhdm_offset, root_table_physical, virtual_address);
        }
        let Some(next_offset) = offset.checked_add(memory::FRAME_SIZE) else {
            return;
        };
        offset = next_offset;
    }

    offset = 0;
    while offset < length {
        if let Some(virtual_address) = virtual_base.checked_add(offset) {
            let _ = paging::prune_empty_user_page_tables(
                hhdm_offset,
                root_table_physical,
                virtual_address,
                allocator,
            );
        }
        let Some(next_offset) = offset.checked_add(memory::FRAME_SIZE) else {
            return;
        };
        offset = next_offset;
    }
}

pub(super) fn unmap_current_process_physical_range(
    virtual_base: u64,
    length: u64,
) -> Result<(), IpcError> {
    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    let root_table_physical = runtime()
        .processes
        .current_process()
        .map(|process| process.context.cr3)
        .ok_or(IpcError::BadCapability)?;
    let allocator = frame_allocator()?;
    rollback_current_process_physical_range(
        hhdm_offset,
        root_table_physical,
        virtual_base,
        length,
        allocator,
    );
    Ok(())
}

pub(super) fn device_user_mapping_base(
    window_base: u64,
    object: KernelObjectId,
    length: u64,
) -> Result<u64, IpcError> {
    if length == 0 || length > USER_DEVICE_MAPPING_STRIDE {
        return Err(IpcError::BadCapability);
    }

    let offset = object
        .raw()
        .checked_mul(USER_DEVICE_MAPPING_STRIDE)
        .ok_or(IpcError::BadCapability)?;
    let base = window_base
        .checked_add(offset)
        .ok_or(IpcError::BadCapability)?;
    let end = base.checked_add(length).ok_or(IpcError::BadCapability)?;
    if base >= paging::USER_CANONICAL_LIMIT || end > paging::USER_CANONICAL_LIMIT {
        return Err(IpcError::BadCapability);
    }
    Ok(base)
}

pub(super) fn align_up(value: u64, align: u64) -> Option<u64> {
    Some(value.checked_add(align - 1)? & !(align - 1))
}

pub(super) fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        buffer[offset + index] = bytes[index];
        index += 1;
    }
}

pub(super) fn write_dma_mapping_info(
    buffer: &mut [u8; DMA_MAPPING_INFO_BYTES],
    mapping: DmaUserMapping,
) {
    write_u64(buffer, 0, mapping.virtual_base);
    write_u64(buffer, 8, mapping.physical_base);
    write_u64(buffer, 16, mapping.length);
}

pub(super) fn ranges_overlap(
    left_start: u64,
    left_len: u64,
    right_start: u64,
    right_len: u64,
) -> bool {
    if left_len == 0 || right_len == 0 {
        return false;
    }
    let left_end = left_start.saturating_add(left_len);
    let right_end = right_start.saturating_add(right_len);
    left_start < right_end && right_start < left_end
}
