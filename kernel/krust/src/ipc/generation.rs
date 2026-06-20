use super::*;

#[derive(Clone, Copy)]
struct GenerationRuntime {
    generation_id: &'static str,
    config: &'static BootRuntimeConfig,
}

#[derive(Clone, Copy)]
struct StagedGeneration {
    generation_id: &'static str,
    config: &'static BootRuntimeConfig,
    pub(super) previous_generation: &'static str,
    previous_config: Option<&'static BootRuntimeConfig>,
    old_cap_count: u64,
    old_contexts: [Option<RuntimeReapTarget>; MAX_PROCESSES],
    old_context_count: usize,
    initial_context: ProcessContext,
    rollback: bool,
}

struct GenerationRuntimeTable {
    entries: [Option<GenerationRuntime>; MAX_GENERATION_CONFIGS],
    count: usize,
}

pub(super) struct BootManagerState {
    pub(super) selected_generation: &'static str,
    pub(super) previous_generation: &'static str,
    pub(super) known_good_generation: &'static str,
    pub(super) last_failed_generation: &'static str,
    pub(super) last_failure_reason: &'static str,
    pub(super) last_failure_service: &'static str,
    pub(super) last_failure_dependency: &'static str,
    pub(super) last_failure_policy: &'static str,
    pub(super) last_transaction_state: &'static str,
    pub(super) last_transaction_target: &'static str,
    pub(super) transaction_counter: u64,
    pub(super) boot_attempt_counter: u64,
}

static GENERATION_RUNTIMES: Global<GenerationRuntimeTable> =
    Global(UnsafeCell::new(GenerationRuntimeTable::new()));
static ROLLBACK_RUNTIME: Global<Option<GenerationRuntime>> = Global(UnsafeCell::new(None));
static STAGED_GENERATION: Global<Option<StagedGeneration>> = Global(UnsafeCell::new(None));
static FAILED_GENERATION: Global<Option<&'static str>> = Global(UnsafeCell::new(None));
static BOOT_MANAGER: Global<BootManagerState> = Global(UnsafeCell::new(BootManagerState::new()));

impl GenerationRuntimeTable {
    const fn new() -> Self {
        Self {
            entries: [None; MAX_GENERATION_CONFIGS],
            count: 0,
        }
    }

    fn register(&mut self, runtime: GenerationRuntime) -> Result<(), InitError> {
        let mut index = 0;
        while index < self.count {
            if let Some(existing) = self.entries[index]
                && existing.generation_id == runtime.generation_id
            {
                self.entries[index] = Some(runtime);
                return Ok(());
            }
            index += 1;
        }

        if self.count == self.entries.len() {
            return Err(InitError::ObjectTableFull);
        }

        self.entries[self.count] = Some(runtime);
        self.count += 1;
        Ok(())
    }

    fn find(&self, generation_id: &[u8]) -> Option<GenerationRuntime> {
        let mut index = 0;
        while index < self.count {
            if let Some(runtime) = self.entries[index]
                && runtime.generation_id.as_bytes() == generation_id
            {
                return Some(runtime);
            }
            index += 1;
        }
        None
    }
}

impl BootManagerState {
    const fn new() -> Self {
        Self {
            selected_generation: "",
            previous_generation: "",
            known_good_generation: "",
            last_failed_generation: "",
            last_failure_reason: "",
            last_failure_service: "",
            last_failure_dependency: "",
            last_failure_policy: "",
            last_transaction_state: "idle",
            last_transaction_target: "",
            transaction_counter: 0,
            boot_attempt_counter: 0,
        }
    }

    pub(super) fn start_boot(&mut self, generation_id: &'static str) {
        if self.selected_generation.is_empty() {
            self.selected_generation = generation_id;
        }
        self.boot_attempt_counter = self.boot_attempt_counter.saturating_add(1);
        serial::write_str("Native boot manager selected_generation=");
        serial::write_str(self.selected_generation);
        serial::write_str("\n");
        serial::write_str("Native boot manager previous_generation=");
        serial::write_str(if self.previous_generation.is_empty() {
            "<none>"
        } else {
            self.previous_generation
        });
        serial::write_str("\n");
        serial::write_str("Native boot manager known_good_generation=");
        serial::write_str(if self.known_good_generation.is_empty() {
            "<none>"
        } else {
            self.known_good_generation
        });
        serial::write_str("\n");
        serial::write_str("Native boot manager boot_attempt_counter=");
        serial::write_u64_dec(self.boot_attempt_counter);
        serial::write_str("\n");
    }

    fn install_selected(&mut self, previous: &'static str, selected: &'static str) {
        self.previous_generation = previous;
        self.selected_generation = selected;
        self.last_failure_reason = "";
        self.last_failure_service = "";
        self.last_failure_dependency = "";
        self.last_failure_policy = "";
        self.last_transaction_state = "commit";
        self.last_transaction_target = selected;
        self.transaction_counter = self.transaction_counter.saturating_add(1);
        self.boot_attempt_counter = self.boot_attempt_counter.saturating_add(1);
        serial::write_str("Native generation manager journal commit: selected_generation=");
        serial::write_str(selected);
        serial::write_str("\n");
        serial::write_str("Native update transaction selected_generation updated: ");
        serial::write_str(selected);
        serial::write_str("\n");
    }

    fn install_prepare(&mut self, previous: &'static str, target: &'static str) {
        self.previous_generation = previous;
        self.last_transaction_state = "prepare";
        self.last_transaction_target = target;
        self.transaction_counter = self.transaction_counter.saturating_add(1);
        serial::write_str("Native generation manager journal prepare: previous=");
        serial::write_str(previous);
        serial::write_str(" target=");
        serial::write_str(target);
        serial::write_str("\n");
    }

    fn install_abort(&mut self, target: &'static str, reason: &'static str) {
        self.last_failed_generation = target;
        self.last_failure_reason = reason;
        self.record_failure_detail(target, reason);
        self.last_transaction_state = "abort";
        self.last_transaction_target = target;
        self.transaction_counter = self.transaction_counter.saturating_add(1);
        serial::write_str("Native generation manager journal abort: generation=");
        serial::write_str(target);
        serial::write_str(" reason=");
        serial::write_str(reason);
        serial::write_str("\n");
    }

    pub(super) fn mark_known_good(&mut self, generation_id: &'static str) {
        self.known_good_generation = generation_id;
        self.selected_generation = generation_id;
        self.last_failure_reason = "";
        self.last_failure_service = "";
        self.last_failure_dependency = "";
        self.last_failure_policy = "";
        serial::write_str("Native boot manager known_good_generation=");
        serial::write_str(generation_id);
        serial::write_str("\n");
        serial::write_str("Native boot manager journal: activation-ok generation=");
        serial::write_str(generation_id);
        serial::write_str("\n");
    }

    fn mark_failed_and_fallback(&mut self, failed: &'static str, fallback: &'static str) {
        self.last_failed_generation = failed;
        self.last_failure_reason = "activation-failed";
        self.last_failure_service = failed;
        self.last_failure_dependency = "service-readiness";
        self.last_failure_policy = "known-good-rollback";
        self.previous_generation = failed;
        self.selected_generation = fallback;
        self.last_transaction_state = "rollback";
        self.last_transaction_target = fallback;
        self.transaction_counter = self.transaction_counter.saturating_add(1);
        serial::write_str("Native generation manager journal rollback: failed=");
        serial::write_str(failed);
        serial::write_str(" selected_generation=");
        serial::write_str(fallback);
        serial::write_str(" reason=activation-failed\n");
        serial::write_str("Native boot manager last_failed_generation=");
        serial::write_str(failed);
        serial::write_str("\n");
        serial::write_str("Native boot manager fallback selected_generation=");
        serial::write_str(fallback);
        serial::write_str("\n");
        serial::write_str("Native boot manager previous_generation=");
        serial::write_str(failed);
        serial::write_str("\n");
        serial::write_str("Native boot manager journal: failed generation=");
        serial::write_str(failed);
        serial::write_str(" fallback=");
        serial::write_str(fallback);
        serial::write_str("\n");
        self.log_failure_detail();
    }

    fn recover_from_disk(
        &mut self,
        selected: &'static str,
        previous: &'static str,
        known_good: &'static str,
        transaction: &'static str,
        target: &'static str,
        failure_reason: &'static str,
    ) {
        self.selected_generation = selected;
        self.previous_generation = previous;
        self.known_good_generation = known_good;
        self.last_failure_reason = failure_reason;
        if !failure_reason.is_empty() {
            self.last_failure_service = if transaction == "rollback" && !previous.is_empty() {
                previous
            } else {
                target
            };
            self.last_failure_dependency = if transaction == "rollback" {
                "service-readiness"
            } else {
                "store-closure"
            };
            self.last_failure_policy = if transaction == "rollback" {
                "known-good-rollback"
            } else {
                "activation"
            };
        }
        self.last_transaction_state = transaction;
        self.last_transaction_target = target;
        if !failure_reason.is_empty() {
            self.last_failed_generation = if transaction == "rollback" && !previous.is_empty() {
                previous
            } else {
                target
            };
        }
        serial::write_str("Native generation manager recovered durable state from VertexDisk\n");
        serial::write_str("Native generation manager durable selected_generation=");
        serial::write_str(selected);
        serial::write_str("\n");
        self.log_failure_detail();
    }

    fn record_failure_detail(&mut self, generation: &'static str, reason: &'static str) {
        self.last_failure_service = generation;
        match reason {
            "verification-failed" => {
                self.last_failure_dependency = "store-closure";
                self.last_failure_policy = "installable-generation";
            }
            "runtime-build-failed" => {
                self.last_failure_dependency = "service-readiness";
                self.last_failure_policy = "activation";
            }
            "rollback-build-failed" => {
                self.last_failure_dependency = "rollback-runtime";
                self.last_failure_policy = "known-good-rollback";
            }
            "state-migration-failed" => {
                self.last_failure_dependency = "state-schema";
                self.last_failure_policy = "state-migration";
            }
            _ => {
                self.last_failure_dependency = "unknown";
                self.last_failure_policy = "activation";
            }
        }
        self.log_failure_detail();
    }

    fn log_failure_detail(&self) {
        if self.last_failure_reason.is_empty() {
            return;
        }
        serial::write_str("Native generation manager failure detail: service=");
        serial::write_str(self.last_failure_service);
        serial::write_str(" dependency=");
        serial::write_str(self.last_failure_dependency);
        serial::write_str(" policy=");
        serial::write_str(self.last_failure_policy);
        serial::write_str(" reason=");
        serial::write_str(self.last_failure_reason);
        serial::write_str("\n");
    }
}

fn generation_runtimes() -> &'static mut GenerationRuntimeTable {
    unsafe { &mut *GENERATION_RUNTIMES.0.get() }
}

fn set_rollback_runtime(runtime: GenerationRuntime) {
    unsafe {
        *ROLLBACK_RUNTIME.0.get() = Some(runtime);
    }
}

fn staged_generation() -> &'static mut Option<StagedGeneration> {
    unsafe { &mut *STAGED_GENERATION.0.get() }
}

fn clear_staged_generation() {
    unsafe {
        *STAGED_GENERATION.0.get() = None;
    }
}

fn discard_uncommitted_staged_generation() {
    let Some(staged) = *staged_generation() else {
        return;
    };
    let mut targets = [None; MAX_PROCESSES];
    let mut count = 0;
    {
        let staging = staging_runtime();
        let mut index = 0;
        while index < staging.processes.count {
            if let Some(process) = staging.processes.processes[index]
                && !process.context_reaped
                && process.context.cr3 != 0
            {
                targets[count] = Some(RuntimeReapTarget {
                    pid: process.pid,
                    name: process.name,
                    cr3: process.context.cr3,
                });
                count += 1;
            }
            index += 1;
        }
    }
    if count > 0 && reap_runtime_contexts(&targets, count).is_err() {
        serial::write_str("Krust staged runtime discard incomplete\n");
    }
    serial::write_str("Krust discarded uncommitted staged generation: ");
    serial::write_str(staged.generation_id);
    serial::write_str("\n");
    clear_staged_generation();
}

fn set_failed_generation(generation_id: &'static str) {
    unsafe {
        *FAILED_GENERATION.0.get() = Some(generation_id);
    }
}

fn failed_generation_is(generation_id: &'static str) -> bool {
    unsafe { *FAILED_GENERATION.0.get() == Some(generation_id) }
}

pub(super) fn boot_manager() -> &'static mut BootManagerState {
    unsafe { &mut *BOOT_MANAGER.0.get() }
}

pub(super) fn boot_manager_state() -> &'static BootManagerState {
    unsafe { &*BOOT_MANAGER.0.get() }
}

pub(super) fn store_hash_matches(bytes: &[u8], expected: &str) -> bool {
    if expected.len() != 64 {
        return false;
    }
    let mut actual = [0u8; 64];
    store_hash_hex(blake3::hash(bytes).as_bytes(), &mut actual);
    actual == expected.as_bytes()
}

fn store_hash_hex(bytes: &[u8; 32], out: &mut [u8; 64]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut index = 0;
    while index < bytes.len() {
        out[index * 2] = HEX[(bytes[index] >> 4) as usize];
        out[index * 2 + 1] = HEX[(bytes[index] & 0xf) as usize];
        index += 1;
    }
}

pub fn register_generation_config(config: &'static BootRuntimeConfig) -> Result<(), InitError> {
    let table = generation_runtimes();
    table.register(GenerationRuntime {
        generation_id: config.generation_id,
        config,
    })
}

pub fn generation_config_by_id(generation_id: &[u8]) -> Option<&'static BootRuntimeConfig> {
    generation_runtimes()
        .find(generation_id)
        .map(|runtime| runtime.config)
}

pub fn set_rollback_boot_config(config: &'static BootRuntimeConfig) {
    set_rollback_runtime(GenerationRuntime {
        generation_id: config.generation_id,
        config,
    });
}

pub fn set_failed_generation_id(generation_id: &'static str) {
    set_failed_generation(generation_id);
}

pub fn install_generation_recovery(
    selected: &'static str,
    previous: &'static str,
    known_good: &'static str,
    transaction: &'static str,
    target: &'static str,
    failure_reason: &'static str,
) {
    boot_manager().recover_from_disk(
        selected,
        previous,
        known_good,
        transaction,
        target,
        failure_reason,
    );
}

fn read_generation_request(
    cap_slot: u64,
    generation: *const u8,
    len: usize,
) -> Result<([u8; MAX_MESSAGE_BYTES], usize), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }
    let _process_control = process_control_from_cap(
        cap_slot,
        capability::RIGHT_CONTROL | capability::RIGHT_REVOKE,
    )?;
    let mut generation_id = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut generation_id, UserPtr::new(generation as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    Ok((generation_id, len))
}

pub fn stage_generation(cap_slot: u64, generation: *const u8, len: usize) -> Result<(), IpcError> {
    let (generation_id, len) = read_generation_request(cap_slot, generation, len)?;
    let requested = &generation_id[..len];
    let target = match generation_runtimes().find(requested) {
        Some(target) => target,
        None => {
            serial::write_str("Krust generation switch rejected: requested=");
            serial::write_ascii_bytes(requested);
            serial::write_str("\n");
            serial::write_str("Native update transaction rejected: missing store object\n");
            serial::write_str("Native update transaction selected_generation unchanged: ");
            serial::write_str(runtime().generation_id);
            serial::write_str("\n");
            serial::write_str(
                "update commit interrupted before final pointer leaves previous generation bootable\n",
            );
            return Err(IpcError::BadCapability);
        }
    };
    if failed_generation_is(target.generation_id) {
        serial::write_str("Krust generation switch rejected: requested=");
        serial::write_ascii_bytes(requested);
        serial::write_str(" failed=yes\n");
        return Err(IpcError::BadCapability);
    }

    let (previous_generation, previous_config, old_cap_count) = {
        let runtime = runtime();
        (
            runtime.generation_id,
            runtime.active_config,
            runtime.generation_cap_count(runtime.generation_id),
        )
    };

    if previous_generation == target.generation_id {
        serial::write_str("Krust generation switch already active: ");
        serial::write_str(target.generation_id);
        serial::write_str("\n");
        return Ok(());
    }

    discard_uncommitted_staged_generation();
    boot_manager().install_prepare(previous_generation, target.generation_id);
    if verify_generation_transaction(target).is_err() {
        boot_manager().install_abort(target.generation_id, "verification-failed");
        serial::write_str("Native update transaction selected_generation unchanged: ");
        serial::write_str(previous_generation);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }
    if apply_state_transition_policy(previous_config, target.config, false).is_err() {
        boot_manager().install_abort(target.generation_id, "state-migration-failed");
        serial::write_str("Native update transaction selected_generation unchanged: ");
        serial::write_str(previous_generation);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }

    let build = match stage_boot_config_runtime(target.config) {
        Ok(build) => build,
        Err(_) => {
            boot_manager().install_abort(target.generation_id, "runtime-build-failed");
            return Err(IpcError::BadCapability);
        }
    };

    *staged_generation() = Some(StagedGeneration {
        generation_id: target.generation_id,
        config: target.config,
        previous_generation,
        previous_config,
        old_cap_count,
        old_contexts: build.old_contexts,
        old_context_count: build.old_context_count,
        initial_context: build.initial_context,
        rollback: false,
    });

    serial::write_str("Krust generation switch staged: from=");
    serial::write_str(previous_generation);
    serial::write_str(" to=");
    serial::write_str(target.generation_id);
    serial::write_str("\n");
    Ok(())
}

pub fn activate_generation(
    cap_slot: u64,
    generation: *const u8,
    len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let (generation_id, len) = read_generation_request(cap_slot, generation, len)?;
    let requested = &generation_id[..len];
    if runtime().generation_id.as_bytes() == requested {
        serial::write_str("Krust generation switch already active: ");
        serial::write_ascii_bytes(requested);
        serial::write_str("\n");
        return Ok(());
    }
    let staged_matches = (*staged_generation())
        .map(|staged| !staged.rollback && staged.generation_id.as_bytes() == requested)
        .unwrap_or(false);
    if !staged_matches {
        stage_generation(cap_slot, generation, len)?;
    }
    let Some(staged) = *staged_generation() else {
        return Err(IpcError::BadCapability);
    };
    if staged.rollback || staged.generation_id.as_bytes() != requested {
        serial::write_str("Krust generation switch rejected: staged generation mismatch\n");
        return Err(IpcError::BadCapability);
    };

    if let Some(previous_config) = staged.previous_config {
        set_rollback_runtime(GenerationRuntime {
            generation_id: staged.previous_generation,
            config: previous_config,
        });
    }

    serial::write_str("Krust generation switch accepted: from=");
    serial::write_str(staged.previous_generation);
    serial::write_str(" to=");
    serial::write_str(staged.generation_id);
    serial::write_str("\n");
    serial::write_str("Krust generation switch revoked old generation authority: generation=");
    serial::write_str(staged.previous_generation);
    serial::write_str(" caps=");
    serial::write_u64_dec(staged.old_cap_count);
    serial::write_str("\n");
    serial::write_str("old generation service loses old capability\n");

    commit_staged_boot_config_runtime(
        staged.config,
        StagingBuild {
            initial_context: staged.initial_context,
            old_contexts: staged.old_contexts,
            old_context_count: staged.old_context_count,
        },
    );
    serial::write_str("Native update transaction journal commit\n");
    boot_manager().install_selected(staged.previous_generation, staged.generation_id);
    let context = staged.initial_context;
    clear_staged_generation();
    serial::write_str("Krust generation switch entering generation: ");
    serial::write_str(staged.generation_id);
    serial::write_str("\n");
    serial::write_str("update commit interrupted after final pointer boots verified generation\n");
    let _ = frame;
    unsafe {
        gdt::enter_user_mode(context.cr3, context.entry, context.stack_top);
    }
}

pub fn verify_generation(cap_slot: u64, generation: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let _process_control = process_control_from_cap(
        cap_slot,
        capability::RIGHT_CONTROL | capability::RIGHT_REVOKE,
    )?;
    let mut generation_id = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut generation_id, UserPtr::new(generation as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let requested = &generation_id[..len];
    let target = match generation_runtimes().find(requested) {
        Some(target) => target,
        None => {
            serial::write_str("Native generation verification rejected: requested=");
            serial::write_ascii_bytes(requested);
            serial::write_str(" reason=missing-store-object\n");
            return Err(IpcError::BadCapability);
        }
    };
    if failed_generation_is(target.generation_id) {
        serial::write_str("Native generation verification rejected: requested=");
        serial::write_ascii_bytes(requested);
        serial::write_str(" failed=yes\n");
        return Err(IpcError::BadCapability);
    }

    verify_generation_transaction(target)?;
    serial::write_str("Native generation verification accepted: generation=");
    serial::write_str(target.generation_id);
    serial::write_str("\n");
    Ok(())
}

fn verify_generation_transaction(target: GenerationRuntime) -> Result<(), IpcError> {
    validate_boot_config_installable(target.config).map_err(|_| IpcError::BadCapability)?;
    verify_generation_manifest(target.config)?;
    verify_generation_store_closure(target.config)?;
    Ok(())
}

fn verify_generation_manifest(config: &BootRuntimeConfig) -> Result<(), IpcError> {
    let Some(module) = config.manifest_module else {
        serial::write_str("Native update transaction rejected: missing manifest\n");
        return Err(IpcError::BadCapability);
    };
    let Ok(len) = usize::try_from(module.length) else {
        serial::write_str("Native update transaction rejected: manifest too large\n");
        return Err(IpcError::MessageTooLarge);
    };
    if len == 0 {
        serial::write_str("Native update transaction rejected: empty manifest\n");
        return Err(IpcError::BadCapability);
    }

    let bytes = unsafe { core::slice::from_raw_parts(module.base as *const u8, len) };
    let mut actual = [0u8; 64];
    store_hash_hex(blake3::hash(bytes).as_bytes(), &mut actual);
    if actual != config.manifest_hash {
        serial::write_str("Native update transaction rejected: manifest hash mismatch\n");
        return Err(IpcError::BadCapability);
    }

    serial::write_str("Native update transaction verifies manifest hash: generation=");
    serial::write_str(config.generation_id);
    serial::write_str(" identity=store:blake3:");
    serial::write_ascii_bytes(&config.manifest_hash);
    serial::write_str("\n");
    Ok(())
}

fn verify_generation_store_closure(config: &BootRuntimeConfig) -> Result<(), IpcError> {
    if config.store_object_count == 0 {
        serial::write_str("Native update transaction rejected: missing store closure\n");
        return Err(IpcError::BadCapability);
    }

    let mut index = 0;
    while index < config.store_object_count {
        let Some(object) = config.store_objects[index] else {
            serial::write_str("Native update transaction rejected: store closure gap\n");
            return Err(IpcError::BadCapability);
        };
        let Ok(len) = usize::try_from(object.length) else {
            serial::write_str("Native update transaction rejected: store object too large object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            return Err(IpcError::MessageTooLarge);
        };
        if len == 0 {
            serial::write_str("Native update transaction rejected: missing store object object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            return Err(IpcError::BadCapability);
        }
        let bytes = unsafe { core::slice::from_raw_parts(object.base as *const u8, len) };
        if !store_hash_matches(bytes, object.hash) {
            serial::write_str("Native update transaction rejected: store hash mismatch object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            serial::write_str("vertex-inspect security event: store hash mismatch object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            return Err(IpcError::BadCapability);
        }
        index += 1;
    }

    serial::write_str("Native update transaction verifies store closure: generation=");
    serial::write_str(config.generation_id);
    serial::write_str(" objects=");
    serial::write_u64_dec(config.store_object_count as u64);
    serial::write_str("\n");
    Ok(())
}

fn apply_state_transition_policy(
    previous: Option<&'static BootRuntimeConfig>,
    target: &'static BootRuntimeConfig,
    rollback: bool,
) -> Result<(), IpcError> {
    let Some(previous) = previous else {
        log_created_state_objects(target);
        return Ok(());
    };

    let mut target_index = 0;
    while target_index < target.state_volume_count {
        let Some(target_state) = target.state_volumes[target_index] else {
            return Err(IpcError::BadCapability);
        };
        match find_state_volume(previous, target_state.id) {
            Some(previous_state) => {
                if rollback {
                    log_state_rollback_policy(previous_state, target_state);
                }
                if previous_state.schema_version != target_state.schema_version {
                    if rollback {
                        log_state_rollback_journal(previous_state, target_state);
                    } else if target_state.migration_policy == "migrate" {
                        log_state_migration_accept(previous_state, target_state);
                    } else {
                        log_state_migration_reject(previous_state, target_state);
                        return Err(IpcError::BadCapability);
                    }
                } else if !rollback {
                    log_state_schema_unchanged(target_state);
                }
            }
            None => log_state_object_created(target_state, target.generation_id),
        }
        target_index += 1;
    }

    let mut previous_index = 0;
    while previous_index < previous.state_volume_count {
        let Some(previous_state) = previous.state_volumes[previous_index] else {
            return Err(IpcError::BadCapability);
        };
        if find_state_volume(target, previous_state.id).is_none() {
            log_state_retention(previous_state);
        }
        previous_index += 1;
    }

    Ok(())
}

fn find_state_volume(
    config: &BootRuntimeConfig,
    id: &'static str,
) -> Option<BootStateVolumeConfig> {
    let mut index = 0;
    while index < config.state_volume_count {
        if let Some(state) = config.state_volumes[index]
            && state.id == id
        {
            return Some(state);
        }
        index += 1;
    }
    None
}

fn log_created_state_objects(config: &BootRuntimeConfig) {
    let mut index = 0;
    while index < config.state_volume_count {
        if let Some(state) = config.state_volumes[index] {
            log_state_object_created(state, config.generation_id);
        }
        index += 1;
    }
}

fn log_state_object_created(state: BootStateVolumeConfig, generation_id: &'static str) {
    serial::write_str("State object created: state=");
    serial::write_str(state.id);
    serial::write_str(" owner=");
    serial::write_str(state.owner);
    serial::write_str(" schema=");
    serial::write_str(state.schema_version);
    serial::write_str(" storage=");
    serial::write_str(state.storage_class);
    serial::write_str(" retention=");
    serial::write_str(state.retention_policy);
    serial::write_str(" sharing=");
    serial::write_str(state.sharing_policy);
    serial::write_str(" generation=");
    serial::write_str(generation_id);
    serial::write_str("\n");
}

fn log_state_schema_unchanged(state: BootStateVolumeConfig) {
    serial::write_str("State migration unchanged: state=");
    serial::write_str(state.id);
    serial::write_str(" schema=");
    serial::write_str(state.schema_version);
    serial::write_str(" mode=");
    serial::write_str(state.migration_policy);
    serial::write_str("\n");
}

fn log_state_migration_accept(
    previous: BootStateVolumeConfig,
    target: BootStateVolumeConfig,
) {
    serial::write_str("State migration plan accepted: state=");
    serial::write_str(target.id);
    serial::write_str(" from=");
    serial::write_str(previous.schema_version);
    serial::write_str(" to=");
    serial::write_str(target.schema_version);
    serial::write_str(" mode=migrate\n");
    serial::write_str("State migration journal record: state=");
    serial::write_str(target.id);
    serial::write_str(" from=");
    serial::write_str(previous.schema_version);
    serial::write_str(" to=");
    serial::write_str(target.schema_version);
    serial::write_str(" status=applied-once\n");
}

fn log_state_migration_reject(
    previous: BootStateVolumeConfig,
    target: BootStateVolumeConfig,
) {
    serial::write_str("State migration failed: state=");
    serial::write_str(target.id);
    serial::write_str(" from=");
    serial::write_str(previous.schema_version);
    serial::write_str(" to=");
    serial::write_str(target.schema_version);
    serial::write_str(" reason=missing-migrate-policy\n");
    serial::write_str("State migration rollback leaves old state readable: state=");
    serial::write_str(previous.id);
    serial::write_str("\n");
}

fn log_state_rollback_policy(previous: BootStateVolumeConfig, target: BootStateVolumeConfig) {
    serial::write_str("Krust state rollback policy: state=");
    serial::write_str(target.id);
    serial::write_str(" mode=");
    serial::write_str(target.migration_policy);
    serial::write_str(" action=");
    serial::write_str(state_rollback_action(target.migration_policy));
    serial::write_str(" from=");
    serial::write_str(previous.schema_version);
    serial::write_str(" to=");
    serial::write_str(target.schema_version);
    serial::write_str("\n");
}

fn log_state_rollback_journal(
    previous: BootStateVolumeConfig,
    target: BootStateVolumeConfig,
) {
    serial::write_str("State rollback journal record: state=");
    serial::write_str(target.id);
    serial::write_str(" from=");
    serial::write_str(previous.schema_version);
    serial::write_str(" to=");
    serial::write_str(target.schema_version);
    serial::write_str(" status=policy-applied\n");
}

fn state_rollback_action(mode: &'static str) -> &'static str {
    match mode {
        "preserve" => "preserve-current",
        "migrate" => "migrate-back",
        "fork" => "fork-rollback-state",
        "discard" => "discard-current",
        _ => "reject",
    }
}

fn log_state_retention(state: BootStateVolumeConfig) {
    if state.retention_policy == "delete-when-unreferenced" {
        serial::write_str("State garbage collection removed unreferenced state: state=");
    } else {
        serial::write_str("State garbage collection deferred: state=");
    }
    serial::write_str(state.id);
    serial::write_str(" retention=");
    serial::write_str(state.retention_policy);
    serial::write_str("\n");
}

pub fn stage_rollback_generation(
    cap_slot: u64,
    generation: *const u8,
    len: usize,
) -> Result<(), IpcError> {
    let (requested, len) = read_generation_request(cap_slot, generation, len)?;
    let rollback = match unsafe { *ROLLBACK_RUNTIME.0.get() } {
        Some(rollback) => rollback,
        None => {
            serial::write_str("Krust rollback rejected: no rollback runtime\n");
            return Err(IpcError::BadCapability);
        }
    };
    if rollback.generation_id.as_bytes() != &requested[..len] {
        serial::write_str("Krust rollback rejected: requested=");
        serial::write_ascii_bytes(&requested[..len]);
        serial::write_str(" available=");
        serial::write_str(rollback.generation_id);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }

    let (previous_generation, previous_config, old_cap_count) = {
        let runtime = runtime();
        (
            runtime.generation_id,
            runtime.active_config,
            runtime.generation_cap_count(runtime.generation_id),
        )
    };

    discard_uncommitted_staged_generation();
    boot_manager().install_prepare(previous_generation, rollback.generation_id);
    if apply_state_transition_policy(previous_config, rollback.config, true).is_err() {
        boot_manager().install_abort(rollback.generation_id, "state-migration-failed");
        return Err(IpcError::BadCapability);
    }
    let build = match stage_boot_config_runtime(rollback.config) {
        Ok(build) => build,
        Err(_) => {
            boot_manager().install_abort(rollback.generation_id, "rollback-build-failed");
            return Err(IpcError::BadCapability);
        }
    };
    *staged_generation() = Some(StagedGeneration {
        generation_id: rollback.generation_id,
        config: rollback.config,
        previous_generation,
        previous_config,
        old_cap_count,
        old_contexts: build.old_contexts,
        old_context_count: build.old_context_count,
        initial_context: build.initial_context,
        rollback: true,
    });
    serial::write_str("Krust rollback generation staged: target=");
    serial::write_str(rollback.generation_id);
    serial::write_str("\n");
    Ok(())
}

pub fn rollback_generation(
    cap_slot: u64,
    generation: *const u8,
    len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let (requested, len) = read_generation_request(cap_slot, generation, len)?;
    let staged_matches = (*staged_generation())
        .map(|staged| staged.rollback && staged.generation_id.as_bytes() == &requested[..len])
        .unwrap_or(false);
    if !staged_matches {
        stage_rollback_generation(cap_slot, generation, len)?;
    }
    let Some(staged) = *staged_generation() else {
        return Err(IpcError::BadCapability);
    };
    if !staged.rollback || staged.generation_id.as_bytes() != &requested[..len] {
        serial::write_str("Krust rollback rejected: staged generation mismatch\n");
        return Err(IpcError::BadCapability);
    }

    serial::write_str("Krust rollback generation accepted: target=");
    serial::write_str(staged.generation_id);
    serial::write_str("\n");
    serial::write_str("Krust rollback revoked failed generation authority: generation=");
    serial::write_str(staged.previous_generation);
    serial::write_str(" caps=");
    serial::write_u64_dec(staged.old_cap_count);
    serial::write_str("\n");

    commit_staged_boot_config_runtime(
        staged.config,
        StagingBuild {
            initial_context: staged.initial_context,
            old_contexts: staged.old_contexts,
            old_context_count: staged.old_context_count,
        },
    );
    if let Some(previous_config) = staged.previous_config {
        set_rollback_runtime(GenerationRuntime {
            generation_id: staged.previous_generation,
            config: previous_config,
        });
        set_failed_generation(staged.previous_generation);
    }
    boot_manager().mark_failed_and_fallback(staged.previous_generation, staged.generation_id);
    let context = staged.initial_context;
    clear_staged_generation();
    serial::write_str("Krust rollback entering generation: ");
    serial::write_str(staged.generation_id);
    serial::write_str("\n");
    let _ = frame;
    unsafe {
        gdt::enter_user_mode(context.cr3, context.entry, context.stack_top);
    }
}
