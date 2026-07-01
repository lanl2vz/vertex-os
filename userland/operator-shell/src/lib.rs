#![no_std]

pub const PACKAGE_ID: &[u8] = b"pkg:vertex.operator-shell";
pub const SERVICE_ID: &[u8] = b"svc:console-shell";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    pub message: &'static [u8],
}

impl Error {
    pub const fn new(message: &'static [u8]) -> Self {
        Self { message }
    }
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentGeneration<'a> {
    pub generation: &'a [u8],
    pub policy_hash: &'a [u8],
    pub graph_hash: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationStatus<'a> {
    pub selected: &'a [u8],
    pub previous: &'a [u8],
    pub known_good: &'a [u8],
    pub transaction: &'a [u8],
    pub target: &'a [u8],
    pub policy_hash: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiffSummary<'report, 'command> {
    pub from: &'command [u8],
    pub to: &'command [u8],
    pub policy_hash: &'report [u8],
    pub service_added: u64,
    pub service_removed: u64,
    pub service_changed: u64,
    pub state_added: u64,
    pub state_removed: u64,
    pub state_changed: u64,
    pub device_added: u64,
    pub device_removed: u64,
    pub device_changed: u64,
    pub capability_added: u64,
    pub capability_removed: u64,
    pub capability_changed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WhyProof<'report, 'command> {
    pub service: &'command [u8],
    pub capability: &'command [u8],
    pub provider: &'report [u8],
    pub rights: &'report [u8],
    pub edge: &'report [u8],
    pub generation: &'report [u8],
    pub policy_hash: &'report [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WhoCanKind {
    StateWriters,
    CapabilityConsumers,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WhoCanEntry<'report, 'command> {
    StateWriter {
        service: &'report [u8],
        state: &'command [u8],
        rights: &'report [u8],
    },
    CapabilityConsumer {
        service: &'report [u8],
        capability: &'command [u8],
        rights: &'report [u8],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WhoCanSummary<'report, 'command> {
    pub object: &'command [u8],
    pub kind: WhoCanKind,
    pub count: u64,
    pub generation: &'report [u8],
    pub policy_hash: &'report [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WhichGeneration<'report, 'command> {
    pub selector: &'command [u8],
    pub process: &'command [u8],
    pub report_process: Option<&'report [u8]>,
    pub generation: &'report [u8],
    pub policy_hash: &'report [u8],
}

impl<'report, 'command> WhichGeneration<'report, 'command> {
    pub fn process(&self) -> &[u8] {
        self.report_process.unwrap_or(self.process)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationRequest<'request, 'command> {
    pub generation: &'command [u8],
    pub request: &'request [u8],
}

pub fn is_operator_command(command: &[u8]) -> bool {
    bytes_eq(command, b"current-generation")
        || bytes_eq(command, b"generations")
        || bytes_eq(command, b"generation-status")
        || starts_with(command, b"diff-generation ")
        || starts_with(command, b"planned-authority-delta ")
        || starts_with(command, b"why ")
        || starts_with(command, b"who-can ")
        || starts_with(command, b"which-generation ")
        || bytes_eq(command, b"package-list")
        || bytes_eq(command, b"activation-log")
        || starts_with(command, b"activate ")
        || starts_with(command, b"rollback ")
        || starts_with(command, b"mark-known-good ")
}

pub fn current_generation(report: &[u8]) -> Result<CurrentGeneration<'_>> {
    let line = operator_report_line(report)?;
    Ok(CurrentGeneration {
        generation: required_field(
            line,
            b"active=",
            b"operator current-generation missing active",
        )?,
        policy_hash: required_field(
            line,
            b"policy_hash=",
            b"operator current-generation missing policy hash",
        )?,
        graph_hash: required_field(
            line,
            b"graph_hash=",
            b"operator current-generation missing graph hash",
        )?,
    })
}

pub fn for_generations<F>(report: &[u8], mut visit: F) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-generation[") {
            visit(line);
            count += 1;
        }
    });
    if count == 0 {
        return Err(Error::new(b"operator generations query failed"));
    }
    Ok(count)
}

pub fn generation_status(report: &[u8]) -> Result<GenerationStatus<'_>> {
    let needles: [&[u8]; 2] = [b"generation-manager v=1", b"selected="];
    let manager = find_line_contains_all(report, &needles)
        .ok_or(Error::new(b"operator generation-status query failed"))?;
    Ok(GenerationStatus {
        selected: required_field(manager, b"selected=", b"generation-status missing selected")?,
        previous: required_field(manager, b"previous=", b"generation-status missing previous")?,
        known_good: required_field(
            manager,
            b"known_good=",
            b"generation-status missing known-good",
        )?,
        transaction: required_field(
            manager,
            b"transaction=",
            b"generation-status missing transaction",
        )?,
        target: required_field(manager, b"target=", b"generation-status missing target")?,
        policy_hash: active_policy_hash(report)?,
    })
}

pub fn diff_generation<'report, 'command>(
    report: &'report [u8],
    command: &'command [u8],
) -> Result<DiffSummary<'report, 'command>> {
    let from = word_or_error(command, 1, b"operator diff missing source generation")?;
    let to = word_or_error(command, 2, b"operator diff missing target generation")?;
    if word_at(command, 3).is_some() {
        return Err(Error::new(b"operator diff rejected: too many arguments"));
    }
    require_operator_generation(report, from)?;
    let target_line = require_operator_generation(report, to)?;
    let policy_hash = required_field(target_line, b"policy_hash=", b"operator diff missing hash")?;

    Ok(DiffSummary {
        from,
        to,
        policy_hash,
        service_added: count_node_delta(report, from, to, b"service"),
        service_removed: count_node_delta(report, to, from, b"service"),
        service_changed: count_service_changed(report, from, to)?,
        state_added: count_node_delta(report, from, to, b"state-volume"),
        state_removed: count_node_delta(report, to, from, b"state-volume"),
        state_changed: count_state_changed(report, from, to)?,
        device_added: count_node_delta(report, from, to, b"device"),
        device_removed: count_node_delta(report, to, from, b"device"),
        device_changed: count_node_changed(report, from, to, b"device")?,
        capability_added: count_capability_delta(report, from, to),
        capability_removed: count_capability_delta(report, to, from),
        capability_changed: count_capability_changed(report, from, to)?,
    })
}

pub fn why<'report, 'command>(
    report: &'report [u8],
    command: &'command [u8],
) -> Result<WhyProof<'report, 'command>> {
    let service = word_or_error(command, 1, b"operator why missing service")?;
    let capability = word_or_error(command, 2, b"operator why missing capability")?;
    if word_at(command, 3).is_some() {
        return Err(Error::new(b"operator why rejected: too many arguments"));
    }
    let generation = active_generation(report)?;
    let requirement = require_operator_requirement(report, generation, service, capability)?;
    let capability_line = require_operator_capability(report, generation, capability)?;
    let requirement_rights = required_field(
        requirement,
        b"rights=",
        b"operator why missing requirement rights",
    )?;
    let capability_rights = required_field(
        capability_line,
        b"rights=",
        b"operator why missing capability rights",
    )?;
    if !rights_cover(capability_rights, requirement_rights) {
        return Err(Error::new(
            b"operator why rejected: requirement rights exceed capability rights",
        ));
    }
    let object = required_field(
        capability_line,
        b"object=",
        b"operator why missing capability object",
    )?;
    let edge = require_operator_edge(report, generation, object, requirement_rights)?;
    let process = operator_service_process(report, generation, service)?;
    require_live_capability(report, process, service, object, requirement_rights)?;
    Ok(WhyProof {
        service,
        capability,
        provider: required_field(
            capability_line,
            b"provider=",
            b"operator why missing capability provider",
        )?,
        rights: requirement_rights,
        edge: required_field(edge, b"id=", b"operator why missing edge id")?,
        generation,
        policy_hash: active_policy_hash(report)?,
    })
}

pub fn who_can<'report, 'command, F>(
    report: &'report [u8],
    command: &'command [u8],
    mut visit: F,
) -> Result<WhoCanSummary<'report, 'command>>
where
    F: FnMut(WhoCanEntry<'report, 'command>),
{
    let object = word_or_error(command, 1, b"operator who-can missing object")?;
    if word_at(command, 2).is_some() {
        return Err(Error::new(b"operator who-can rejected: too many arguments"));
    }
    let generation = active_generation(report)?;
    let policy_hash = active_policy_hash(report)?;
    if starts_with(object, b"state:") {
        let count = for_state_writers(report, generation, object, |service, rights| {
            visit(WhoCanEntry::StateWriter {
                service,
                state: object,
                rights,
            });
        });
        if count == 0 {
            return Err(Error::new(
                b"operator who-can rejected: no graph-authorized state writers",
            ));
        }
        return Ok(WhoCanSummary {
            object,
            kind: WhoCanKind::StateWriters,
            count,
            generation,
            policy_hash,
        });
    }
    if starts_with(object, b"cap:") {
        let count = for_capability_consumers(report, generation, object, |service, rights| {
            visit(WhoCanEntry::CapabilityConsumer {
                service,
                capability: object,
                rights,
            });
        });
        return Ok(WhoCanSummary {
            object,
            kind: WhoCanKind::CapabilityConsumers,
            count,
            generation,
            policy_hash,
        });
    }
    Err(Error::new(
        b"operator who-can rejected: unsupported object kind",
    ))
}

pub fn which_generation<'report, 'command>(
    report: &'report [u8],
    command: &'command [u8],
) -> Result<WhichGeneration<'report, 'command>> {
    let selector = word_or_error(command, 1, b"operator which-generation missing selector")?;
    if word_at(command, 2).is_some() {
        return Err(Error::new(
            b"operator which-generation rejected: too many arguments",
        ));
    }
    let process = if starts_with(selector, b"svc:") {
        let report_process =
            operator_service_process(report, active_generation(report)?, selector)?;
        (selector, Some(report_process))
    } else {
        (selector, None)
    };
    let process_name = process.1.unwrap_or(process.0);
    let line = find_process_line(report, process_name).ok_or(Error::new(
        b"operator which-generation rejected: unknown process",
    ))?;
    Ok(WhichGeneration {
        selector,
        process: process.0,
        report_process: process.1,
        generation: required_field(
            line,
            b"generation=",
            b"operator which-generation missing generation",
        )?,
        policy_hash: active_policy_hash(report)?,
    })
}

pub fn package_list_unavailable(report: &[u8]) -> Result<()> {
    let generation = active_generation(report)?;
    let generation_line = require_operator_generation(report, generation)?;
    let facts = required_field(
        generation_line,
        b"package_facts=",
        b"operator package-list missing facts",
    )?;
    if !bytes_eq(facts, b"absent") {
        return Err(Error::new(
            b"operator package-list rejected: unsupported package fact encoding",
        ));
    }
    Ok(())
}

pub fn activation_log<F>(report: &[u8], mut visit: F) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"service-lifecycle[") {
            visit(line);
            count += 1;
        }
    });
    if count == 0 {
        return Err(Error::new(
            b"operator activation-log rejected: no lifecycle records",
        ));
    }
    Ok(count)
}

pub fn activate_request<'request, 'command>(
    command: &'command [u8],
    request: &'request mut [u8],
) -> Result<GenerationRequest<'request, 'command>> {
    generation_manager_request(
        command,
        b"install ",
        b"operator activate missing generation",
        b"operator activate rejected: too many arguments",
        request,
    )
}

pub fn rollback_request<'request, 'command>(
    command: &'command [u8],
    request: &'request mut [u8],
) -> Result<GenerationRequest<'request, 'command>> {
    generation_manager_request(
        command,
        b"rollback ",
        b"operator rollback missing generation",
        b"operator rollback rejected: too many arguments",
        request,
    )
}

pub fn mark_known_good_request<'request, 'command>(
    report: &[u8],
    command: &'command [u8],
    request: &'request mut [u8],
) -> Result<GenerationRequest<'request, 'command>> {
    let generation = word_or_error(command, 1, b"operator mark-known-good missing generation")?;
    if word_at(command, 2).is_some() {
        return Err(Error::new(
            b"operator mark-known-good rejected: too many arguments",
        ));
    }
    let active = active_generation(report)?;
    if !bytes_eq(active, generation) {
        return Err(Error::new(
            b"operator mark-known-good rejected: target is not active generation",
        ));
    }
    let len = write_request(request, b"mark-known-good ", generation)?;
    Ok(GenerationRequest {
        generation,
        request: &request[..len],
    })
}

fn generation_manager_request<'request, 'command>(
    command: &'command [u8],
    prefix: &[u8],
    missing: &'static [u8],
    too_many: &'static [u8],
    request: &'request mut [u8],
) -> Result<GenerationRequest<'request, 'command>> {
    let generation = word_or_error(command, 1, missing)?;
    if word_at(command, 2).is_some() {
        return Err(Error::new(too_many));
    }
    let len = write_request(request, prefix, generation)?;
    Ok(GenerationRequest {
        generation,
        request: &request[..len],
    })
}

fn write_request(buffer: &mut [u8], prefix: &[u8], generation: &[u8]) -> Result<usize> {
    let mut len = 0;
    append(buffer, &mut len, prefix)?;
    append(buffer, &mut len, generation)?;
    Ok(len)
}

fn operator_report_line(report: &[u8]) -> Result<&[u8]> {
    let needles: [&[u8]; 2] = [b"operator-report v=1", b"active="];
    find_line_contains_all(report, &needles).ok_or(Error::new(b"operator report missing"))
}

fn active_generation(report: &[u8]) -> Result<&[u8]> {
    required_field(
        operator_report_line(report)?,
        b"active=",
        b"operator report missing active generation",
    )
}

fn active_policy_hash(report: &[u8]) -> Result<&[u8]> {
    required_field(
        operator_report_line(report)?,
        b"policy_hash=",
        b"operator report missing policy hash",
    )
}

fn require_operator_generation<'a>(report: &'a [u8], generation: &[u8]) -> Result<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-generation[") && field_eq(line, b"id=", generation)
    })
    .ok_or(Error::new(b"operator rejected: unknown generation"))
}

fn require_operator_requirement<'a>(
    report: &'a [u8],
    generation: &[u8],
    service: &[u8],
    capability: &[u8],
) -> Result<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-requirement[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"service=", service)
            && field_eq(line, b"capability=", capability)
    })
    .ok_or(Error::new(b"operator rejected: missing policy requirement"))
}

fn require_operator_capability<'a>(
    report: &'a [u8],
    generation: &[u8],
    capability: &[u8],
) -> Result<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-capability[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"id=", capability)
    })
    .ok_or(Error::new(b"operator rejected: missing policy capability"))
}

fn require_operator_edge<'a>(
    report: &'a [u8],
    generation: &[u8],
    object: &[u8],
    required_rights: &[u8],
) -> Result<&'a [u8]> {
    find_line_where(report, |line| {
        if !(starts_with(line, b"operator-edge[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"kind=", b"capability")
            && field_eq(line, b"to=", object))
        {
            return false;
        }
        field_slice(line, b"rights=")
            .is_some_and(|edge_rights| rights_cover(edge_rights, required_rights))
    })
    .ok_or(Error::new(
        b"operator rejected: missing graph capability edge",
    ))
}

fn operator_service_process<'a>(
    report: &'a [u8],
    generation: &[u8],
    service: &[u8],
) -> Result<&'a [u8]> {
    let line = find_line_where(report, |line| {
        starts_with(line, b"operator-service[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"id=", service)
    })
    .ok_or(Error::new(b"operator rejected: unknown service"))?;
    required_field(line, b"process=", b"operator service missing process")
}

fn require_live_capability(
    report: &[u8],
    process: &[u8],
    service: &[u8],
    object: &[u8],
    required_rights: &[u8],
) -> Result<()> {
    let generation = active_generation(report)?;
    let mut accepted = false;
    for_each_line(report, |line| {
        if starts_with(line, b"space=")
            && field_eq(line, b"proc=", process)
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"graph_from=", service)
            && field_eq(line, b"graph_target=", object)
            && field_eq(line, b"revoked=", b"no")
            && let Some(rights) = field_slice(line, b"rights=")
            && rights_cover(rights, required_rights)
        {
            accepted = true;
        }
    });
    if accepted {
        Ok(())
    } else {
        Err(Error::new(
            b"operator rejected: live capability missing or insufficient",
        ))
    }
}

fn for_state_writers<'report, F>(
    report: &'report [u8],
    generation: &[u8],
    state: &[u8],
    mut visit: F,
) -> u64
where
    F: FnMut(&'report [u8], &'report [u8]),
{
    let mut writers = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-state-path[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"state=", state)
            && let Some(rights) = field_slice(line, b"rights=")
            && right_present(rights, b"write")
            && let Some(service) = field_slice(line, b"service=")
        {
            visit(service, rights);
            writers += 1;
        }
    });
    writers
}

fn for_capability_consumers<'report, F>(
    report: &'report [u8],
    generation: &[u8],
    capability: &[u8],
    mut visit: F,
) -> u64
where
    F: FnMut(&'report [u8], &'report [u8]),
{
    let mut consumers = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-requirement[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"capability=", capability)
            && let Some(service) = field_slice(line, b"service=")
            && let Some(rights) = field_slice(line, b"rights=")
        {
            visit(service, rights);
            consumers += 1;
        }
    });
    consumers
}

fn count_node_delta(report: &[u8], from: &[u8], to: &[u8], kind: &[u8]) -> u64 {
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-node[")
            && field_eq(line, b"generation=", to)
            && field_eq(line, b"kind=", kind)
            && let Some(id) = field_slice(line, b"id=")
            && !operator_node_exists(report, from, kind, id)
        {
            count += 1;
        }
    });
    count
}

fn count_capability_delta(report: &[u8], from: &[u8], to: &[u8]) -> u64 {
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-capability[")
            && field_eq(line, b"generation=", to)
            && let Some(id) = field_slice(line, b"id=")
            && !operator_capability_exists(report, from, id)
        {
            count += 1;
        }
    });
    count
}

fn count_service_changed(report: &[u8], from: &[u8], to: &[u8]) -> Result<u64> {
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-node[")
            && field_eq(line, b"generation=", to)
            && field_eq(line, b"kind=", b"service")
            && let Some(id) = field_slice(line, b"id=")
            && let Some(previous_node) = operator_node_line(report, from, b"service", id)
        {
            let Some(previous_service) = operator_service_line(report, from, id) else {
                error = Some(Error::new(b"operator diff rejected: service fact missing"));
                return;
            };
            let Some(current_service) = operator_service_line(report, to, id) else {
                error = Some(Error::new(b"operator diff rejected: service fact missing"));
                return;
            };
            match (
                operator_node_semantically_equal(previous_node, line),
                operator_service_semantically_equal(previous_service, current_service),
            ) {
                (Ok(node_equal), Ok(service_equal)) => {
                    if !node_equal || !service_equal {
                        count += 1;
                    }
                }
                (Err(err), _) | (_, Err(err)) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        Err(err)
    } else {
        Ok(count)
    }
}

fn count_state_changed(report: &[u8], from: &[u8], to: &[u8]) -> Result<u64> {
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-node[")
            && field_eq(line, b"generation=", to)
            && field_eq(line, b"kind=", b"state-volume")
            && let Some(id) = field_slice(line, b"id=")
            && let Some(previous_node) = operator_node_line(report, from, b"state-volume", id)
        {
            let Some(previous_state) = operator_state_line(report, from, id) else {
                error = Some(Error::new(b"operator diff rejected: state fact missing"));
                return;
            };
            let Some(current_state) = operator_state_line(report, to, id) else {
                error = Some(Error::new(b"operator diff rejected: state fact missing"));
                return;
            };
            match (
                operator_node_semantically_equal(previous_node, line),
                operator_state_semantically_equal(previous_state, current_state),
            ) {
                (Ok(node_equal), Ok(state_equal)) => {
                    if !node_equal || !state_equal {
                        count += 1;
                    }
                }
                (Err(err), _) | (_, Err(err)) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        Err(err)
    } else {
        Ok(count)
    }
}

fn count_node_changed(report: &[u8], from: &[u8], to: &[u8], kind: &[u8]) -> Result<u64> {
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-node[")
            && field_eq(line, b"generation=", to)
            && field_eq(line, b"kind=", kind)
            && let Some(id) = field_slice(line, b"id=")
            && let Some(previous) = operator_node_line(report, from, kind, id)
        {
            match operator_node_semantically_equal(previous, line) {
                Ok(false) => count += 1,
                Ok(true) => {}
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        Err(err)
    } else {
        Ok(count)
    }
}

fn count_capability_changed(report: &[u8], from: &[u8], to: &[u8]) -> Result<u64> {
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-capability[")
            && field_eq(line, b"generation=", to)
            && let Some(id) = field_slice(line, b"id=")
            && let Some(previous) = operator_capability_line(report, from, id)
        {
            match operator_capability_semantically_equal(previous, line) {
                Ok(false) => count += 1,
                Ok(true) => {}
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        Err(err)
    } else {
        Ok(count)
    }
}

fn operator_node_line<'a>(
    report: &'a [u8],
    generation: &[u8],
    kind: &[u8],
    id: &[u8],
) -> Option<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-node[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"kind=", kind)
            && field_eq(line, b"id=", id)
    })
}

fn operator_capability_line<'a>(
    report: &'a [u8],
    generation: &[u8],
    id: &[u8],
) -> Option<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-capability[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"id=", id)
    })
}

fn operator_service_line<'a>(report: &'a [u8], generation: &[u8], id: &[u8]) -> Option<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-service[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"id=", id)
    })
}

fn operator_state_line<'a>(report: &'a [u8], generation: &[u8], id: &[u8]) -> Option<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-state[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"id=", id)
    })
}

fn operator_node_exists(report: &[u8], generation: &[u8], kind: &[u8], id: &[u8]) -> bool {
    operator_node_line(report, generation, kind, id).is_some()
}

fn operator_capability_exists(report: &[u8], generation: &[u8], id: &[u8]) -> bool {
    operator_capability_line(report, generation, id).is_some()
}

fn operator_node_semantically_equal(left: &[u8], right: &[u8]) -> Result<bool> {
    Ok(
        field_pair_eq(left, right, b"kind=", b"operator node missing kind")?
            && field_pair_eq(left, right, b"id=", b"operator node missing id")?
            && field_pair_eq(
                left,
                right,
                b"object_kind=",
                b"operator node missing object kind",
            )?
            && field_pair_eq(left, right, b"label=", b"operator node missing label")?,
    )
}

fn operator_capability_semantically_equal(left: &[u8], right: &[u8]) -> Result<bool> {
    Ok(
        field_pair_eq(left, right, b"id=", b"operator capability missing id")?
            && field_pair_eq(
                left,
                right,
                b"provider=",
                b"operator capability missing provider",
            )?
            && field_pair_eq(
                left,
                right,
                b"object_kind=",
                b"operator capability missing object kind",
            )?
            && field_pair_eq(
                left,
                right,
                b"object=",
                b"operator capability missing object",
            )?
            && rights_pair_eq(left, right, b"operator capability missing rights")?,
    )
}

fn operator_service_semantically_equal(left: &[u8], right: &[u8]) -> Result<bool> {
    Ok(
        field_pair_eq(left, right, b"id=", b"operator service missing id")?
            && field_pair_eq(
                left,
                right,
                b"process=",
                b"operator service missing process",
            )?
            && field_pair_eq(
                left,
                right,
                b"restart=",
                b"operator service missing restart",
            )?
            && field_pair_eq(
                left,
                right,
                b"mount_root=",
                b"operator service missing mount root",
            )?,
    )
}

fn operator_state_semantically_equal(left: &[u8], right: &[u8]) -> Result<bool> {
    Ok(
        field_pair_eq(left, right, b"id=", b"operator state missing id")?
            && field_pair_eq(left, right, b"owner=", b"operator state missing owner")?
            && field_pair_eq(left, right, b"schema=", b"operator state missing schema")?
            && field_pair_eq(left, right, b"storage=", b"operator state missing storage")?
            && field_pair_eq(
                left,
                right,
                b"migration=",
                b"operator state missing migration",
            )?
            && field_pair_eq(
                left,
                right,
                b"retention=",
                b"operator state missing retention",
            )?
            && field_pair_eq(left, right, b"sharing=", b"operator state missing sharing")?,
    )
}

fn field_pair_eq(left: &[u8], right: &[u8], prefix: &[u8], message: &'static [u8]) -> Result<bool> {
    Ok(bytes_eq(
        required_field(left, prefix, message)?,
        required_field(right, prefix, message)?,
    ))
}

fn rights_pair_eq(left: &[u8], right: &[u8], message: &'static [u8]) -> Result<bool> {
    let left_rights = required_field(left, b"rights=", message)?;
    let right_rights = required_field(right, b"rights=", message)?;
    Ok(rights_cover(left_rights, right_rights) && rights_cover(right_rights, left_rights))
}

fn find_process_line<'a>(report: &'a [u8], process: &[u8]) -> Option<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"process[") && field_eq(line, b"name=", process)
    })
}

fn word_or_error<'a>(command: &'a [u8], index: usize, message: &'static [u8]) -> Result<&'a [u8]> {
    word_at(command, index).ok_or(Error::new(message))
}

fn word_at(command: &[u8], requested: usize) -> Option<&[u8]> {
    let mut cursor = 0;
    let mut index = 0;
    while cursor < command.len() {
        while cursor < command.len() && command[cursor] == b' ' {
            cursor += 1;
        }
        if cursor == command.len() {
            return None;
        }
        let start = cursor;
        while cursor < command.len() && command[cursor] != b' ' {
            cursor += 1;
        }
        if index == requested {
            return Some(&command[start..cursor]);
        }
        index += 1;
    }
    None
}

fn required_field<'a>(line: &'a [u8], prefix: &[u8], message: &'static [u8]) -> Result<&'a [u8]> {
    field_slice(line, prefix).ok_or(Error::new(message))
}

fn rights_cover(available: &[u8], required: &[u8]) -> bool {
    if bytes_eq(required, b"none") {
        return true;
    }
    let mut start = 0;
    while start <= required.len() {
        let mut end = start;
        while end < required.len() && required[end] != b'|' {
            end += 1;
        }
        if end == start || !right_present(available, &required[start..end]) {
            return false;
        }
        if end == required.len() {
            break;
        }
        start = end + 1;
    }
    true
}

fn right_present(rights: &[u8], right: &[u8]) -> bool {
    let mut start = 0;
    while start <= rights.len() {
        let mut end = start;
        while end < rights.len() && rights[end] != b'|' {
            end += 1;
        }
        if bytes_eq(&rights[start..end], right) {
            return true;
        }
        if end == rights.len() {
            break;
        }
        start = end + 1;
    }
    false
}

fn find_line_contains_all<'a>(haystack: &'a [u8], needles: &[&[u8]]) -> Option<&'a [u8]> {
    find_line_where(haystack, |line| contains_all(line, needles))
}

fn find_line_where<'a, F>(haystack: &'a [u8], mut predicate: F) -> Option<&'a [u8]>
where
    F: FnMut(&[u8]) -> bool,
{
    let mut start = 0;
    while start <= haystack.len() {
        let mut end = start;
        while end < haystack.len() && haystack[end] != b'\n' {
            end += 1;
        }
        let line = &haystack[start..end];
        if predicate(line) {
            return Some(line);
        }
        if end == haystack.len() {
            break;
        }
        start = end + 1;
    }
    None
}

fn for_each_line<'a, F>(haystack: &'a [u8], mut visit: F)
where
    F: FnMut(&'a [u8]),
{
    let mut start = 0;
    while start <= haystack.len() {
        let mut end = start;
        while end < haystack.len() && haystack[end] != b'\n' {
            end += 1;
        }
        visit(&haystack[start..end]);
        if end == haystack.len() {
            break;
        }
        start = end + 1;
    }
}

fn contains_all(haystack: &[u8], needles: &[&[u8]]) -> bool {
    let mut index = 0;
    while index < needles.len() {
        if find_subslice(haystack, needles[index]).is_none() {
            return false;
        }
        index += 1;
    }
    true
}

fn field_eq(line: &[u8], prefix: &[u8], expected: &[u8]) -> bool {
    field_slice(line, prefix).is_some_and(|value| bytes_eq(value, expected))
}

fn field_slice<'a>(line: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    let start = find_subslice(line, prefix)? + prefix.len();
    let mut end = start;
    while end < line.len() && line[end] != b' ' && line[end] != b'\n' {
        end += 1;
    }
    Some(&line[start..end])
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    let mut start = 0;
    while start + needle.len() <= haystack.len() {
        let mut index = 0;
        while index < needle.len() && haystack[start + index] == needle[index] {
            index += 1;
        }
        if index == needle.len() {
            return Some(start);
        }
        start += 1;
    }
    None
}

fn starts_with(value: &[u8], prefix: &[u8]) -> bool {
    if value.len() < prefix.len() {
        return false;
    }
    let mut index = 0;
    while index < prefix.len() {
        if value[index] != prefix[index] {
            return false;
        }
        index += 1;
    }
    true
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

fn append(buffer: &mut [u8], len: &mut usize, value: &[u8]) -> Result<()> {
    let mut index = 0;
    while index < value.len() {
        if *len >= buffer.len() {
            return Err(Error::new(b"operator request too large"));
        }
        buffer[*len] = value[index];
        *len += 1;
        index += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    const WRONG_TARGET_REPORT: &[u8] = b"operator-report v=1 active=gen:a registered=1 policy_hash=hash:policy graph_hash=hash:graph
operator-generation[0] id=gen:a active=yes selected=yes previous=no known_good=yes policy_hash=hash:policy graph_hash=hash:graph services=2 capabilities=1 states=0 devices=0 packages=0 package_facts=absent
operator-service[0] generation=gen:a id=svc:echo-server process=echo restart=never mount_root=/
operator-requirement[0] generation=gen:a service=svc:echo-server capability=cap:log.sink rights=send
operator-capability[0] generation=gen:a id=cap:log.sink provider=svc:logd object_kind=endpoint object=log-sink rights=send
operator-edge[0] generation=gen:a kind=capability id=edge:echo-log from=svc:echo-server to=log-sink rights=send
space=initial proc=echo cap[0] endpoint=other-log rights=send cap_id=1 parent_cap_id=0 generation=gen:a graph_from=svc:echo-server graph_target=other-log graph_edge=edge:other owner_pid=2 owner=echo delegated_by_pid=1 delegated_by=vertex-init revoked=no
";

    const CHANGED_CAP_REPORT: &[u8] = b"operator-report v=1 active=gen:a registered=2 policy_hash=hash:a graph_hash=hash:a
operator-generation[0] id=gen:a active=yes selected=yes previous=no known_good=yes policy_hash=hash:a graph_hash=hash:a services=0 capabilities=1 states=0 devices=0 packages=0 package_facts=absent
operator-generation[1] id=gen:b active=no selected=no previous=no known_good=no policy_hash=hash:b graph_hash=hash:b services=0 capabilities=1 states=0 devices=0 packages=0 package_facts=absent
operator-capability[0] generation=gen:a id=cap:log.sink provider=svc:logd object_kind=endpoint object=log-sink rights=send
operator-capability[1] generation=gen:b id=cap:log.sink provider=svc:logd object_kind=endpoint object=log-sink rights=send|inspect
";

    #[test]
    fn why_rejects_live_capability_with_wrong_graph_target() {
        let error = why(WRONG_TARGET_REPORT, b"why svc:echo-server cap:log.sink")
            .expect_err("wrong live graph_target must not prove the policy cap");
        assert_eq!(
            error.message,
            b"operator rejected: live capability missing or insufficient"
        );
    }

    #[test]
    fn diff_reports_same_id_capability_changes() {
        let summary = diff_generation(CHANGED_CAP_REPORT, b"planned-authority-delta gen:a gen:b")
            .expect("same-id rights changes should be valid diff input");
        assert_eq!(summary.capability_added, 0);
        assert_eq!(summary.capability_removed, 0);
        assert_eq!(summary.capability_changed, 1);
    }

    #[test]
    fn activate_request_is_target_neutral_generation_manager_message() {
        let mut request = [0u8; 64];
        let message = activate_request(b"activate gen:b", &mut request).unwrap();
        assert_eq!(message.generation, b"gen:b");
        assert_eq!(message.request, b"install gen:b");
    }
}
