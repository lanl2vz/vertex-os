use super::vfs_transactions::{block_current_on_network_port, wake_blocked_network_receiver};
use super::*;

pub fn virtio_device_probe(cap_slot: u64) -> Result<(), IpcError> {
    let device = virtio_device_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    if device.transport != VIRTIO_PCI_IO_TRANSPORT_ID {
        return Err(IpcError::BadCapability);
    }
    record_virtio_device_owner(device.id)?;
    serial::write_str("Virtio device probe accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" virtio-device=");
    serial::write_str(device.name);
    serial::write_str(" transport=");
    serial::write_str(device.transport);
    serial::write_str("\n");
    Ok(())
}

pub fn virtio_device_report(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len != VIRTIO_DRIVER_REPORT_BYTES {
        return Err(IpcError::InvalidUserBuffer);
    }
    let device = virtio_device_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    if device.transport != VIRTIO_PCI_IO_TRANSPORT_ID {
        return Err(IpcError::BadCapability);
    }

    let mut bytes = [0u8; VIRTIO_DRIVER_REPORT_BYTES];
    usercopy::copy_from_user(&mut bytes, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let queue_size = read_report_u64(&bytes, 0);
    if queue_size > VIRTIO_QUEUE_MAX_SIZE as u64 {
        return Err(IpcError::BadCapability);
    }
    let avail_idx = read_report_u64(&bytes, 8);
    let used_idx = read_report_u64(&bytes, 16);
    if avail_idx > u16::MAX as u64 || used_idx > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }
    let last_error = virtio_error_label(read_report_u64(&bytes, 56))?;

    let process = current_process_id();
    let process_name = current_process_name();
    let runtime = runtime();
    let Some(device) = runtime.objects.get_virtio_device_mut(device.id) else {
        return Err(IpcError::BadCapability);
    };
    if device.owner != ProcessId::empty() && device.owner != process {
        return Err(IpcError::BadCapability);
    }
    device.owner = process;
    device.queue_size = queue_size as u16;
    device.avail_idx = avail_idx as u16;
    device.used_idx = used_idx as u16;
    device.submissions = read_report_u64(&bytes, 24);
    device.completions = read_report_u64(&bytes, 32);
    device.timeouts = read_report_u64(&bytes, 40);
    device.reset_count = read_report_u64(&bytes, 48);
    device.last_error = last_error;

    serial::write_str("Virtio driver report accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" virtio-device=");
    serial::write_str(device.name);
    serial::write_str(" submissions=");
    serial::write_u64_dec(device.submissions);
    serial::write_str(" completions=");
    serial::write_u64_dec(device.completions);
    serial::write_str(" timeouts=");
    serial::write_u64_dec(device.timeouts);
    serial::write_str(" resets=");
    serial::write_u64_dec(device.reset_count);
    serial::write_str(" last_error=");
    serial::write_str(device.last_error);
    serial::write_str("\n");
    Ok(())
}

fn record_virtio_device_owner(device_id: KernelObjectId) -> Result<(), IpcError> {
    let process = current_process_id();
    let runtime = runtime();
    let Some(device) = runtime.objects.get_virtio_device_mut(device_id) else {
        return Err(IpcError::BadCapability);
    };
    if device.owner != ProcessId::empty() && device.owner != process {
        return Err(IpcError::BadCapability);
    }
    device.owner = process;
    Ok(())
}

fn virtio_error_label(code: u64) -> Result<&'static str, IpcError> {
    match code {
        VIRTIO_ERROR_NONE => Ok("none"),
        VIRTIO_ERROR_COMPLETION_TIMEOUT => Ok("completion-timeout"),
        VIRTIO_ERROR_RESET_FAILED => Ok("reset-failed"),
        VIRTIO_ERROR_INIT_FAILED => Ok("init-failed"),
        VIRTIO_ERROR_STATUS => Ok("status-error"),
        _ => Err(IpcError::BadCapability),
    }
}

fn read_report_u64(bytes: &[u8; VIRTIO_DRIVER_REPORT_BYTES], offset: usize) -> u64 {
    let mut value = 0u64;
    let mut index = 0;
    while index < 8 {
        value |= (bytes[offset + index] as u64) << (index * 8);
        index += 1;
    }
    value
}

pub fn virtio_rng_read(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
) -> Result<usize, IpcError> {
    let device = virtio_device_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    if !virtio_device_is(device, VIRTIO_RNG_DEVICE_ID) {
        return Err(IpcError::BadCapability);
    }
    let copy_len = min(max_len, 32);
    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        copy_len,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;
    if copy_len == 0 {
        return Ok(0);
    }
    let mut bytes = [0u8; 32];
    let actual_len = virtio_rng_fill(&mut bytes[..copy_len])?;
    usercopy::copy_to_user(UserPtr::new(destination as u64), &bytes[..actual_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Virtio RNG read accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" virtio-device=");
    serial::write_str(device.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(actual_len as u64);
    serial::write_str("\n");
    Ok(actual_len)
}

pub fn virtio_net_tx(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }
    let device = virtio_device_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    if !virtio_device_is(device, VIRTIO_NET_DEVICE_ID) {
        return Err(IpcError::BadCapability);
    }
    let mut frame = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut frame, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    virtio_net_send_frame(&frame[..len])?;

    serial::write_str("Virtio net TX completed: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" virtio-device=");
    serial::write_str(device.name);
    serial::write_str(" frame-bytes=");
    serial::write_u64_dec(len as u64);
    serial::write_str("\n");
    Ok(())
}

pub fn virtio_net_rx(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
) -> Result<usize, IpcError> {
    let device = virtio_device_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    if !virtio_device_is(device, VIRTIO_NET_DEVICE_ID) {
        return Err(IpcError::BadCapability);
    }
    if max_len < MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }
    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        MAX_MESSAGE_BYTES,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;
    let mut frame = [0u8; MAX_MESSAGE_BYTES];
    let frame_len = virtio_net_receive_frame(&mut frame)?;
    usercopy::copy_to_user(UserPtr::new(destination as u64), &frame[..frame_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Virtio net RX completed: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" virtio-device=");
    serial::write_str(device.name);
    serial::write_str(" frame-bytes=");
    serial::write_u64_dec(frame_len as u64);
    serial::write_str("\n");
    Ok(frame_len)
}

pub fn network_send_udp(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }
    let port = network_port_from_cap(cap_slot, capability::RIGHT_BIND | capability::RIGHT_LISTEN)?;
    let mut payload = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut payload, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let sender = runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
        .ok_or(IpcError::BadCapability)?;
    {
        let runtime = runtime();
        let Some(port) = runtime.objects.get_network_port_mut(port.id) else {
            return Err(IpcError::BadCapability);
        };
        port.enqueue_udp(sender, &payload, len)?;
    }

    serial::write_str("UDP send queued for netstack: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" network-port=");
    serial::write_str(port.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(len as u64);
    serial::write_str("\n");
    serial::write_str("network-port bind/listen rights enforced by netstack boundary\n");
    wake_blocked_network_receiver(port.id);
    Ok(())
}

pub fn network_recv_udp(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let port = network_port_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        max_len,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;

    let message = {
        let runtime = runtime();
        let Some(port) = runtime.objects.get_network_port_mut(port.id) else {
            return Err(IpcError::BadCapability);
        };
        port.dequeue_udp()
    };

    let Some(message) = message else {
        if block_current_on_network_port(port.id, destination as u64, max_len, frame) {
            return Ok(());
        }
        return Err(IpcError::Empty);
    };

    let copy_len = min(message.len, max_len);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &message.bytes[..copy_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Network-port UDP request delivered to netstack: network-port=");
    serial::write_str(port.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");
    frame.rax = copy_len as u64;
    Ok(())
}

fn virtio_rng_fill(destination: &mut [u8]) -> Result<usize, IpcError> {
    if destination.is_empty() {
        return Ok(0);
    }

    let state = virtio_rng_state()?;
    let data = queue_data_virtual(&state.queue);
    zero_dma(data, destination.len());
    let used_len = match virtio_submit_single(
        state.io_base,
        0,
        &mut state.queue,
        destination.len() as u32,
        true,
    ) {
        Ok(used_len) => used_len,
        Err(error) => {
            if error == IpcError::Empty {
                reset_virtio_rng_state(state, "rng-timeout");
            }
            return Err(error);
        }
    };
    let actual_len = min(destination.len(), used_len as usize);
    read_dma_bytes(data, &mut destination[..actual_len]);
    Ok(actual_len)
}

fn virtio_net_send_frame(frame: &[u8]) -> Result<(), IpcError> {
    let state = virtio_net_state()?;
    virtio_net_send_frame_locked(state, frame)
}

fn virtio_net_receive_frame(destination: &mut [u8]) -> Result<usize, IpcError> {
    let state = virtio_net_state()?;
    if !state.rx_posted {
        virtio_net_post_rx_buffer(state)?;
    }

    serial::write_str("virtio-net RX waits for interrupt-backed completion\n");
    let used_len = match virtio_wait_used(state.io_base, &mut state.rx) {
        Ok(used_len) => used_len,
        Err(error) => {
            if error == IpcError::Empty {
                reset_virtio_net_state(state, "net-rx-timeout");
            }
            return Err(error);
        }
    };
    state.rx_posted = false;
    if used_len as usize <= VIRTIO_NET_HDR_LEN {
        virtio_net_post_rx_buffer(state)?;
        return Err(IpcError::BadCapability);
    }

    let frame_len = (used_len as usize) - VIRTIO_NET_HDR_LEN;
    if frame_len > destination.len() {
        virtio_net_post_rx_buffer(state)?;
        return Err(IpcError::MessageTooLarge);
    }

    read_dma_bytes(
        queue_data_virtual(&state.rx) + VIRTIO_NET_HDR_LEN as u64,
        &mut destination[..frame_len],
    );
    virtio_net_post_rx_buffer(state)?;
    Ok(frame_len)
}

fn virtio_device_is(device: VirtioDeviceObject, expected_name: &str) -> bool {
    device.name == expected_name && device.transport == VIRTIO_PCI_IO_TRANSPORT_ID
}

fn virtio_rng_state() -> Result<&'static mut VirtioRngState, IpcError> {
    let state = unsafe { &mut *VIRTIO_RNG_STATE.0.get() };
    if !state.initialized {
        init_virtio_rng(state)?;
    }
    Ok(state)
}

fn virtio_net_state() -> Result<&'static mut VirtioNetState, IpcError> {
    let state = unsafe { &mut *VIRTIO_NET_STATE.0.get() };
    if !state.initialized {
        init_virtio_net(state)?;
    }
    Ok(state)
}

fn init_virtio_rng(state: &mut VirtioRngState) -> Result<(), IpcError> {
    let io_base = discover_virtio_pci_io_device(PCI_DEVICE_VIRTIO_RNG_IO_TRANSPORT)?;
    let (dma_physical, dma_virtual) = if state.queue.dma_physical == 0 {
        allocate_virtio_dma(VIRTIO_RNG_DMA_FRAMES)?
    } else {
        (state.queue.dma_physical, state.queue.dma_virtual)
    };
    let mut queue = VirtioQueueState::new(dma_physical, dma_virtual);
    let reset_count = state.reset_count;
    let last_error = state.last_error;

    virtio_write8(io_base, VIRTIO_PCI_STATUS, 0);
    virtio_write8(io_base, VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE);
    virtio_write8(
        io_base,
        VIRTIO_PCI_STATUS,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER,
    );
    let _features = virtio_read32(io_base, VIRTIO_PCI_HOST_FEATURES);
    virtio_write32(io_base, VIRTIO_PCI_GUEST_FEATURES, 0);
    if virtio_setup_queue(io_base, 0, &mut queue).is_err() {
        virtio_write8(io_base, VIRTIO_PCI_STATUS, VIRTIO_STATUS_FAILED);
        return Err(IpcError::BadCapability);
    }
    virtio_write8(
        io_base,
        VIRTIO_PCI_STATUS,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK,
    );

    *state = VirtioRngState {
        initialized: true,
        io_base,
        queue,
        owner: current_process_id(),
        reset_count,
        last_error,
    };
    serial::write_str("Virtio RNG PCI queue initialized\n");
    Ok(())
}

fn init_virtio_net(state: &mut VirtioNetState) -> Result<(), IpcError> {
    let io_base = discover_virtio_pci_io_device(PCI_DEVICE_VIRTIO_NET_IO_TRANSPORT)?;
    let (dma_physical, dma_virtual) = if state.rx.dma_physical == 0 {
        allocate_virtio_dma(VIRTIO_NET_DMA_FRAMES)?
    } else {
        (state.rx.dma_physical, state.rx.dma_virtual)
    };
    let mut rx = VirtioQueueState::new(dma_physical, dma_virtual);
    let mut tx = VirtioQueueState::new(
        dma_physical + VIRTIO_QUEUE_STRIDE as u64,
        dma_virtual + VIRTIO_QUEUE_STRIDE as u64,
    );
    let reset_count = state.reset_count;
    let last_error = state.last_error;

    virtio_write8(io_base, VIRTIO_PCI_STATUS, 0);
    virtio_write8(io_base, VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE);
    virtio_write8(
        io_base,
        VIRTIO_PCI_STATUS,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER,
    );
    let _features = virtio_read32(io_base, VIRTIO_PCI_HOST_FEATURES);
    virtio_write32(io_base, VIRTIO_PCI_GUEST_FEATURES, 0);
    if virtio_setup_queue(io_base, 0, &mut rx).is_err()
        || virtio_setup_queue(io_base, 1, &mut tx).is_err()
    {
        virtio_write8(io_base, VIRTIO_PCI_STATUS, VIRTIO_STATUS_FAILED);
        return Err(IpcError::BadCapability);
    }
    virtio_write8(
        io_base,
        VIRTIO_PCI_STATUS,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK,
    );

    *state = VirtioNetState {
        initialized: true,
        io_base,
        rx,
        tx,
        rx_posted: false,
        owner: current_process_id(),
        reset_count,
        last_error,
    };
    virtio_net_post_rx_buffer(state)?;
    serial::write_str("Virtio net PCI queues initialized\n");
    Ok(())
}

fn reset_virtio_rng_state(state: &mut VirtioRngState, reason: &'static str) {
    if state.io_base != 0 {
        virtio_write8(state.io_base, VIRTIO_PCI_STATUS, 0);
    }
    state.initialized = false;
    state.reset_count = state.reset_count.saturating_add(1);
    state.last_error = reason;
    state.queue.last_error = reason;
    serial::write_str("Virtio RNG reset after error: reason=");
    serial::write_str(reason);
    serial::write_str(" resets=");
    serial::write_u64_dec(state.reset_count);
    serial::write_str("\n");
}

fn reset_virtio_net_state(state: &mut VirtioNetState, reason: &'static str) {
    if state.io_base != 0 {
        virtio_write8(state.io_base, VIRTIO_PCI_STATUS, 0);
    }
    state.initialized = false;
    state.rx_posted = false;
    state.reset_count = state.reset_count.saturating_add(1);
    state.last_error = reason;
    state.rx.last_error = reason;
    state.tx.last_error = reason;
    serial::write_str("Virtio net reset after error: reason=");
    serial::write_str(reason);
    serial::write_str(" resets=");
    serial::write_u64_dec(state.reset_count);
    serial::write_str("\n");
}

fn virtio_setup_queue(
    io_base: u16,
    queue_index: u16,
    queue: &mut VirtioQueueState,
) -> Result<(), IpcError> {
    virtio_write16(io_base, VIRTIO_PCI_QUEUE_SEL, queue_index);
    let queue_size = virtio_read16(io_base, VIRTIO_PCI_QUEUE_NUM);
    if !(VIRTIO_QUEUE_MIN_SIZE..=VIRTIO_QUEUE_MAX_SIZE).contains(&queue_size) {
        return Err(IpcError::BadCapability);
    }
    let avail_offset = VIRTIO_QUEUE_DESC_OFFSET + queue_size as usize * 16;
    let used_offset = align_up_usize(
        avail_offset + 6 + queue_size as usize * 2,
        VIRTIO_QUEUE_RING_ALIGN,
    );
    let data_offset = align_up_usize(
        used_offset + 6 + queue_size as usize * 8,
        VIRTIO_QUEUE_RING_ALIGN,
    );
    if data_offset >= VIRTIO_QUEUE_STRIDE {
        return Err(IpcError::BadCapability);
    }
    queue.queue_size = queue_size;
    queue.avail_offset = avail_offset;
    queue.used_offset = used_offset;
    queue.data_offset = data_offset;
    queue.avail_idx = 0;
    queue.used_idx = 0;

    zero_dma(queue.dma_virtual, VIRTIO_QUEUE_STRIDE);
    write_dma_u16(
        queue.dma_virtual + queue.avail_offset as u64,
        VIRTIO_AVAIL_F_NO_INTERRUPT,
    );
    virtio_write32(
        io_base,
        VIRTIO_PCI_QUEUE_PFN,
        (queue.dma_physical >> 12) as u32,
    );
    Ok(())
}

fn virtio_net_post_rx_buffer(state: &mut VirtioNetState) -> Result<(), IpcError> {
    zero_dma(queue_data_virtual(&state.rx), VIRTIO_NET_RX_BUFFER_LEN);
    virtio_post_single(
        state.io_base,
        0,
        &mut state.rx,
        VIRTIO_NET_RX_BUFFER_LEN as u32,
        true,
    )?;
    state.rx_posted = true;
    Ok(())
}

fn virtio_net_send_frame_locked(state: &mut VirtioNetState, frame: &[u8]) -> Result<(), IpcError> {
    if frame.len() > MAX_MESSAGE_BYTES + UDP_IPV4_HEADER_LEN {
        return Err(IpcError::MessageTooLarge);
    }
    let payload_len = if frame.len() < ETHERNET_MIN_FRAME_LEN {
        ETHERNET_MIN_FRAME_LEN
    } else {
        frame.len()
    };
    let total_len = payload_len + VIRTIO_NET_HDR_LEN;
    if total_len > VIRTIO_NET_RX_BUFFER_LEN {
        return Err(IpcError::MessageTooLarge);
    }

    let data = queue_data_virtual(&state.tx);
    let data_physical = queue_data_physical(&state.tx);
    zero_dma(data, total_len);
    write_dma_bytes(data + VIRTIO_NET_HDR_LEN as u64, frame);
    write_virtio_desc(
        &state.tx,
        0,
        data_physical,
        VIRTIO_NET_HDR_LEN as u32,
        VIRTIO_DESC_F_NEXT,
        1,
    );
    write_virtio_desc(
        &state.tx,
        1,
        data_physical + VIRTIO_NET_HDR_LEN as u64,
        payload_len as u32,
        0,
        0,
    );
    virtio_kick_queue_head(state.io_base, 1, &mut state.tx, 0);
    if let Err(error) = virtio_wait_used(state.io_base, &mut state.tx) {
        if error == IpcError::Empty {
            reset_virtio_net_state(state, "net-tx-timeout");
        }
        return Err(error);
    }
    Ok(())
}

fn virtio_submit_single(
    io_base: u16,
    queue_index: u16,
    queue: &mut VirtioQueueState,
    data_len: u32,
    writable: bool,
) -> Result<u32, IpcError> {
    virtio_post_single(io_base, queue_index, queue, data_len, writable)?;
    virtio_wait_used(io_base, queue)
}

fn virtio_post_single(
    io_base: u16,
    queue_index: u16,
    queue: &mut VirtioQueueState,
    data_len: u32,
    writable: bool,
) -> Result<(), IpcError> {
    let flags = if writable { VIRTIO_DESC_F_WRITE } else { 0 };
    write_virtio_desc(queue, 0, queue_data_physical(queue), data_len, flags, 0);
    virtio_kick_queue_head(io_base, queue_index, queue, 0);
    Ok(())
}

fn virtio_kick_queue_head(io_base: u16, queue_index: u16, queue: &mut VirtioQueueState, head: u16) {
    let ring_offset = queue.avail_offset + 4 + ((queue.avail_idx % queue.queue_size) as usize * 2);
    write_dma_u16(queue.dma_virtual + ring_offset as u64, head);
    queue.avail_idx = queue.avail_idx.wrapping_add(1);
    queue.submissions = queue.submissions.saturating_add(1);
    compiler_fence(Ordering::SeqCst);
    write_dma_u16(
        queue.dma_virtual + queue.avail_offset as u64 + 2,
        queue.avail_idx,
    );
    compiler_fence(Ordering::SeqCst);
    virtio_write16(io_base, VIRTIO_PCI_QUEUE_NOTIFY, queue_index);
}

fn virtio_wait_used(io_base: u16, queue: &mut VirtioQueueState) -> Result<u32, IpcError> {
    let target_used = queue.used_idx.wrapping_add(1);
    let mut spins = 0u64;
    while read_dma_u16(queue.dma_virtual + queue.used_offset as u64 + 2) != target_used {
        spins += 1;
        if spins > VIRTIO_POLL_SPINS {
            queue.timeouts = queue.timeouts.saturating_add(1);
            queue.last_error = "completion-timeout";
            return Err(IpcError::Empty);
        }
        if spins & 0xffff == 0 {
            queue.interrupt_waits = queue.interrupt_waits.saturating_add(1);
            timer::wait_for_interrupt();
        } else if spins & 0xfff == 0 {
            pause_cpu();
        }
    }
    compiler_fence(Ordering::SeqCst);
    let used_offset = queue.used_offset + 4 + ((queue.used_idx % queue.queue_size) as usize * 8);
    let used_len = read_dma_u32(queue.dma_virtual + used_offset as u64 + 4);
    queue.used_idx = target_used;
    queue.completions = queue.completions.saturating_add(1);
    let _isr = virtio_read8(io_base, VIRTIO_PCI_ISR);
    Ok(used_len)
}

fn queue_data_virtual(queue: &VirtioQueueState) -> u64 {
    queue.dma_virtual + queue.data_offset as u64
}

fn queue_data_physical(queue: &VirtioQueueState) -> u64 {
    queue.dma_physical + queue.data_offset as u64
}

fn align_up_usize(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

fn write_virtio_desc(
    queue: &VirtioQueueState,
    index: usize,
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
) {
    let offset = VIRTIO_QUEUE_DESC_OFFSET + index * 16;
    write_dma_u64(queue.dma_virtual + offset as u64, addr);
    write_dma_u32(queue.dma_virtual + offset as u64 + 8, len);
    write_dma_u16(queue.dma_virtual + offset as u64 + 12, flags);
    write_dma_u16(queue.dma_virtual + offset as u64 + 14, next);
}

fn allocate_virtio_dma(frame_count: u64) -> Result<(u64, u64), IpcError> {
    let frame = frame_allocator()?
        .allocate_contiguous_owned(frame_count, memory::FrameOwner::dma(frame_count))
        .ok_or(IpcError::BadCapability)?;
    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    let bytes = frame_count
        .checked_mul(memory::FRAME_SIZE)
        .ok_or(IpcError::BadCapability)? as usize;
    let virtual_base = hhdm_offset + frame.start();
    zero_dma(virtual_base, bytes);
    Ok((frame.start(), virtual_base))
}

fn discover_virtio_pci_io_device(device_id: u16) -> Result<u16, IpcError> {
    let mut slot = 0u8;
    while slot < 32 {
        let vendor = pci_read_u16(0, slot, 0, 0x00);
        let device = pci_read_u16(0, slot, 0, 0x02);
        if vendor == PCI_VENDOR_VIRTIO && device == device_id {
            let command = pci_read_u16(0, slot, 0, PCI_COMMAND);
            pci_write_u16(
                0,
                slot,
                0,
                PCI_COMMAND,
                (command | PCI_COMMAND_IO | PCI_COMMAND_BUS_MASTER)
                    & !PCI_COMMAND_INTERRUPT_DISABLE,
            );
            let bar0 = pci_read_u32(0, slot, 0, PCI_BAR0);
            if bar0 & 1 == 0 {
                return Err(IpcError::BadCapability);
            }
            return Ok((bar0 & !0x3) as u16);
        }
        slot += 1;
    }
    Err(IpcError::BadCapability)
}

fn pci_address(bus: u8, slot: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xfc)
}

fn pci_select(bus: u8, slot: u8, function: u8, offset: u8) -> u16 {
    unsafe {
        serial::outl_raw(PCI_CONFIG_ADDRESS, pci_address(bus, slot, function, offset));
    }
    PCI_CONFIG_DATA + ((offset as u16) & 0x3)
}

fn pci_read_u16(bus: u8, slot: u8, function: u8, offset: u8) -> u16 {
    let port = pci_select(bus, slot, function, offset);
    unsafe { serial::inw_raw(port) }
}

fn pci_read_u32(bus: u8, slot: u8, function: u8, offset: u8) -> u32 {
    let port = pci_select(bus, slot, function, offset);
    unsafe { serial::inl_raw(port) }
}

fn pci_write_u16(bus: u8, slot: u8, function: u8, offset: u8, value: u16) {
    let port = pci_select(bus, slot, function, offset);
    unsafe {
        serial::outw_raw(port, value);
    }
}

fn virtio_read8(io_base: u16, offset: u16) -> u8 {
    unsafe { serial::inb_raw(io_base + offset) }
}

fn virtio_read16(io_base: u16, offset: u16) -> u16 {
    unsafe { serial::inw_raw(io_base + offset) }
}

fn virtio_read32(io_base: u16, offset: u16) -> u32 {
    unsafe { serial::inl_raw(io_base + offset) }
}

fn virtio_write8(io_base: u16, offset: u16, value: u8) {
    unsafe {
        serial::outb_raw(io_base + offset, value);
    }
}

fn virtio_write16(io_base: u16, offset: u16, value: u16) {
    unsafe {
        serial::outw_raw(io_base + offset, value);
    }
}

fn virtio_write32(io_base: u16, offset: u16, value: u32) {
    unsafe {
        serial::outl_raw(io_base + offset, value);
    }
}

fn zero_dma(base: u64, len: usize) {
    let mut index = 0;
    while index < len {
        write_dma_u8(base + index as u64, 0);
        index += 1;
    }
}

fn write_dma_bytes(base: u64, value: &[u8]) {
    let mut index = 0;
    while index < value.len() {
        write_dma_u8(base + index as u64, value[index]);
        index += 1;
    }
}

fn read_dma_bytes(base: u64, out: &mut [u8]) {
    let mut index = 0;
    while index < out.len() {
        out[index] = read_dma_u8(base + index as u64);
        index += 1;
    }
}

fn write_dma_u8(address: u64, value: u8) {
    unsafe {
        (address as *mut u8).write_volatile(value);
    }
}

fn read_dma_u8(address: u64) -> u8 {
    unsafe { (address as *const u8).read_volatile() }
}

fn write_dma_u16(address: u64, value: u16) {
    write_dma_bytes(address, &value.to_le_bytes());
}

fn read_dma_u16(address: u64) -> u16 {
    u16::from_le_bytes([read_dma_u8(address), read_dma_u8(address + 1)])
}

fn write_dma_u32(address: u64, value: u32) {
    write_dma_bytes(address, &value.to_le_bytes());
}

fn read_dma_u32(address: u64) -> u32 {
    u32::from_le_bytes([
        read_dma_u8(address),
        read_dma_u8(address + 1),
        read_dma_u8(address + 2),
        read_dma_u8(address + 3),
    ])
}

fn write_dma_u64(address: u64, value: u64) {
    write_dma_bytes(address, &value.to_le_bytes());
}

fn pause_cpu() {
    unsafe {
        asm!("pause", options(nomem, nostack, preserves_flags));
    }
}

pub fn io_read(cap_slot: u64, port: u64) -> Result<u64, IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_READ)?;
    if !port_span_in_range(range, port, 1) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    let value = unsafe { serial::inb_raw(port as u16) };
    Ok(value as u64)
}

pub fn io_write(cap_slot: u64, port: u64, value: u64) -> Result<(), IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_WRITE)?;
    if !port_span_in_range(range, port, 1) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    unsafe {
        serial::outb_raw(port as u16, value as u8);
    }
    Ok(())
}

pub fn io_read16(cap_slot: u64, port: u64) -> Result<u64, IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_READ)?;
    if !port_span_in_range(range, port, 2) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    let value = unsafe { serial::inw_raw(port as u16) };
    Ok(value as u64)
}

pub fn io_write16(cap_slot: u64, port: u64, value: u64) -> Result<(), IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_WRITE)?;
    if !port_span_in_range(range, port, 2) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    unsafe {
        serial::outw_raw(port as u16, value as u16);
    }
    Ok(())
}

pub fn io_read32(cap_slot: u64, port: u64) -> Result<u64, IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_READ)?;
    if !port_span_in_range(range, port, 4) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    let value = unsafe { serial::inl_raw(port as u16) };
    Ok(value as u64)
}

pub fn io_write32(cap_slot: u64, port: u64, value: u64) -> Result<(), IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_WRITE)?;
    if !port_span_in_range(range, port, 4) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    unsafe {
        serial::outl_raw(port as u16, value as u32);
    }
    Ok(())
}

pub fn irq_wait(cap_slot: u64, timeout_ms: u64, frame: &mut SyscallFrame) -> Result<(), IpcError> {
    let line = interrupt_line_from_cap(cap_slot, capability::RIGHT_LISTEN)?;
    if consume_pending_interrupt(line.id) {
        serial::write_str("IRQ wait delivered pending: proc=");
        serial::write_str(current_process_name());
        serial::write_str(" interrupt-line=");
        serial::write_str(line.name);
        serial::write_str(" line=");
        serial::write_u64_dec(line.line);
        serial::write_str("\n");
        frame.rax = STATUS_OK;
        return Ok(());
    }

    serial::write_str("IRQ wait accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" interrupt-line=");
    serial::write_str(line.name);
    serial::write_str(" line=");
    serial::write_u64_dec(line.line);
    serial::write_str("\n");

    if timeout_ms == 0 {
        frame.rax = STATUS_OK;
        return Ok(());
    }

    let timeout_tsc = Some(deadline_after_ms(timeout_ms));
    if block_current_on_interrupt(line.id, timeout_tsc, frame) {
        return Ok(());
    }

    Err(IpcError::Empty)
}

fn consume_pending_interrupt(interrupt: KernelObjectId) -> bool {
    let runtime = runtime();
    let Some(line) = runtime.objects.get_interrupt_line_mut(interrupt) else {
        return false;
    };
    if line.pending_count == 0 {
        return false;
    }
    line.pending_count -= 1;
    line.delivered_count = line.delivered_count.saturating_add(1);
    true
}

fn block_current_on_interrupt(
    interrupt: KernelObjectId,
    timeout_tsc: Option<u64>,
    frame: &mut SyscallFrame,
) -> bool {
    let (name, line_name, line_number) = {
        let runtime = runtime();
        let Some(line) = runtime.objects.get_interrupt_line(interrupt) else {
            return false;
        };
        let Some(process) = runtime.processes.current_process_mut() else {
            return false;
        };

        process.saved_frame = *frame;
        process.saved_frame.rax = STATUS_OK;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnInterrupt {
            interrupt,
            timeout_tsc,
        };
        (process.name, line.name, line.line)
    };

    serial::write_str("IRQ wait blocked: proc=");
    serial::write_str(name);
    serial::write_str(" interrupt-line=");
    serial::write_str(line_name);
    serial::write_str(" line=");
    serial::write_u64_dec(line_number);
    if timeout_tsc.is_some() {
        serial::write_str(" timeout=yes");
    }
    serial::write_str("\n");

    if schedule_next_ready(frame) {
        return true;
    }

    let runtime = runtime();
    if let Some(process) = runtime.processes.current_process_mut() {
        process.state = ProcessState::Running;
    }

    serial::write_str("Scheduler blocked: proc=");
    serial::write_str(name);
    serial::write_str(" no ready process\n");
    false
}

pub fn record_hardware_irq(irq_line: u64) {
    let Some(line) = runtime().objects.get_interrupt_line_by_number(irq_line) else {
        serial::write_str("Spurious legacy IRQ: line=");
        serial::write_u64_dec(irq_line);
        serial::write_str("\n");
        return;
    };

    if let Some(waiter_index) = blocked_interrupt_waiter_index(line.id) {
        let waiter_name = {
            let runtime = runtime();
            let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() else {
                return;
            };
            waiter.saved_frame.rax = STATUS_OK;
            waiter.state = ProcessState::Ready;
            waiter.name
        };
        if let Some(line) = runtime().objects.get_interrupt_line_mut(line.id) {
            line.delivered_count = line.delivered_count.saturating_add(1);
        }
        serial::write_str("IRQ delivered: line=");
        serial::write_u64_dec(irq_line);
        serial::write_str(" interrupt-line=");
        serial::write_str(line.name);
        serial::write_str("\n");
        serial::write_str("IRQ wake waiter: proc=");
        serial::write_str(waiter_name);
        serial::write_str(" interrupt-line=");
        serial::write_str(line.name);
        serial::write_str("\n");
        return;
    }

    if let Some(line) = runtime().objects.get_interrupt_line_mut(line.id) {
        line.pending_count = line.pending_count.saturating_add(1);
        serial::write_str("IRQ pending recorded: line=");
        serial::write_u64_dec(irq_line);
        serial::write_str(" interrupt-line=");
        serial::write_str(line.name);
        serial::write_str(" pending=");
        serial::write_u64_dec(line.pending_count);
        serial::write_str("\n");
    }
}

fn blocked_interrupt_waiter_index(interrupt: KernelObjectId) -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnInterrupt {
                interrupt: waiting_interrupt,
                ..
            } = process.state
            && waiting_interrupt == interrupt
        {
            return Some(index);
        }
        index += 1;
    }

    None
}

pub(super) fn interrupt_waiter_count(runtime: &RuntimeState, interrupt: KernelObjectId) -> u64 {
    let mut count = 0;
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnInterrupt {
                interrupt: waiting_interrupt,
                ..
            } = process.state
            && waiting_interrupt == interrupt
        {
            count += 1;
        }
        index += 1;
    }
    count
}

pub(super) fn interrupt_owner_name(
    runtime: &RuntimeState,
    interrupt: KernelObjectId,
) -> &'static str {
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index] {
            if capability_space_has_live_right(
                runtime,
                process.caps,
                interrupt,
                capability::RIGHT_LISTEN,
            ) || capability_space_has_live_right(
                runtime,
                process.initial_caps,
                interrupt,
                capability::RIGHT_LISTEN,
            ) {
                return process.name;
            }
        }
        index += 1;
    }

    "<none>"
}

fn capability_space_has_live_right(
    runtime: &RuntimeState,
    space: CapabilitySpace,
    object: KernelObjectId,
    right: u64,
) -> bool {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = space.caps[slot]
            && cap.object == object
            && cap.rights & right != 0
            && !cap.revoked
            && !runtime.cap_id_revoked(cap.id)
        {
            return true;
        }
        slot += 1;
    }

    false
}

pub fn mmio_map(cap_slot: u64) -> Result<u64, IpcError> {
    let region = mmio_region_from_cap(cap_slot, capability::RIGHT_MAP)?;
    let physical_base = align_down(region.base, memory::FRAME_SIZE);
    let page_offset = region
        .base
        .checked_sub(physical_base)
        .ok_or(IpcError::BadCapability)?;
    let map_len = align_up(
        region
            .length
            .checked_add(page_offset)
            .ok_or(IpcError::BadCapability)?,
        memory::FRAME_SIZE,
    )
    .ok_or(IpcError::BadCapability)?;
    let virtual_base = device_user_mapping_base(USER_MMIO_MAPPING_BASE, region.id, map_len)?;
    let user_base = virtual_base
        .checked_add(page_offset)
        .ok_or(IpcError::BadCapability)?;
    map_current_process_physical_range(
        virtual_base,
        physical_base,
        map_len,
        paging::PageFlags::user_device(),
    )?;
    serial::write_str("MMIO map accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" mmio-region=");
    serial::write_str(region.name);
    serial::write_str(" base=");
    serial::write_u64_hex(region.base);
    serial::write_str(" length=");
    serial::write_u64_hex(region.length);
    serial::write_str(" virt=");
    serial::write_u64_hex(user_base);
    serial::write_str("\n");
    Ok(user_base)
}

pub fn dma_map(cap_slot: u64, destination: *mut u8, max_len: usize) -> Result<(), IpcError> {
    if max_len < DMA_MAPPING_INFO_BYTES {
        return Err(IpcError::InvalidUserBuffer);
    }

    let region = dma_region_from_cap(
        cap_slot,
        capability::RIGHT_READ | capability::RIGHT_WRITE | capability::RIGHT_MAP,
    )?;
    let owner = runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
        .ok_or(IpcError::BadCapability)?;
    if (destination as u64) & 7 != 0 {
        return Err(IpcError::InvalidUserBuffer);
    }
    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        DMA_MAPPING_INFO_BYTES,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;
    let map_len = align_up(region.length, memory::FRAME_SIZE).ok_or(IpcError::BadCapability)?;
    let virtual_base = device_user_mapping_base(USER_DMA_MAPPING_BASE, region.id, map_len)?;
    if let Some(mapping) = runtime()
        .processes
        .current_process()
        .and_then(|process| process.dma_mapping(region.id))
    {
        let mut info = [0u8; DMA_MAPPING_INFO_BYTES];
        write_dma_mapping_info(&mut info, mapping);
        usercopy::copy_to_user(UserPtr::new(destination as u64), &info)
            .map_err(|_| IpcError::InvalidUserBuffer)?;
        serial::write_str("DMA map reused: proc=");
        serial::write_str(current_process_name());
        serial::write_str(" dma-region=");
        serial::write_str(region.name);
        serial::write_str(" virt=");
        serial::write_u64_hex(mapping.virtual_base);
        serial::write_str(" length=");
        serial::write_u64_hex(mapping.length);
        serial::write_str("\n");
        return Ok(());
    }

    claim_dma_region(region.id, owner)?;
    if !zero_dma_physical_range(region.base, region.length) {
        release_dma_region_claim(region.id, owner);
        return Err(IpcError::BadCapability);
    }

    map_current_process_physical_range(
        virtual_base,
        region.base,
        map_len,
        paging::PageFlags::user(true, false),
    )
    .map_err(|error| {
        release_dma_region_claim(region.id, owner);
        error
    })?;

    let mut info = [0u8; DMA_MAPPING_INFO_BYTES];
    let mapping = DmaUserMapping {
        region: region.id,
        virtual_base,
        physical_base: region.base,
        length: region.length,
    };
    write_dma_mapping_info(&mut info, mapping);
    if usercopy::copy_to_user(UserPtr::new(destination as u64), &info).is_err() {
        let _ = unmap_current_process_physical_range(virtual_base, map_len);
        release_dma_region_claim(region.id, owner);
        return Err(IpcError::InvalidUserBuffer);
    }
    let Some(process) = runtime().processes.current_process_mut() else {
        let _ = unmap_current_process_physical_range(virtual_base, map_len);
        release_dma_region_claim(region.id, owner);
        return Err(IpcError::BadCapability);
    };
    if process.add_dma_mapping(mapping).is_err() {
        let _ = unmap_current_process_physical_range(virtual_base, map_len);
        release_dma_region_claim(region.id, owner);
        return Err(IpcError::BadCapability);
    }

    serial::write_str("DMA map accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" dma-region=");
    serial::write_str(region.name);
    serial::write_str(" phys=");
    serial::write_u64_hex(region.base);
    serial::write_str(" virt=");
    serial::write_u64_hex(virtual_base);
    serial::write_str(" length=");
    serial::write_u64_hex(region.length);
    serial::write_str("\n");
    Ok(())
}

fn claim_dma_region(region_id: KernelObjectId, owner: ProcessId) -> Result<(), IpcError> {
    let (name, mapped_by) = {
        let runtime = runtime();
        let Some(region) = runtime.objects.get_dma_region(region_id) else {
            return Err(IpcError::BadCapability);
        };
        (region.name, region.mapped_by)
    };
    if mapped_by != ProcessId::empty() && mapped_by != owner {
        serial::write_str("DMA map rejected: dma-region=");
        serial::write_str(name);
        serial::write_str(" already-owned-by=");
        serial::write_str(process_name_by_pid(runtime(), mapped_by));
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }

    if mapped_by == ProcessId::empty() {
        let runtime = runtime();
        let Some(region) = runtime.objects.get_dma_region_mut(region_id) else {
            return Err(IpcError::BadCapability);
        };
        region.mapped_by = owner;
        region.map_count = region.map_count.saturating_add(1);
    }
    Ok(())
}

fn release_dma_region_claim(region_id: KernelObjectId, owner: ProcessId) {
    let runtime = runtime();
    if let Some(region) = runtime.objects.get_dma_region_mut(region_id)
        && region.mapped_by == owner
    {
        region.mapped_by = ProcessId::empty();
        region.release_count = region.release_count.saturating_add(1);
    }
}

pub(super) fn release_process_dma_mappings(pid: ProcessId) {
    let mut slot = 0;
    while slot < MAX_OBJECTS {
        let release = {
            let runtime = runtime();
            let Some(process) = runtime.processes.process_mut(pid) else {
                return;
            };
            let name = process.name;
            process
                .take_dma_mapping(slot)
                .map(|mapping| (name, mapping))
        };
        if let Some((name, mapping)) = release {
            release_dma_mapping(pid, name, mapping);
        }
        slot += 1;
    }
}

pub(super) fn release_all_runtime_dma_mappings() {
    let mut process_index = 0;
    loop {
        let pid = {
            let runtime = runtime();
            if process_index >= runtime.processes.count {
                break;
            }
            let Some(process) = runtime.processes.processes[process_index] else {
                process_index += 1;
                continue;
            };
            process.pid
        };
        release_process_dma_mappings(pid);
        process_index += 1;
    }
}

pub(super) fn release_process_virtio_ownership(pid: ProcessId) {
    let owner_name = process_name_by_pid(runtime(), pid);
    release_process_kernel_virtio_ownership(pid, owner_name);
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(KernelObject::VirtioDevice(device)) = runtime.objects.objects[index].as_mut()
            && device.owner == pid
        {
            device.owner = ProcessId::empty();
            serial::write_str("Virtio device ownership released: proc=");
            serial::write_str(owner_name);
            serial::write_str(" virtio-device=");
            serial::write_str(device.name);
            serial::write_str("\n");
        }
        index += 1;
    }
}

fn release_process_kernel_virtio_ownership(pid: ProcessId, owner_name: &'static str) {
    let rng = unsafe { &mut *VIRTIO_RNG_STATE.0.get() };
    if rng.owner == pid {
        if rng.io_base != 0 {
            virtio_write8(rng.io_base, VIRTIO_PCI_STATUS, 0);
        }
        rng.initialized = false;
        rng.owner = ProcessId::empty();
        rng.reset_count = rng.reset_count.saturating_add(1);
        rng.last_error = "owner-release";
        rng.queue.last_error = "owner-release";
        serial::write_str("Virtio kernel device ownership released: proc=");
        serial::write_str(owner_name);
        serial::write_str(" virtio-device=");
        serial::write_str(VIRTIO_RNG_DEVICE_ID);
        serial::write_str("\n");
    }

    let net = unsafe { &mut *VIRTIO_NET_STATE.0.get() };
    if net.owner == pid {
        if net.io_base != 0 {
            virtio_write8(net.io_base, VIRTIO_PCI_STATUS, 0);
        }
        net.initialized = false;
        net.rx_posted = false;
        net.owner = ProcessId::empty();
        net.reset_count = net.reset_count.saturating_add(1);
        net.last_error = "owner-release";
        net.rx.last_error = "owner-release";
        net.tx.last_error = "owner-release";
        serial::write_str("Virtio kernel device ownership released: proc=");
        serial::write_str(owner_name);
        serial::write_str(" virtio-device=");
        serial::write_str(VIRTIO_NET_DEVICE_ID);
        serial::write_str("\n");
    }
}

fn release_dma_mapping(owner: ProcessId, owner_name: &'static str, mapping: DmaUserMapping) {
    let _ = zero_dma_physical_range(mapping.physical_base, mapping.length);
    release_dma_region_claim(mapping.region, owner);
    serial::write_str("DMA mapping released: proc=");
    serial::write_str(owner_name);
    serial::write_str(" phys=");
    serial::write_u64_hex(mapping.physical_base);
    serial::write_str(" length=");
    serial::write_u64_hex(mapping.length);
    serial::write_str("\n");
}

fn zero_dma_physical_range(physical_base: u64, length: u64) -> bool {
    if length == 0 {
        return true;
    }
    let Some(hhdm_offset) = limine::hhdm_offset() else {
        return false;
    };
    let Ok(length) = usize::try_from(length) else {
        return false;
    };
    let Some(virtual_base) = hhdm_offset.checked_add(physical_base) else {
        return false;
    };
    unsafe {
        core::ptr::write_bytes(virtual_base as *mut u8, 0, length);
    }
    true
}
