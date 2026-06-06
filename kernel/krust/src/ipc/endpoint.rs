use crate::{
    kernel::{
        IpcEndpoint, IpcError, KernelObjectId, MAX_MESSAGE_BYTES, MESSAGE_QUEUE_CAPACITY, ProcessId,
    },
    serial,
};

pub fn run_fifo_regression() {
    let provider = ProcessId::new(1);
    let client_a = ProcessId::new(2);
    let client_b = ProcessId::new(3);
    let mut endpoint = IpcEndpoint::new(
        KernelObjectId::new(0xf100),
        "fifo-regression",
        ProcessId::empty(),
    );
    let mut message = [0u8; MAX_MESSAGE_BYTES];

    message[0] = b'a';
    if endpoint.enqueue(client_a, &message, 1).is_err() {
        fifo_regression_failed("enqueue a");
        return;
    }
    message[0] = b'b';
    if endpoint.enqueue(client_b, &message, 1).is_err() {
        fifo_regression_failed("enqueue b");
        return;
    }
    if endpoint
        .dequeue_for(provider)
        .map(|queued| queued.len == 1 && queued.bytes[0] == b'a')
        != Some(true)
    {
        fifo_regression_failed("fifo first");
        return;
    }
    if endpoint
        .dequeue_for(provider)
        .map(|queued| queued.len == 1 && queued.bytes[0] == b'b')
        != Some(true)
    {
        fifo_regression_failed("fifo second");
        return;
    }
    serial::write_str("IPC FIFO regression: queued sends preserve FIFO order\n");

    let mut full_endpoint = IpcEndpoint::new(
        KernelObjectId::new(0xf101),
        "fifo-full-regression",
        ProcessId::empty(),
    );
    let mut index = 0;
    while index < MESSAGE_QUEUE_CAPACITY {
        message[0] = b'0' + index as u8;
        if full_endpoint.enqueue(client_a, &message, 1).is_err() {
            fifo_regression_failed("fill queue");
            return;
        }
        index += 1;
    }
    if !matches!(
        full_endpoint.enqueue(client_b, &message, 1),
        Err(IpcError::MessageTooLarge)
    ) {
        fifo_regression_failed("queue full");
        return;
    }
    serial::write_str("IPC FIFO regression: queue-full send rejected\n");

    let mut receiver_endpoint = IpcEndpoint::new(
        KernelObjectId::new(0xf102),
        "fifo-receiver-regression",
        ProcessId::empty(),
    );
    message[0] = b'a';
    if receiver_endpoint.enqueue(client_a, &message, 1).is_err() {
        fifo_regression_failed("receiver enqueue a");
        return;
    }
    if receiver_endpoint.has_message_for(client_a) {
        fifo_regression_failed("self message visible");
        return;
    }
    if !receiver_endpoint.has_message_for(client_b) {
        fifo_regression_failed("other receiver hidden");
        return;
    }
    message[0] = b'b';
    if receiver_endpoint.enqueue(client_b, &message, 1).is_err() {
        fifo_regression_failed("receiver enqueue b");
        return;
    }
    if !receiver_endpoint.has_message_for(client_a) || !receiver_endpoint.has_message_for(client_b)
    {
        fifo_regression_failed("blocked receiver eligibility");
        return;
    }
    if receiver_endpoint
        .dequeue_for(client_a)
        .map(|queued| queued.len == 1 && queued.bytes[0] == b'b')
        != Some(true)
    {
        fifo_regression_failed("receiver a eligible message");
        return;
    }
    if receiver_endpoint
        .dequeue_for(client_b)
        .map(|queued| queued.len == 1 && queued.bytes[0] == b'a')
        != Some(true)
    {
        fifo_regression_failed("receiver b eligible message");
        return;
    }
    serial::write_str(
        "IPC FIFO regression: receiver-specific dequeue preserves eligible ordering\n",
    );
    serial::write_str("IPC FIFO regression: multiple blocked receivers match eligible messages\n");
    serial::write_str("IPC FIFO regression ok\n");
}

fn fifo_regression_failed(reason: &str) {
    serial::write_str("IPC FIFO regression failed: ");
    serial::write_str(reason);
    serial::write_str("\n");
}
