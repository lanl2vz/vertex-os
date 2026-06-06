use super::vfs_wire::{
    read_u16_le, read_u64_le, serial_write_vfs_name, write_u16_le, write_u64_le,
    write_vfs_stat_record,
};
use super::*;

pub(super) fn start_vfs_state_transaction(
    state: KernelObjectId,
    node: VfsNode,
    description: OpenFileDescription,
    operation: VfsStateOperation,
    offset: u64,
    destination: u64,
    max_len: usize,
    write_len: usize,
    update_offset: bool,
    payload: &[u8; MAX_MESSAGE_BYTES],
    payload_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let (state_name, request_endpoint, reply_endpoint) = {
        let runtime = runtime();
        let state_object = runtime
            .objects
            .get_state_volume(state)
            .ok_or(IpcError::VfsBadHandle)?;
        let request_endpoint = runtime
            .state_vfs_request_endpoint
            .ok_or(IpcError::VfsUnsupported)?;
        let reply_endpoint = runtime
            .state_vfs_reply_endpoint
            .ok_or(IpcError::VfsUnsupported)?;
        (state_object.name, request_endpoint, reply_endpoint)
    };
    let state_name_len = state_name.len();
    let request_len = VFS_STATE_TRANSACTION_ID_BYTES
        .checked_add(VFS_STATE_REQUEST_HEADER_BYTES)
        .and_then(|len| len.checked_add(state_name_len))
        .and_then(|len| len.checked_add(payload_len))
        .ok_or(IpcError::VfsNoSpace)?;
    if state_name_len > u16::MAX as usize
        || payload_len > u16::MAX as usize
        || request_len > MAX_MESSAGE_BYTES
    {
        return Err(IpcError::VfsNoSpace);
    }
    let transaction_id = {
        let runtime = runtime();
        let id = runtime.next_vfs_state_transaction_id;
        if id == 0 || id == u64::MAX {
            return Err(IpcError::VfsNoSpace);
        }
        runtime.next_vfs_state_transaction_id = id + 1;
        id
    };

    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::VfsPermission);
        };
        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnVfsState {
            reply_endpoint,
            node: node.id,
            description: description.id,
            operation,
            transaction_id,
            offset,
            destination,
            max_len,
            write_len,
            update_offset,
        };
        process.name
    };

    let mut queued_request = [0u8; MAX_MESSAGE_BYTES];
    write_u64_le(&mut queued_request, 0, transaction_id);
    let request_offset = VFS_STATE_TRANSACTION_ID_BYTES;
    queued_request[request_offset] = VFS_STATE_REQUEST_MAGIC[0];
    queued_request[request_offset + 1] = VFS_STATE_REQUEST_MAGIC[1];
    queued_request[request_offset + 2] = VFS_STATE_REQUEST_VERSION;
    queued_request[request_offset + 3] = vfs_state_operation_code(operation);
    write_u16_le(
        &mut queued_request,
        request_offset + 4,
        state_name_len as u16,
    );
    write_u16_le(&mut queued_request, request_offset + 6, payload_len as u16);
    let state_offset = request_offset + VFS_STATE_REQUEST_HEADER_BYTES;
    queued_request[state_offset..state_offset + state_name_len]
        .copy_from_slice(state_name.as_bytes());
    let payload_offset = state_offset + state_name_len;
    queued_request[payload_offset..payload_offset + payload_len]
        .copy_from_slice(&payload[..payload_len]);
    let queued_request_len = request_len;
    let enqueue_result = {
        let runtime = runtime();
        runtime
            .objects
            .get_endpoint_mut(request_endpoint)
            .ok_or(IpcError::BadCapability)?
            .enqueue(ProcessId::empty(), &queued_request, queued_request_len)
    };
    if let Err(error) = enqueue_result {
        restore_current_vfs_state_waiter(reply_endpoint);
        return Err(error);
    }

    serial::write_str("VFS state transaction request: proc=");
    serial::write_str(current);
    serial::write_str(" state=");
    serial::write_str(state_name);
    serial::write_str(" op=");
    serial::write_str(vfs_state_operation_label(operation));
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" description=");
    serial::write_u64_dec(description.id.raw());
    serial::write_str(" tx=");
    serial::write_u64_dec(transaction_id);
    serial::write_str("\n");

    wake_blocked_receiver(request_endpoint);

    if schedule_next_ready(frame) {
        return Ok(());
    }

    restore_current_vfs_state_waiter(reply_endpoint);
    if let Some(endpoint) = runtime().objects.get_endpoint_mut(request_endpoint) {
        let _ = endpoint.remove_vfs_state_request(ProcessId::empty(), transaction_id);
    }

    serial::write_str("Scheduler blocked: proc=");
    serial::write_str(current);
    serial::write_str(" no ready process for VFS state transaction\n");
    Err(IpcError::Empty)
}

pub(super) fn start_vfs_service_read_transaction(
    node: VfsNode,
    description: OpenFileDescription,
    offset: u64,
    destination: u64,
    max_len: usize,
    update_offset: bool,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let (request_endpoint, reply_endpoint) = {
        let runtime = runtime();
        (
            runtime
                .state_vfs_request_endpoint
                .ok_or(IpcError::VfsUnsupported)?,
            runtime
                .state_vfs_reply_endpoint
                .ok_or(IpcError::VfsUnsupported)?,
        )
    };
    let transaction_id = {
        let runtime = runtime();
        let id = runtime.next_vfs_state_transaction_id;
        if id == 0 || id == u64::MAX {
            return Err(IpcError::VfsNoSpace);
        }
        runtime.next_vfs_state_transaction_id = id + 1;
        id
    };

    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::VfsPermission);
        };
        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnVfsState {
            reply_endpoint,
            node: node.id,
            description: description.id,
            operation: VfsStateOperation::ServiceRead,
            transaction_id,
            offset,
            destination,
            max_len,
            write_len: 0,
            update_offset,
        };
        process.name
    };

    let mut queued_request = [0u8; MAX_MESSAGE_BYTES];
    write_u64_le(&mut queued_request, 0, transaction_id);
    queued_request[8] = VFS_SERVICE_REQUEST_MAGIC[0];
    queued_request[9] = VFS_SERVICE_REQUEST_MAGIC[1];
    queued_request[10] = VFS_SERVICE_REQUEST_VERSION;
    queued_request[11] = VFS_SERVICE_OP_READ_REPORT;
    let enqueue_result = {
        let runtime = runtime();
        runtime
            .objects
            .get_endpoint_mut(request_endpoint)
            .ok_or(IpcError::BadCapability)?
            .enqueue(
                ProcessId::empty(),
                &queued_request,
                VFS_SERVICE_REQUEST_BYTES,
            )
    };
    if let Err(error) = enqueue_result {
        restore_current_vfs_state_waiter(reply_endpoint);
        return Err(error);
    }

    serial::write_str("VFS filesystem service request: proc=");
    serial::write_str(current);
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" tx=");
    serial::write_u64_dec(transaction_id);
    serial::write_str("\n");

    wake_blocked_receiver(request_endpoint);

    if schedule_next_ready(frame) {
        return Ok(());
    }

    restore_current_vfs_state_waiter(reply_endpoint);
    if let Some(endpoint) = runtime().objects.get_endpoint_mut(request_endpoint) {
        let _ = endpoint.remove_vfs_state_request(ProcessId::empty(), transaction_id);
    }

    serial::write_str("Scheduler blocked: proc=");
    serial::write_str(current);
    serial::write_str(" no ready process for VFS filesystem service transaction\n");
    Err(IpcError::Empty)
}

fn restore_current_vfs_state_waiter(reply_endpoint: KernelObjectId) {
    let runtime = runtime();
    if let Some(process) = runtime.processes.current_process_mut()
        && let ProcessState::BlockedOnVfsState {
            reply_endpoint: waiting_endpoint,
            ..
        } = process.state
        && waiting_endpoint == reply_endpoint
    {
        process.state = ProcessState::Running;
    }
}

pub(super) fn start_vertexfs_sync_transaction(
    backing: usize,
    inode_id: u32,
    checksum: u32,
    write_count: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if write_count == 0 || write_count > VERTEXFS_SYNC_MAX_DEVICE_WRITES {
        return Err(IpcError::VfsUnsupported);
    }
    let (request_endpoint, reply_endpoint, first_sector) = {
        let runtime = runtime();
        let request_endpoint = runtime
            .vertexfs_device_request_endpoint
            .ok_or(IpcError::VfsUnsupported)?;
        let reply_endpoint = runtime
            .vertexfs_device_reply_endpoint
            .ok_or(IpcError::VfsUnsupported)?;
        if blocked_vertexfs_sync_waiter_index(reply_endpoint).is_some() {
            return Err(IpcError::VfsBusy);
        }
        let first_sector = vertexfs_device_absolute_sector(runtime.vertexfs_sync_writes[0].sector)?;
        (request_endpoint, reply_endpoint, first_sector)
    };

    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::VfsPermission);
        };
        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnVertexFsSync {
            request_endpoint,
            reply_endpoint,
            backing,
            inode_id,
            checksum,
            write_count,
            next_write: 1,
            expected_sector: first_sector,
        };
        process.name
    };

    if let Err(error) = queue_vertexfs_device_write(request_endpoint, 0) {
        restore_current_vertexfs_sync_waiter(reply_endpoint);
        return Err(error);
    }
    wake_blocked_receiver(request_endpoint);

    serial::write_str("VertexFS v1 fsync device transaction started: proc=");
    serial::write_str(current);
    serial::write_str(" inode=");
    serial::write_u64_dec(inode_id as u64);
    serial::write_str(" sectors=");
    serial::write_u64_dec(write_count as u64);
    serial::write_str("\n");

    if schedule_next_ready(frame) {
        return Ok(());
    }

    restore_current_vertexfs_sync_waiter(reply_endpoint);
    if let Some(endpoint) = runtime().objects.get_endpoint_mut(request_endpoint) {
        let _ = endpoint.remove_all_from_sender(ProcessId::empty());
    }
    serial::write_str("Scheduler blocked: proc=");
    serial::write_str(current);
    serial::write_str(" no ready process for VertexFS device sync\n");
    Err(IpcError::Empty)
}

fn restore_current_vertexfs_sync_waiter(reply_endpoint: KernelObjectId) {
    let runtime = runtime();
    if let Some(process) = runtime.processes.current_process_mut()
        && let ProcessState::BlockedOnVertexFsSync {
            reply_endpoint: waiting_endpoint,
            ..
        } = process.state
        && waiting_endpoint == reply_endpoint
    {
        process.state = ProcessState::Running;
    }
}

fn queue_vertexfs_device_write(
    request_endpoint: KernelObjectId,
    write_index: usize,
) -> Result<u64, IpcError> {
    let write = {
        let runtime = runtime();
        if write_index >= runtime.vertexfs_sync_write_count {
            return Err(IpcError::VfsBadHandle);
        }
        runtime.vertexfs_sync_writes[write_index]
    };
    let absolute_sector = vertexfs_device_absolute_sector(write.sector)?;
    let mut request = [0u8; MAX_MESSAGE_BYTES];
    write_u16_le(&mut request, 0, BLOCK_PROTOCOL_V1);
    write_u16_le(&mut request, 2, BLOCK_OP_WRITE_SECTOR);
    write_u16_le(&mut request, 4, 0);
    write_u64_le(&mut request, 8, absolute_sector);

    let enqueue_result = {
        let runtime = runtime();
        let endpoint = runtime
            .objects
            .get_endpoint_mut(request_endpoint)
            .ok_or(IpcError::BadCapability)?;
        endpoint.enqueue(ProcessId::empty(), &request, BLOCK_REQUEST_LEN)?;
        let mut payload = [0u8; MAX_MESSAGE_BYTES];
        payload.copy_from_slice(&write.bytes);
        if let Err(error) = endpoint.enqueue(ProcessId::empty(), &payload, VERTEXFS_SECTOR_SIZE) {
            let _ = endpoint.remove_all_from_sender(ProcessId::empty());
            return Err(error);
        }
        Ok(())
    };
    enqueue_result?;
    Ok(absolute_sector)
}

fn vertexfs_device_ack_ok(message: IpcMessage, expected_sector: u64) -> bool {
    message.len == BLOCK_WRITE_ACK_LEN
        && read_u16_le(&message.bytes, 0) == BLOCK_PROTOCOL_V1
        && read_u16_le(&message.bytes, 2) == BLOCK_OP_WRITE_SECTOR
        && read_u16_le(&message.bytes, 4) == 0
        && read_u64_le(&message.bytes, 8) == expected_sector
}

pub(super) fn abort_vfs_state_transactions(status: u64) {
    let (request_endpoint, reply_endpoint) = {
        let runtime = runtime();
        (
            runtime.state_vfs_request_endpoint,
            runtime.state_vfs_reply_endpoint,
        )
    };
    if let Some(request_endpoint) = request_endpoint
        && let Some(endpoint) = runtime().objects.get_endpoint_mut(request_endpoint)
    {
        let removed = endpoint.remove_all_from_sender(ProcessId::empty());
        if removed > 0 {
            serial::write_str("VFS state transaction requests dropped: count=");
            serial::write_u64_dec(removed as u64);
            serial::write_str("\n");
        }
    }
    let Some(reply_endpoint) = reply_endpoint else {
        return;
    };

    let mut aborted = 0;
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index].as_mut()
            && let ProcessState::BlockedOnVfsState {
                reply_endpoint: waiting_endpoint,
                operation,
                ..
            } = process.state
            && waiting_endpoint == reply_endpoint
        {
            process.saved_frame.rax = status;
            process.state = ProcessState::Ready;
            aborted += 1;
            serial::write_str("VFS state transaction aborted: proc=");
            serial::write_str(process.name);
            serial::write_str(" op=");
            serial::write_str(vfs_state_operation_label(operation));
            serial::write_str(" status=");
            serial::write_u64_dec(status);
            serial::write_str("\n");
        }
        index += 1;
    }
    if aborted > 0 {
        serial::write_str("VFS state transaction abort wake count=");
        serial::write_u64_dec(aborted);
        serial::write_str("\n");
    }
}

fn vfs_state_operation_label(operation: VfsStateOperation) -> &'static str {
    match operation {
        VfsStateOperation::Read => "read",
        VfsStateOperation::Stat => "stat",
        VfsStateOperation::Write => "write",
        VfsStateOperation::Control => "control",
        VfsStateOperation::ServiceRead => "service-read",
    }
}

pub(super) fn abort_vertexfs_sync_transactions(status: u64) {
    let (request_endpoint, reply_endpoint) = {
        let runtime = runtime();
        (
            runtime.vertexfs_device_request_endpoint,
            runtime.vertexfs_device_reply_endpoint,
        )
    };
    if let Some(endpoint_id) = request_endpoint
        && let Some(endpoint) = runtime().objects.get_endpoint_mut(endpoint_id)
    {
        let _ = endpoint.remove_all_from_sender(ProcessId::empty());
    }
    let Some(reply_endpoint) = reply_endpoint else {
        return;
    };
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index].as_mut()
            && let ProcessState::BlockedOnVertexFsSync {
                reply_endpoint: waiting_endpoint,
                ..
            } = process.state
            && waiting_endpoint == reply_endpoint
        {
            process.saved_frame.rax = status;
            process.state = ProcessState::Ready;
            serial::write_str("VertexFS v1 fsync device transaction aborted: proc=");
            serial::write_str(process.name);
            serial::write_str("\n");
        }
        index += 1;
    }
}

fn vfs_state_operation_code(operation: VfsStateOperation) -> u8 {
    match operation {
        VfsStateOperation::Read => VFS_STATE_OP_READ_VALUE,
        VfsStateOperation::Stat => VFS_STATE_OP_STAT_VALUE,
        VfsStateOperation::Write => VFS_STATE_OP_WRITE_VALUE,
        VfsStateOperation::Control => VFS_STATE_OP_CONTROL,
        VfsStateOperation::ServiceRead => VFS_SERVICE_OP_READ_REPORT,
    }
}

pub(super) fn block_current_on_vfs_read(
    node: VfsNodeId,
    description: FileDescriptionId,
    destination: u64,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> bool {
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return false;
        };

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnVfsRead {
            node,
            description,
            destination,
            max_len,
        };
        process.name
    };

    serial::write_str("VFS read blocked: proc=");
    serial::write_str(current);
    serial::write_str(" vnode=");
    serial::write_u64_dec(node.raw());
    serial::write_str(" description=");
    serial::write_u64_dec(description.raw());
    serial::write_str("\n");

    if schedule_next_ready(frame) {
        true
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }

        serial::write_str("Scheduler blocked: proc=");
        serial::write_str(current);
        serial::write_str(" no ready process\n");
        false
    }
}

pub(super) fn block_current_on_network_port(
    port: KernelObjectId,
    destination: u64,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> bool {
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return false;
        };

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnNetworkPort {
            port,
            destination,
            max_len,
        };
        process.name
    };

    serial::write_str("Network-port receive blocked: proc=");
    serial::write_str(current);
    serial::write_str(" port=");
    serial::write_u64_dec(port.raw());
    serial::write_str("\n");

    if schedule_next_ready(frame) {
        true
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }

        serial::write_str("Scheduler blocked: proc=");
        serial::write_str(current);
        serial::write_str(" no ready process for network-port receive\n");
        false
    }
}

pub(super) fn wake_blocked_network_receiver(port: KernelObjectId) {
    let Some(waiter_index) = blocked_network_receiver_index(port) else {
        return;
    };

    let (name, receiver_cr3, destination, max_len, current_cr3) = {
        let runtime = runtime();
        let Some(waiter) = runtime.processes.processes[waiter_index] else {
            return;
        };
        let ProcessState::BlockedOnNetworkPort {
            destination,
            max_len,
            ..
        } = waiter.state
        else {
            return;
        };

        let current_cr3 = runtime
            .processes
            .current_process()
            .map(|process| process.context.cr3)
            .unwrap_or_else(paging::active_root_table_physical);

        (
            waiter.name,
            waiter.context.cr3,
            destination,
            max_len,
            current_cr3,
        )
    };

    let (port_name, message) = {
        let runtime = runtime();
        let Some(port_object) = runtime.objects.get_network_port_mut(port) else {
            return;
        };
        let Some(message) = port_object.dequeue_udp() else {
            return;
        };
        (port_object.name, message)
    };

    let copy_len = min(message.len, max_len);
    let copy_result = unsafe {
        gdt::switch_address_space(receiver_cr3);
        let result = usercopy::copy_to_user(UserPtr::new(destination), &message.bytes[..copy_len]);
        gdt::switch_address_space(current_cr3);
        result
    };

    match copy_result {
        Ok(()) => {
            let runtime = runtime();
            if let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() {
                waiter.saved_frame.rax = copy_len as u64;
                waiter.state = ProcessState::Ready;
            }
            serial::write_str("Network-port UDP request delivered to netstack: network-port=");
            serial::write_str(port_name);
            serial::write_str(" bytes=");
            serial::write_u64_dec(copy_len as u64);
            serial::write_str("\n");
            serial::write_str("Network-port receive wake: proc=");
            serial::write_str(name);
            serial::write_str("\n");
        }
        Err(_) => {
            let runtime = runtime();
            if let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() {
                waiter.saved_frame.rax = STATUS_BAD_BUFFER;
                waiter.state = ProcessState::Ready;
            }
            serial::write_str("Network-port receive wake failed: bad user buffer proc=");
            serial::write_str(name);
            serial::write_str("\n");
        }
    }
}

pub(super) fn wake_blocked_vfs_pipe_read(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let Some(waiter_index) = blocked_vfs_pipe_reader_index() else {
        return false;
    };

    let (name, reader_cr3, destination, max_len, description, node, current_cr3) = {
        let runtime = runtime();
        let Some(waiter) = runtime.processes.processes[waiter_index] else {
            return false;
        };
        let ProcessState::BlockedOnVfsRead {
            node,
            description,
            destination,
            max_len,
        } = waiter.state
        else {
            return false;
        };

        let current_cr3 = runtime
            .processes
            .current_process()
            .map(|process| process.context.cr3)
            .unwrap_or_else(paging::active_root_table_physical);

        (
            waiter.name,
            waiter.context.cr3,
            destination,
            max_len,
            description,
            node,
            current_cr3,
        )
    };

    let copy_len = min(bytes.len(), max_len);
    let copy_result = unsafe {
        gdt::switch_address_space(reader_cr3);
        let result = usercopy::copy_to_user(UserPtr::new(destination), &bytes[..copy_len]);
        gdt::switch_address_space(current_cr3);
        result
    };

    match copy_result {
        Ok(()) => {
            let file_name = {
                let runtime = runtime();
                runtime
                    .vfs_node(node)
                    .map(|node| node.name)
                    .unwrap_or_else(VfsName::empty)
            };
            {
                let runtime = runtime();
                if let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() {
                    waiter.saved_frame.rax = copy_len as u64;
                    waiter.state = ProcessState::Ready;
                }
            }

            serial::write_str("VFS pipe wake reader: proc=");
            serial::write_str(name);
            serial::write_str(" file=");
            serial_write_vfs_name(file_name);
            serial::write_str(" description=");
            serial::write_u64_dec(description.raw());
            serial::write_str(" bytes=");
            serial::write_u64_dec(copy_len as u64);
            serial::write_str("\n");
            true
        }
        Err(_) => {
            {
                let runtime = runtime();
                if let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() {
                    waiter.saved_frame.rax = STATUS_BAD_BUFFER;
                    waiter.state = ProcessState::Ready;
                }
            }
            serial::write_str("VFS pipe wake reader failed: bad user buffer proc=");
            serial::write_str(name);
            serial::write_str("\n");
            true
        }
    }
}

pub(super) fn wake_blocked_vfs_state_reply(endpoint: KernelObjectId) {
    let Some(waiter_index) = blocked_vfs_state_waiter_index(endpoint) else {
        return;
    };

    let (
        name,
        receiver_pid,
        receiver_cr3,
        destination,
        max_len,
        operation,
        transaction_id,
        offset,
        update_offset,
        write_len,
        description,
        node,
        current_cr3,
    ) = {
        let runtime = runtime();
        let Some(waiter) = runtime.processes.processes[waiter_index] else {
            return;
        };
        let ProcessState::BlockedOnVfsState {
            destination,
            max_len,
            operation,
            transaction_id,
            offset,
            update_offset,
            write_len,
            description,
            node,
            ..
        } = waiter.state
        else {
            return;
        };

        let current_cr3 = runtime
            .processes
            .current_process()
            .map(|process| process.context.cr3)
            .unwrap_or_else(paging::active_root_table_physical);

        (
            waiter.name,
            waiter.pid,
            waiter.context.cr3,
            destination,
            max_len,
            operation,
            transaction_id,
            offset,
            update_offset,
            write_len,
            description,
            node,
            current_cr3,
        )
    };

    let Some(message) = ({
        let runtime = runtime();
        let Some(endpoint_object) = runtime.objects.get_endpoint_mut(endpoint) else {
            return;
        };
        endpoint_object.dequeue_vfs_state_reply_for(receiver_pid, transaction_id)
    }) else {
        return;
    };

    let result = match operation {
        VfsStateOperation::Read | VfsStateOperation::ServiceRead => wake_blocked_vfs_state_read(
            receiver_cr3,
            current_cr3,
            destination,
            max_len,
            offset,
            description,
            update_offset,
            message,
        ),
        VfsStateOperation::Stat => wake_blocked_vfs_state_stat(
            receiver_cr3,
            current_cr3,
            destination,
            max_len,
            description,
            node,
            message,
        ),
        VfsStateOperation::Write => {
            wake_blocked_vfs_state_write(offset, description, update_offset, write_len, message)
        }
        VfsStateOperation::Control => {
            wake_blocked_vfs_state_write(offset, description, update_offset, write_len, message)
        }
    };

    {
        let runtime = runtime();
        if let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() {
            waiter.saved_frame.rax = result;
            waiter.state = ProcessState::Ready;
        }
    }

    let file_name = {
        let runtime = runtime();
        runtime
            .vfs_node(node)
            .map(|node| node.name)
            .unwrap_or_else(VfsName::empty)
    };
    if operation == VfsStateOperation::ServiceRead {
        serial::write_str("VFS filesystem service transaction wake: proc=");
    } else {
        serial::write_str("VFS state transaction wake: proc=");
    }
    serial::write_str(name);
    serial::write_str(" file=");
    serial_write_vfs_name(file_name);
    serial::write_str(" op=");
    serial::write_str(vfs_state_operation_label(operation));
    serial::write_str(" result=");
    serial::write_u64_dec(result);
    serial::write_str("\n");
}

pub(super) fn wake_blocked_vertexfs_sync_reply(endpoint: KernelObjectId) {
    let Some(waiter_index) = blocked_vertexfs_sync_waiter_index(endpoint) else {
        return;
    };

    let (
        name,
        receiver_pid,
        request_endpoint,
        backing,
        inode_id,
        checksum,
        write_count,
        next_write,
        expected_sector,
    ) = {
        let runtime = runtime();
        let Some(waiter) = runtime.processes.processes[waiter_index] else {
            return;
        };
        let ProcessState::BlockedOnVertexFsSync {
            request_endpoint,
            backing,
            inode_id,
            checksum,
            write_count,
            next_write,
            expected_sector,
            ..
        } = waiter.state
        else {
            return;
        };
        (
            waiter.name,
            waiter.pid,
            request_endpoint,
            backing,
            inode_id,
            checksum,
            write_count,
            next_write,
            expected_sector,
        )
    };

    let Some(message) = ({
        let runtime = runtime();
        let Some(endpoint_object) = runtime.objects.get_endpoint_mut(endpoint) else {
            return;
        };
        endpoint_object.dequeue_for(receiver_pid)
    }) else {
        return;
    };

    if !vertexfs_device_ack_ok(message, expected_sector) {
        if let Some(waiter) = runtime().processes.processes[waiter_index].as_mut() {
            waiter.saved_frame.rax = STATUS_VFS_UNSUPPORTED;
            waiter.state = ProcessState::Ready;
        }
        serial::write_str("VertexFS v1 fsync device write rejected: proc=");
        serial::write_str(name);
        serial::write_str(" sector=");
        serial::write_u64_dec(expected_sector);
        serial::write_str("\n");
        return;
    }

    if next_write < write_count {
        let Ok(next_sector) = queue_vertexfs_device_write(request_endpoint, next_write) else {
            if let Some(waiter) = runtime().processes.processes[waiter_index].as_mut() {
                waiter.saved_frame.rax = STATUS_VFS_UNSUPPORTED;
                waiter.state = ProcessState::Ready;
            }
            serial::write_str("VertexFS v1 fsync device queue failed: proc=");
            serial::write_str(name);
            serial::write_str("\n");
            return;
        };
        if let Some(waiter) = runtime().processes.processes[waiter_index].as_mut()
            && let ProcessState::BlockedOnVertexFsSync {
                next_write: waiting_next_write,
                expected_sector: waiting_expected_sector,
                ..
            } = &mut waiter.state
        {
            *waiting_next_write = next_write + 1;
            *waiting_expected_sector = next_sector;
        }
        wake_blocked_receiver(request_endpoint);
        return;
    }

    let result = runtime().finish_vertexfs_sync_file(backing, checksum);
    if let Some(waiter) = runtime().processes.processes[waiter_index].as_mut() {
        waiter.saved_frame.rax = if result.is_ok() {
            STATUS_OK
        } else {
            STATUS_VFS_BAD_HANDLE
        };
        waiter.state = ProcessState::Ready;
    }
    serial::write_str("VertexFS v1 fsync device transaction committed: proc=");
    serial::write_str(name);
    serial::write_str(" inode=");
    serial::write_u64_dec(inode_id as u64);
    serial::write_str(" sectors=");
    serial::write_u64_dec(write_count as u64);
    serial::write_str(" checksum=");
    serial::write_u64_dec(checksum as u64);
    serial::write_str("\n");
}

fn wake_blocked_vfs_state_read(
    receiver_cr3: u64,
    current_cr3: u64,
    destination: u64,
    max_len: usize,
    offset: u64,
    description: FileDescriptionId,
    update_offset: bool,
    message: IpcMessage,
) -> u64 {
    if message.len < VFS_STATE_TRANSACTION_ID_BYTES {
        return STATUS_VFS_UNSUPPORTED;
    }
    let payload_len = message.len - VFS_STATE_TRANSACTION_ID_BYTES;
    let start = min(usize::try_from(offset).unwrap_or(usize::MAX), payload_len);
    let copy_len = min(payload_len - start, max_len);
    let payload_start = VFS_STATE_TRANSACTION_ID_BYTES + start;
    let copy_result = unsafe {
        gdt::switch_address_space(receiver_cr3);
        let result = usercopy::copy_to_user(
            UserPtr::new(destination),
            &message.bytes[payload_start..payload_start + copy_len],
        );
        gdt::switch_address_space(current_cr3);
        result
    };
    if copy_result.is_err() {
        return STATUS_BAD_BUFFER;
    }
    if update_offset {
        let Some(new_offset) = offset.checked_add(copy_len as u64) else {
            return STATUS_VFS_UNSUPPORTED;
        };
        let Some(file) = runtime().file_description_mut(description) else {
            return STATUS_VFS_BAD_HANDLE;
        };
        file.offset = new_offset;
    }
    copy_len as u64
}

fn wake_blocked_vfs_state_stat(
    receiver_cr3: u64,
    current_cr3: u64,
    destination: u64,
    max_len: usize,
    description: FileDescriptionId,
    node_id: VfsNodeId,
    message: IpcMessage,
) -> u64 {
    if max_len < VFS_STAT_BYTES || message.len != VFS_STATE_TRANSACTION_ID_BYTES + 8 {
        return STATUS_VFS_UNSUPPORTED;
    }
    let node = {
        let runtime = runtime();
        let Some(node) = runtime.vfs_node(node_id) else {
            return STATUS_VFS_BAD_HANDLE;
        };
        node
    };
    let rights = {
        let runtime = runtime();
        let Some(file) = runtime.file_description(description) else {
            return STATUS_VFS_BAD_HANDLE;
        };
        file.rights
    };
    let mut stat = [0u8; VFS_STAT_BYTES];
    write_vfs_stat_record(
        &mut stat,
        node,
        read_u64_le(&message.bytes, VFS_STATE_TRANSACTION_ID_BYTES),
        rights,
    );
    let copy_result = unsafe {
        gdt::switch_address_space(receiver_cr3);
        let result = usercopy::copy_to_user(UserPtr::new(destination), &stat);
        gdt::switch_address_space(current_cr3);
        result
    };
    if copy_result.is_err() {
        return STATUS_BAD_BUFFER;
    }
    VFS_STAT_BYTES as u64
}

fn wake_blocked_vfs_state_write(
    offset: u64,
    description: FileDescriptionId,
    update_offset: bool,
    write_len: usize,
    message: IpcMessage,
) -> u64 {
    if message.len != VFS_STATE_TRANSACTION_ID_BYTES + 2
        || message.bytes[VFS_STATE_TRANSACTION_ID_BYTES] != b'O'
        || message.bytes[VFS_STATE_TRANSACTION_ID_BYTES + 1] != b'K'
    {
        return STATUS_VFS_UNSUPPORTED;
    }
    if update_offset {
        let Some(new_offset) = offset.checked_add(write_len as u64) else {
            return STATUS_VFS_UNSUPPORTED;
        };
        let Some(file) = runtime().file_description_mut(description) else {
            return STATUS_VFS_BAD_HANDLE;
        };
        file.offset = new_offset;
    }
    write_len as u64
}

fn blocked_vfs_state_waiter_index(endpoint: KernelObjectId) -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnVfsState {
                reply_endpoint,
                transaction_id,
                ..
            } = process.state
            && reply_endpoint == endpoint
            && runtime
                .objects
                .get_endpoint(endpoint)
                .map(|endpoint_object| {
                    endpoint_object.has_vfs_state_reply_for(process.pid, transaction_id)
                })
                .unwrap_or(false)
        {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn blocked_vertexfs_sync_waiter_index(endpoint: KernelObjectId) -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnVertexFsSync { reply_endpoint, .. } = process.state
            && reply_endpoint == endpoint
            && runtime
                .objects
                .get_endpoint(endpoint)
                .map(|endpoint_object| endpoint_object.has_message_for(process.pid))
                .unwrap_or(false)
        {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn blocked_vfs_pipe_reader_index() -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnVfsRead { node, .. } = process.state
            && runtime
                .vfs_node(node)
                .map(|node| matches!(node.backing, VfsBacking::Pipe))
                .unwrap_or(false)
        {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn blocked_network_receiver_index(port: KernelObjectId) -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnNetworkPort {
                port: waiting_port, ..
            } = process.state
            && waiting_port == port
            && runtime
                .objects
                .get_network_port(port)
                .map(NetworkPortObject::has_pending_udp)
                .unwrap_or(false)
        {
            return Some(index);
        }
        index += 1;
    }
    None
}
