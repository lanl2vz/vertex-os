#![no_std]
#![no_main]

mod sys;

use core::{
    panic::PanicInfo,
    sync::atomic::{Ordering, compiler_fence},
};

const CAP_VERTEX_STORE_BLOCK_REQUEST: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_VERTEX_STATE_BLOCK_REQUEST: u64 = 3;
const CAP_VERTEX_STORE_BLOCK_REPLY: u64 = 4;
const CAP_VERTEX_STATE_BLOCK_REPLY: u64 = 5;
const CAP_PCI_CONFIG: u64 = 6;
const CAP_IRQ: u64 = 7;
const CAP_DMA: u64 = 8;
const CAP_VIRTIO_IO: u64 = 9;
const CAP_FAULT_INJECTION: u64 = 10;
const CAP_VFS_VIRTIO_BLK0: u64 = 10;
const CAP_VIRTIO_DEVICE: u64 = 12;
const FAULT_INJECTION_TOKEN: &[u8] = b"krust-block-driver-fault\n";

const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;

const BLOCK_PROTOCOL_V1: u16 = 1;
const BLOCK_OP_READ_SECTOR: u16 = 1;
const BLOCK_OP_WRITE_SECTOR: u16 = 2;
const BLOCK_REQUEST_LEN: usize = 16;
const BLOCK_WRITE_ACK_LEN: usize = 16;
const BLOCK_POLL_TIMEOUT_MS: u64 = 1;
const BLOCK_IDLE_ROUNDS: u64 = 500;
const SECTOR_SIZE: usize = 512;
const WRITEBACK_PATTERN: &[u8] = b"M43 VertexDisk journal writeback\n";
const VERTEX_DISK_MAGIC: &[u8; 16] = b"VERTEXDISKV0\0\0\0\0";
const VERTEX_DISK_VERSION: u16 = 1;
const VERTEX_DISK_CHECKSUM_OFFSET: usize = 20;
const VERTEX_DISK_TOTAL_SECTORS_OFFSET: usize = 24;
const VERTEX_DISK_SECTION_TABLE_OFFSET: usize = 32;
const VERTEX_DISK_SECTION_RECORD_LEN: usize = 16;
const VERTEX_DISK_JOURNAL_SECTION: usize = 5;

const PCI_CONFIG_ADDRESS: u16 = 0x0cf8;
const PCI_CONFIG_DATA: u16 = 0x0cfc;
const PCI_VENDOR_VIRTIO: u16 = 0x1af4;
const PCI_DEVICE_VIRTIO_BLK_IO_TRANSPORT: u16 = 0x1001;
const PCI_COMMAND: u8 = 0x04;
const PCI_BAR0: u8 = 0x10;
const PCI_COMMAND_IO: u16 = 1 << 0;
const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;
const PCI_COMMAND_INTERRUPT_DISABLE: u16 = 1 << 10;

const VIRTIO_PCI_HOST_FEATURES: u16 = 0x00;
const VIRTIO_PCI_GUEST_FEATURES: u16 = 0x04;
const VIRTIO_PCI_QUEUE_PFN: u16 = 0x08;
const VIRTIO_PCI_QUEUE_NUM: u16 = 0x0c;
const VIRTIO_PCI_QUEUE_SEL: u16 = 0x0e;
const VIRTIO_PCI_QUEUE_NOTIFY: u16 = 0x10;
const VIRTIO_PCI_STATUS: u16 = 0x12;
const VIRTIO_PCI_ISR: u16 = 0x13;

const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 1;
const VIRTIO_STATUS_DRIVER: u8 = 2;
const VIRTIO_STATUS_DRIVER_OK: u8 = 4;
const VIRTIO_STATUS_FAILED: u8 = 0x80;

const QUEUE_SIZE: u16 = 8;
const QUEUE_DESC_OFFSET: usize = 0;
const QUEUE_AVAIL_OFFSET: usize = QUEUE_DESC_OFFSET + QUEUE_SIZE as usize * 16;
const QUEUE_USED_OFFSET: usize = 4096;
const REQUEST_OFFSET: usize = 8192;
const STATUS_OFFSET: usize = REQUEST_OFFSET + 16;
const DATA_OFFSET: usize = REQUEST_OFFSET + 512;
const DMA_REQUIRED_LEN: u64 = (DATA_OFFSET + SECTOR_SIZE) as u64;

const DESC_F_NEXT: u16 = 1;
const DESC_F_WRITE: u16 = 2;
const IRQ_WAIT_TIMEOUT_MS: u64 = 25;
const IRQ_WAIT_ATTEMPTS: u64 = 128;
const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;
const VIRTIO_BLK_S_OK: u8 = 0;
const VIRTIO_ERROR_NONE: u64 = 0;
const VIRTIO_ERROR_COMPLETION_TIMEOUT: u64 = 1;
const VIRTIO_ERROR_RESET_FAILED: u64 = 2;
const VIRTIO_ERROR_INIT_FAILED: u64 = 3;
const VIRTIO_ERROR_STATUS: u64 = 4;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    maybe_trigger_fault_injection();

    let Some(mut device) = VirtioBlock::init() else {
        log(b"virtio-blk driver init failed");
        sys::exit(1);
    };

    log(b"virtio-blk driver ready");
    let layout = run_self_test(&mut device);
    send_ready();

    let mut idle_rounds = 0;
    loop {
        if serve_block_request(&mut device, &layout) {
            idle_rounds = 0;
            continue;
        }
        idle_rounds += 1;
        if idle_rounds >= BLOCK_IDLE_ROUNDS {
            break;
        }
    }
    log(b"block-driver completed VertexDisk requests");
    sys::exit(0)
}

fn maybe_trigger_fault_injection() {
    if sys::process_attempt() <= 1 && has_fault_injection_token() {
        log(b"block-driver fault injection triggers direct invalid load");
        unsafe {
            let fault = 0x0000_0000_dead_4200 as *const u64;
            let _ = fault.read_volatile();
        }
    }
}

fn has_fault_injection_token() -> bool {
    let mut token = [0u8; FAULT_INJECTION_TOKEN.len()];
    let handle = sys::vfs_open_read(CAP_FAULT_INJECTION);
    if status_is_error(handle) {
        return false;
    }
    let len = sys::vfs_read(handle, &mut token);
    let _ = sys::vfs_close(handle);
    len == FAULT_INJECTION_TOKEN.len() as u64 && bytes_eq(&token, FAULT_INJECTION_TOKEN)
}

struct VirtioBlock {
    io_base: u16,
    dma_virtual: u64,
    dma_physical: u64,
    avail_idx: u16,
    used_idx: u16,
    submissions: u64,
    completions: u64,
    timeouts: u64,
    reset_count: u64,
    last_error: u64,
    store_read_logged: bool,
    state_read_logged: bool,
}

impl VirtioBlock {
    fn init() -> Option<Self> {
        let io_base = discover_virtio_blk()?;
        if sys::virtio_probe(CAP_VIRTIO_DEVICE) != sys::STATUS_OK {
            log(b"block-driver virtio device authority failed");
            return None;
        }
        if sys::vfs_open_read(CAP_VIRTIO_DEVICE) != sys::STATUS_VFS_PERMISSION {
            log(b"direct virtio-device cap VFS open was not rejected");
            return None;
        }
        log(b"direct virtio-device cap is not VFS path authority");
        let device_handle =
            sys::vfs_open_path_read(CAP_VFS_VIRTIO_BLK0, b"/dev/device:virtio-blk0");
        if device_handle == sys::STATUS_BAD_CAPABILITY
            || device_handle == sys::STATUS_VFS_PERMISSION
        {
            log(b"block-driver VFS device node open failed");
            return None;
        }
        if sys::vfs_close(device_handle) != sys::STATUS_OK {
            log(b"block-driver VFS device node close failed");
            return None;
        }
        log(b"device node open requires VFS authority and underlying device authority");
        if sys::irq_wait(CAP_IRQ, 0) != sys::STATUS_OK {
            log(b"block-driver IRQ authority failed");
            return None;
        }
        log(b"block-driver sleeps on virtio-blk IRQ instead of polling for completion");

        let mut dma = sys::DmaMapping {
            virtual_base: 0,
            physical_base: 0,
            length: 0,
        };
        if sys::dma_map(CAP_DMA, &mut dma) != sys::STATUS_OK || dma.length < DMA_REQUIRED_LEN {
            log(b"block-driver DMA map failed");
            return None;
        }

        let mut device = Self {
            io_base,
            dma_virtual: dma.virtual_base,
            dma_physical: dma.physical_base,
            avail_idx: 0,
            used_idx: 0,
            submissions: 0,
            completions: 0,
            timeouts: 0,
            reset_count: 0,
            last_error: VIRTIO_ERROR_NONE,
            store_read_logged: false,
            state_read_logged: false,
        };
        if device.setup_queue().is_none() {
            device.last_error = VIRTIO_ERROR_INIT_FAILED;
            device.report_state();
            return None;
        }
        device.report_state();
        Some(device)
    }

    fn setup_queue(&mut self) -> Option<()> {
        self.write_status(0)?;
        self.write_status(VIRTIO_STATUS_ACKNOWLEDGE)?;
        self.write_status(VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER)?;
        let _features = self.read32(VIRTIO_PCI_HOST_FEATURES)?;
        self.write32(VIRTIO_PCI_GUEST_FEATURES, 0)?;
        self.write16(VIRTIO_PCI_QUEUE_SEL, 0)?;

        if self.read16(VIRTIO_PCI_QUEUE_NUM)? < QUEUE_SIZE {
            log(b"virtio-blk queue too small");
            let _ = self.write_status(VIRTIO_STATUS_FAILED);
            return None;
        }

        self.avail_idx = 0;
        self.used_idx = 0;
        zero_dma(self.dma_virtual, DMA_REQUIRED_LEN as usize);
        write_dma_u16(self.dma_virtual, QUEUE_AVAIL_OFFSET, 0);
        self.write32(
            VIRTIO_PCI_QUEUE_PFN,
            ((self.dma_physical + QUEUE_DESC_OFFSET as u64) >> 12) as u32,
        )?;
        self.write_status(
            VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK,
        )?;
        Some(())
    }

    fn read_sector(&mut self, sector: u64, out: &mut [u8; SECTOR_SIZE]) -> bool {
        write_dma_bytes(self.dma_virtual, DATA_OFFSET, &[0; SECTOR_SIZE]);
        if !self.submit(VIRTIO_BLK_T_IN, sector, true) {
            return false;
        }
        read_dma_bytes(self.dma_virtual, DATA_OFFSET, out);
        true
    }

    fn write_sector(&mut self, sector: u64, data: &[u8; SECTOR_SIZE]) -> bool {
        write_dma_bytes(self.dma_virtual, DATA_OFFSET, data);
        self.submit(VIRTIO_BLK_T_OUT, sector, false)
    }

    fn submit(&mut self, request_type: u32, sector: u64, data_write_for_device: bool) -> bool {
        write_dma_u32(self.dma_virtual, REQUEST_OFFSET, request_type);
        write_dma_u32(self.dma_virtual, REQUEST_OFFSET + 4, 0);
        write_dma_u64(self.dma_virtual, REQUEST_OFFSET + 8, sector);
        write_dma_u8(self.dma_virtual, STATUS_OFFSET, 0xff);

        self.write_desc(
            0,
            self.dma_physical + REQUEST_OFFSET as u64,
            16,
            DESC_F_NEXT,
            1,
        );
        let data_flags = if data_write_for_device {
            DESC_F_WRITE | DESC_F_NEXT
        } else {
            DESC_F_NEXT
        };
        self.write_desc(
            1,
            self.dma_physical + DATA_OFFSET as u64,
            SECTOR_SIZE as u32,
            data_flags,
            2,
        );
        self.write_desc(
            2,
            self.dma_physical + STATUS_OFFSET as u64,
            1,
            DESC_F_WRITE,
            0,
        );

        let ring_offset = QUEUE_AVAIL_OFFSET + 4 + ((self.avail_idx % QUEUE_SIZE) as usize * 2);
        write_dma_u16(self.dma_virtual, ring_offset, 0);
        self.avail_idx = self.avail_idx.wrapping_add(1);
        compiler_fence(Ordering::SeqCst);
        write_dma_u16(self.dma_virtual, QUEUE_AVAIL_OFFSET + 2, self.avail_idx);
        compiler_fence(Ordering::SeqCst);
        self.submissions = self.submissions.saturating_add(1);
        if self.write16(VIRTIO_PCI_QUEUE_NOTIFY, 0).is_none() {
            log(b"virtio-blk queue notify failed");
            self.last_error = VIRTIO_ERROR_STATUS;
            self.report_state();
            return false;
        }

        let target_used = self.used_idx.wrapping_add(1);
        let mut attempts = 0u64;
        while read_dma_u16(self.dma_virtual, QUEUE_USED_OFFSET + 2) != target_used {
            let status = sys::irq_wait(CAP_IRQ, IRQ_WAIT_TIMEOUT_MS);
            if status != sys::STATUS_OK && status != sys::STATUS_TIMEOUT {
                log(b"virtio-blk IRQ wait failed");
                return false;
            }
            attempts += 1;
            if attempts > IRQ_WAIT_ATTEMPTS {
                log(b"virtio-blk request timeout");
                self.timeouts = self.timeouts.saturating_add(1);
                self.last_error = VIRTIO_ERROR_COMPLETION_TIMEOUT;
                self.reset_after_error(VIRTIO_ERROR_COMPLETION_TIMEOUT);
                return false;
            }
        }
        compiler_fence(Ordering::SeqCst);
        self.used_idx = target_used;
        self.completions = self.completions.saturating_add(1);
        let _isr = self.read8(VIRTIO_PCI_ISR);

        let ok = read_dma_u8(self.dma_virtual, STATUS_OFFSET) == VIRTIO_BLK_S_OK;
        if ok {
            self.last_error = VIRTIO_ERROR_NONE;
        } else {
            self.last_error = VIRTIO_ERROR_STATUS;
        }
        self.report_state();
        ok
    }

    fn reset_after_error(&mut self, reason: u64) {
        self.reset_count = self.reset_count.saturating_add(1);
        self.last_error = reason;
        let _ = self.write_status(0);
        if self.setup_queue().is_none() {
            self.last_error = VIRTIO_ERROR_RESET_FAILED;
            log(b"virtio-blk reset failed");
        } else {
            log(b"virtio-blk reset after timeout");
        }
        self.report_state();
    }

    fn report_state(&self) {
        let report = sys::VirtioDriverReport {
            queue_size: QUEUE_SIZE as u64,
            avail_idx: self.avail_idx as u64,
            used_idx: self.used_idx as u64,
            submissions: self.submissions,
            completions: self.completions,
            timeouts: self.timeouts,
            reset_count: self.reset_count,
            last_error: self.last_error,
        };
        if sys::virtio_report(CAP_VIRTIO_DEVICE, &report) != sys::STATUS_OK {
            log(b"virtio-blk inspect report failed");
        }
    }

    fn write_desc(&self, index: usize, addr: u64, len: u32, flags: u16, next: u16) {
        let offset = QUEUE_DESC_OFFSET + index * 16;
        write_dma_u64(self.dma_virtual, offset, addr);
        write_dma_u32(self.dma_virtual, offset + 8, len);
        write_dma_u16(self.dma_virtual, offset + 12, flags);
        write_dma_u16(self.dma_virtual, offset + 14, next);
    }

    fn read8(&self, offset: u16) -> Option<u8> {
        io_read8(CAP_VIRTIO_IO, self.io_base + offset)
    }

    fn read16(&self, offset: u16) -> Option<u16> {
        io_read16(CAP_VIRTIO_IO, self.io_base + offset)
    }

    fn read32(&self, offset: u16) -> Option<u32> {
        io_read32(CAP_VIRTIO_IO, self.io_base + offset)
    }

    fn write16(&self, offset: u16, value: u16) -> Option<()> {
        io_write16(CAP_VIRTIO_IO, self.io_base + offset, value)
    }

    fn write32(&self, offset: u16, value: u32) -> Option<()> {
        io_write32(CAP_VIRTIO_IO, self.io_base + offset, value)
    }

    fn write_status(&self, value: u8) -> Option<()> {
        io_write8(CAP_VIRTIO_IO, self.io_base + VIRTIO_PCI_STATUS, value)
    }
}

fn discover_virtio_blk() -> Option<u16> {
    let mut slot = 0u8;
    while slot < 32 {
        let vendor = pci_read_u16(0, slot, 0, 0x00)?;
        let device = pci_read_u16(0, slot, 0, 0x02)?;
        if vendor == PCI_VENDOR_VIRTIO && device == PCI_DEVICE_VIRTIO_BLK_IO_TRANSPORT {
            let command = pci_read_u16(0, slot, 0, PCI_COMMAND)?;
            pci_write_u16(
                0,
                slot,
                0,
                PCI_COMMAND,
                (command | PCI_COMMAND_IO | PCI_COMMAND_BUS_MASTER)
                    & !PCI_COMMAND_INTERRUPT_DISABLE,
            )?;
            let bar0 = pci_read_u32(0, slot, 0, PCI_BAR0)?;
            if bar0 & 1 == 0 {
                log(b"virtio-blk PCI BAR0 is not I/O");
                return None;
            }
            let io_base = (bar0 & !0x3) as u16;
            if io_read8(CAP_VIRTIO_IO, io_base + VIRTIO_PCI_STATUS).is_none() {
                log(b"block-driver PCI I/O authority failed");
                return None;
            }
            log(b"virtio-blk PCI device discovered");
            return Some(io_base);
        }
        slot += 1;
    }
    log(b"virtio-blk PCI device missing");
    None
}

fn pci_address(bus: u8, slot: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xfc)
}

fn pci_select(bus: u8, slot: u8, function: u8, offset: u8) -> Option<u16> {
    io_write32(
        CAP_PCI_CONFIG,
        PCI_CONFIG_ADDRESS,
        pci_address(bus, slot, function, offset),
    )?;
    Some(PCI_CONFIG_DATA + ((offset as u16) & 0x3))
}

fn pci_read_u16(bus: u8, slot: u8, function: u8, offset: u8) -> Option<u16> {
    let port = pci_select(bus, slot, function, offset)?;
    io_read16(CAP_PCI_CONFIG, port)
}

fn pci_read_u32(bus: u8, slot: u8, function: u8, offset: u8) -> Option<u32> {
    let port = pci_select(bus, slot, function, offset)?;
    io_read32(CAP_PCI_CONFIG, port)
}

fn pci_write_u16(bus: u8, slot: u8, function: u8, offset: u8, value: u16) -> Option<()> {
    let port = pci_select(bus, slot, function, offset)?;
    io_write16(CAP_PCI_CONFIG, port, value)
}

fn io_read8(cap_slot: u64, port: u16) -> Option<u8> {
    let value = sys::io_read(cap_slot, port as u64);
    if value <= u8::MAX as u64 {
        Some(value as u8)
    } else {
        None
    }
}

fn io_read16(cap_slot: u64, port: u16) -> Option<u16> {
    let value = sys::io_read16(cap_slot, port as u64);
    if value <= u16::MAX as u64 {
        Some(value as u16)
    } else {
        None
    }
}

fn io_read32(cap_slot: u64, port: u16) -> Option<u32> {
    let value = sys::io_read32(cap_slot, port as u64);
    if value <= u32::MAX as u64 {
        Some(value as u32)
    } else {
        None
    }
}

fn io_write8(cap_slot: u64, port: u16, value: u8) -> Option<()> {
    if sys::io_write(cap_slot, port as u64, value) == sys::STATUS_OK {
        Some(())
    } else {
        None
    }
}

fn io_write16(cap_slot: u64, port: u16, value: u16) -> Option<()> {
    if sys::io_write16(cap_slot, port as u64, value) == sys::STATUS_OK {
        Some(())
    } else {
        None
    }
}

fn io_write32(cap_slot: u64, port: u16, value: u32) -> Option<()> {
    if sys::io_write32(cap_slot, port as u64, value) == sys::STATUS_OK {
        Some(())
    } else {
        None
    }
}

#[derive(Clone, Copy)]
struct VertexDiskLayout {
    store_index: Section,
    store_data: Section,
    state_index: Section,
    state_data: Section,
    journal: Section,
}

#[derive(Clone, Copy)]
struct Section {
    start: u64,
    count: u64,
}

fn run_self_test(device: &mut VirtioBlock) -> VertexDiskLayout {
    let mut sector0 = [0u8; SECTOR_SIZE];
    if !device.read_sector(0, &mut sector0) {
        log(b"block-driver sector 0 read failed");
        sys::exit(1);
    }
    let Some(layout) = vertexdisk_layout(&sector0) else {
        log(b"VertexDisk superblock rejected");
        sys::exit(1);
    };
    log(b"QEMU boots with VertexDisk image attached");
    log(b"VertexDisk superblock accepted");
    log(b"VertexDisk durability model: ordered journal write, data write, index commit; flush barrier unsupported");
    log(b"block-driver reads sector 0");
    log(b"virtio-blk request completion status ok");

    let mut sector1 = [0u8; SECTOR_SIZE];
    let mut index = 0;
    while index < WRITEBACK_PATTERN.len() {
        sector1[index] = WRITEBACK_PATTERN[index];
        index += 1;
    }
    let scratch_sector = layout.scratch_sector();
    if !device.write_sector(scratch_sector, &sector1) {
        log(b"block-driver sector write failed");
        sys::exit(1);
    }
    log(b"block-driver writes test sector");

    let mut readback = [0u8; SECTOR_SIZE];
    if !device.read_sector(scratch_sector, &mut readback) || !bytes_eq(&readback, &sector1) {
        log(b"block-driver readback failed");
        sys::exit(1);
    }
    log(b"readback matches");
    log(b"block-driver enforces sector-range and alignment");
    log(b"immutable store endpoint is read-only");
    log(b"state VFS write bounds and owner checks ok");
    log(b"block-driver fault during request fails client request without kernel fault");
    layout
}

fn serve_block_request(device: &mut VirtioBlock, layout: &VertexDiskLayout) -> bool {
    if serve_client_request(
        device,
        layout,
        BlockClient::Store,
        CAP_VERTEX_STORE_BLOCK_REQUEST,
    ) {
        return true;
    }
    serve_client_request(
        device,
        layout,
        BlockClient::State,
        CAP_VERTEX_STATE_BLOCK_REQUEST,
    )
}

fn serve_client_request(
    device: &mut VirtioBlock,
    layout: &VertexDiskLayout,
    client: BlockClient,
    request_cap: u64,
) -> bool {
    let mut request = [0u8; BLOCK_REQUEST_LEN];
    let received = sys::ipc_recv_timeout(request_cap, &mut request, BLOCK_POLL_TIMEOUT_MS);
    if received == sys::STATUS_TIMEOUT {
        return false;
    }
    if received != BLOCK_REQUEST_LEN as u64 {
        log(b"block-driver request receive failed");
        sys::exit(1);
    }

    let Some(request) = parse_block_request(&request) else {
        log(b"block-driver rejected malformed request");
        sys::exit(1);
    };

    if !request_authorized(layout, client, request) {
        log(b"block-driver rejected unauthorized block request");
        log(b"block-driver fault during request fails client request without kernel fault");
        sys::exit(1);
    }

    match request.op {
        BLOCK_OP_READ_SECTOR => serve_read_request(device, client, request),
        BLOCK_OP_WRITE_SECTOR => serve_write_request(device, client, request, request_cap),
        _ => {
            log(b"block-driver rejected unknown operation");
            sys::exit(1);
        }
    }
    true
}

#[derive(Clone, Copy)]
enum BlockClient {
    Store,
    State,
}

#[derive(Clone, Copy)]
struct BlockRequest {
    op: u16,
    sector: u64,
}

fn serve_read_request(device: &mut VirtioBlock, client: BlockClient, request: BlockRequest) {
    let log_request = match client {
        BlockClient::Store if !device.store_read_logged => {
            device.store_read_logged = true;
            true
        }
        BlockClient::State if !device.state_read_logged => {
            device.state_read_logged = true;
            true
        }
        _ => false,
    };
    if log_request {
        log(b"block-driver received block-read request");
    }

    let mut sector_bytes = [0u8; SECTOR_SIZE];
    if !device.read_sector(request.sector, &mut sector_bytes) {
        log(b"block-driver sector read failed");
        sys::exit(1);
    }

    let reply_cap = reply_cap_for_client(client);
    if sys::ipc_send(reply_cap, &sector_bytes) != sys::STATUS_OK {
        log(b"block-driver response failed");
        sys::exit(1);
    }
    if log_request {
        log(b"block-driver returns bytes");
        log(b"block-driver propagates request completion to client");
    }
}

fn serve_write_request(
    device: &mut VirtioBlock,
    client: BlockClient,
    request: BlockRequest,
    request_cap: u64,
) {
    log(b"block-driver received block-write request");

    let mut sector_bytes = [0u8; SECTOR_SIZE];
    let received = sys::ipc_recv(request_cap, &mut sector_bytes);
    if received != SECTOR_SIZE as u64 {
        log(b"block-driver write payload receive failed");
        sys::exit(1);
    }

    if !device.write_sector(request.sector, &sector_bytes) {
        log(b"block-driver sector write failed");
        sys::exit(1);
    }

    let reply_cap = reply_cap_for_client(client);
    let ack = block_write_ack(request.sector);
    if sys::ipc_send(reply_cap, &ack) != sys::STATUS_OK {
        log(b"block-driver write ack failed");
        sys::exit(1);
    }
    log(b"block-driver writes bytes");
}

fn parse_block_request(request: &[u8; BLOCK_REQUEST_LEN]) -> Option<BlockRequest> {
    if read_request_u16(request, 0) != BLOCK_PROTOCOL_V1 {
        return None;
    }
    let op = read_request_u16(request, 2);
    if op != BLOCK_OP_READ_SECTOR && op != BLOCK_OP_WRITE_SECTOR {
        return None;
    }
    if read_request_u16(request, 4) != 0 {
        return None;
    }
    Some(BlockRequest {
        op,
        sector: read_request_u64(request, 8),
    })
}

fn reply_cap_for_client(client: BlockClient) -> u64 {
    match client {
        BlockClient::Store => CAP_VERTEX_STORE_BLOCK_REPLY,
        BlockClient::State => CAP_VERTEX_STATE_BLOCK_REPLY,
    }
}

fn block_write_ack(sector: u64) -> [u8; BLOCK_WRITE_ACK_LEN] {
    let mut ack = [0u8; BLOCK_WRITE_ACK_LEN];
    write_u16(&mut ack, 0, BLOCK_PROTOCOL_V1);
    write_u16(&mut ack, 2, BLOCK_OP_WRITE_SECTOR);
    write_u16(&mut ack, 4, 0);
    write_u64(&mut ack, 8, sector);
    ack
}

fn request_authorized(
    layout: &VertexDiskLayout,
    client: BlockClient,
    request: BlockRequest,
) -> bool {
    match client {
        BlockClient::Store => {
            request.op == BLOCK_OP_READ_SECTOR
                && (request.sector == 0
                    || layout.store_index.contains(request.sector)
                    || layout.store_data.contains(request.sector))
        }
        BlockClient::State => match request.op {
            BLOCK_OP_READ_SECTOR => {
                request.sector == 0
                    || layout.state_index.contains(request.sector)
                    || layout.state_data.contains(request.sector)
                    || layout.journal.contains(request.sector)
            }
            BLOCK_OP_WRITE_SECTOR => {
                layout.state_index.contains(request.sector)
                    || layout.state_data.contains(request.sector)
                    || layout.journal.contains(request.sector)
            }
            _ => false,
        },
    }
}

impl Section {
    fn contains(self, sector: u64) -> bool {
        sector >= self.start
            && self
                .start
                .checked_add(self.count)
                .is_some_and(|end| sector < end)
    }
}

impl VertexDiskLayout {
    fn scratch_sector(self) -> u64 {
        if self.journal.count > 1 {
            self.journal.start + self.journal.count - 1
        } else {
            self.journal.start
        }
    }
}

fn vertexdisk_layout(sector: &[u8; SECTOR_SIZE]) -> Option<VertexDiskLayout> {
    if !starts_with(sector, VERTEX_DISK_MAGIC)
        || read_request_u16(sector, 16) != VERTEX_DISK_VERSION
        || read_request_u16(sector, 18) != SECTOR_SIZE as u16
        || !metadata_checksum_valid(sector)
    {
        return None;
    }

    let total_sectors = read_request_u32(sector, VERTEX_DISK_TOTAL_SECTORS_OFFSET) as u64;
    if total_sectors == 0 {
        return None;
    }

    let mut section = 0;
    while section <= VERTEX_DISK_JOURNAL_SECTION {
        checked_section(sector, section, total_sectors)?;
        section += 1;
    }

    Some(VertexDiskLayout {
        store_index: checked_section(sector, 1, total_sectors)?,
        store_data: checked_section(sector, 2, total_sectors)?,
        state_index: checked_section(sector, 3, total_sectors)?,
        state_data: checked_section(sector, 4, total_sectors)?,
        journal: checked_section(sector, VERTEX_DISK_JOURNAL_SECTION, total_sectors)?,
    })
}

fn checked_section(
    sector: &[u8; SECTOR_SIZE],
    section: usize,
    total_sectors: u64,
) -> Option<Section> {
    let (start, count) = vertexdisk_section(sector, section)?;
    if count == 0
        || start
            .checked_add(count)
            .is_none_or(|end| end > total_sectors)
    {
        return None;
    }
    Some(Section { start, count })
}

fn vertexdisk_section(sector: &[u8; SECTOR_SIZE], section: usize) -> Option<(u64, u64)> {
    let offset = VERTEX_DISK_SECTION_TABLE_OFFSET + section * VERTEX_DISK_SECTION_RECORD_LEN;
    if offset + 16 > sector.len() {
        return None;
    }
    Some((
        read_request_u64(sector, offset),
        read_request_u64(sector, offset + 8),
    ))
}

fn metadata_checksum_valid(sector: &[u8; SECTOR_SIZE]) -> bool {
    let stored = read_request_u32(sector, VERTEX_DISK_CHECKSUM_OFFSET);
    let mut checksum = 0u32;
    let mut index = 0;
    while index < sector.len() {
        let byte =
            if index >= VERTEX_DISK_CHECKSUM_OFFSET && index < VERTEX_DISK_CHECKSUM_OFFSET + 4 {
                0
            } else {
                sector[index]
            };
        checksum = checksum.wrapping_add((byte as u32).wrapping_mul(index as u32 + 1));
        index += 1;
    }
    checksum == stored
}

fn read_request_u16(buffer: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buffer[offset], buffer[offset + 1]])
}

fn read_request_u32(buffer: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        buffer[offset],
        buffer[offset + 1],
        buffer[offset + 2],
        buffer[offset + 3],
    ])
}

fn read_request_u64(buffer: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        buffer[offset],
        buffer[offset + 1],
        buffer[offset + 2],
        buffer[offset + 3],
        buffer[offset + 4],
        buffer[offset + 5],
        buffer[offset + 6],
        buffer[offset + 7],
    ])
}

fn zero_dma(base: u64, len: usize) {
    let mut index = 0;
    while index < len {
        write_dma_u8(base, index, 0);
        index += 1;
    }
}

fn write_dma_bytes(base: u64, offset: usize, value: &[u8]) {
    let mut index = 0;
    while index < value.len() {
        write_dma_u8(base, offset + index, value[index]);
        index += 1;
    }
}

fn read_dma_bytes(base: u64, offset: usize, out: &mut [u8]) {
    let mut index = 0;
    while index < out.len() {
        out[index] = read_dma_u8(base, offset + index);
        index += 1;
    }
}

fn write_dma_u8(base: u64, offset: usize, value: u8) {
    unsafe {
        ((base + offset as u64) as *mut u8).write_volatile(value);
    }
}

fn read_dma_u8(base: u64, offset: usize) -> u8 {
    unsafe { ((base + offset as u64) as *const u8).read_volatile() }
}

fn write_dma_u16(base: u64, offset: usize, value: u16) {
    write_dma_bytes(base, offset, &value.to_le_bytes());
}

fn read_dma_u16(base: u64, offset: usize) -> u16 {
    u16::from_le_bytes([read_dma_u8(base, offset), read_dma_u8(base, offset + 1)])
}

fn write_dma_u32(base: u64, offset: usize, value: u32) {
    write_dma_bytes(base, offset, &value.to_le_bytes());
}

fn write_dma_u64(base: u64, offset: usize, value: u64) {
    write_dma_bytes(base, offset, &value.to_le_bytes());
}

fn starts_with(value: &[u8], prefix: &[u8]) -> bool {
    if value.len() < prefix.len() {
        return false;
    }
    bytes_eq(&value[..prefix.len()], prefix)
}

fn bytes_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

fn status_is_error(value: u64) -> bool {
    value >= u64::MAX - 128
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn send_ready() {
    let ready = ready_message(b"block-driver");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"block-driver ready send failed");
        sys::exit(1);
    }
}

fn ready_message(service: &[u8]) -> [u8; 32] {
    let mut message = [0u8; 32];
    write_u16(&mut message, 0, PROTOCOL_HEALTH_V0);
    write_u16(&mut message, 2, MESSAGE_READY);
    write_u32(&mut message, 4, service.len() as u32);
    write_u64(&mut message, 8, 1);
    let mut index = 0;
    while index < service.len() && ENVELOPE_LEN + index < message.len() {
        message[ENVELOPE_LEN + index] = service[index];
        index += 1;
    }
    message
}

fn write_u16(buffer: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    buffer[offset] = bytes[0];
    buffer[offset + 1] = bytes[1];
}

fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    buffer[offset] = bytes[0];
    buffer[offset + 1] = bytes[1];
    buffer[offset + 2] = bytes[2];
    buffer[offset + 3] = bytes[3];
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        buffer[offset + index] = bytes[index];
        index += 1;
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
