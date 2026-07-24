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
    pub last_failed: &'a [u8],
    pub transaction: &'a [u8],
    pub target: &'a [u8],
    pub failure_reason: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemVerification<'a> {
    pub generation: &'a [u8],
    pub policy_hash: &'a [u8],
    pub graph_hash: &'a [u8],
    pub services: u64,
    pub capabilities: u64,
    pub states: u64,
    pub devices: u64,
    pub packages: u64,
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
        || bytes_eq(command, b"overview")
        || bytes_eq(command, b"services")
        || starts_with(command, b"service ")
        || bytes_eq(command, b"capabilities")
        || starts_with(command, b"capabilities ")
        || starts_with(command, b"capability ")
        || bytes_eq(command, b"states")
        || starts_with(command, b"state ")
        || bytes_eq(command, b"devices")
        || starts_with(command, b"device ")
        || starts_with(command, b"diff-generation ")
        || starts_with(command, b"planned-authority-delta ")
        || starts_with(command, b"why ")
        || starts_with(command, b"who-can ")
        || starts_with(command, b"which-generation ")
        || bytes_eq(command, b"package-list")
        || bytes_eq(command, b"activation-log")
        || bytes_eq(command, b"verify-system")
        || starts_with(command, b"activate ")
        || starts_with(command, b"rollback ")
        || starts_with(command, b"mark-known-good ")
}

pub fn normalize_command<'a>(input: &[u8], output: &'a mut [u8]) -> Result<&'a [u8]> {
    let mut input_index = 0;
    let mut output_len = 0;
    let mut pending_space = false;
    let mut in_verb = true;

    while input_index < input.len() {
        let mut byte = input[input_index];
        input_index += 1;

        if is_command_space(byte) {
            if output_len != 0 {
                pending_space = true;
            }
            continue;
        }

        if pending_space {
            if output_len >= output.len() {
                return Err(Error::new(b"operator command too large"));
            }
            output[output_len] = b' ';
            output_len += 1;
            pending_space = false;
            in_verb = false;
        }

        if in_verb && byte.is_ascii_uppercase() {
            byte += b'a' - b'A';
        }
        if output_len >= output.len() {
            return Err(Error::new(b"operator command too large"));
        }
        output[output_len] = byte;
        output_len += 1;
    }

    Ok(&output[..output_len])
}

pub fn help<F>(command: &[u8], mut visit: F) -> Result<()>
where
    F: FnMut(&[u8]),
{
    let topic = word_at(command, 1);
    if word_at(command, 2).is_some() {
        return Err(Error::new(b"operator help rejected: too many arguments"));
    }
    let Some(topic) = topic else {
        emit_line(&mut visit, &[b"Vertex OS operator console commands"])?;
        emit_line(
            &mut visit,
            &[b"discover  overview | services | capabilities | states | devices"],
        )?;
        emit_line(
            &mut visit,
            &[b"inspect   service <id> | capability <id> | state <id> | device <id>"],
        )?;
        emit_line(
            &mut visit,
            &[b"explain   why <service> <capability> | who-can <object>"],
        )?;
        emit_line(
            &mut visit,
            &[b"system    current-generation | generations | generation-status"],
        )?;
        emit_line(
            &mut visit,
            &[b"compare   diff-generation <from> <to> | planned-authority-delta <from> <to>"],
        )?;
        emit_line(
            &mut visit,
            &[b"change    activate <generation> | rollback <generation> | mark-known-good <generation>"],
        )?;
        emit_line(
            &mut visit,
            &[b"verify    verify-system | activation-log | package-list | which-generation <process>"],
        )?;
        emit_line(
            &mut visit,
            &[b"utility   generation | counter | increment | state-health | halt"],
        )?;
        emit_line(
            &mut visit,
            &[b"tip       help <command> shows usage and examples"],
        )?;
        return Ok(());
    };

    if bytes_eq(topic, b"overview") {
        emit_line(&mut visit, &[b"overview"])?;
        emit_line(&mut visit, &[b"  system summary for the active generation"])?;
        emit_line(&mut visit, &[b"  example: overview"])?;
        return Ok(());
    }
    if bytes_eq(topic, b"services") || bytes_eq(topic, b"service") {
        emit_line(&mut visit, &[b"services"])?;
        emit_line(
            &mut visit,
            &[b"  list services with process state and restart policy"],
        )?;
        emit_line(&mut visit, &[b"service <service-or-process>"])?;
        emit_line(
            &mut visit,
            &[b"  show requirements and state paths for one service"],
        )?;
        emit_line(&mut visit, &[b"  example: service svc:echo-server"])?;
        return Ok(());
    }
    if bytes_eq(topic, b"capabilities") || bytes_eq(topic, b"capability") {
        emit_line(&mut visit, &[b"capabilities"])?;
        emit_line(
            &mut visit,
            &[b"  list capability IDs, providers, rights, and consumers"],
        )?;
        emit_line(&mut visit, &[b"capabilities for <service-or-process>"])?;
        emit_line(&mut visit, &[b"capability <capability-id>"])?;
        emit_line(&mut visit, &[b"  example: capability cap:log.sink"])?;
        return Ok(());
    }
    if bytes_eq(topic, b"why") {
        emit_line(&mut visit, &[b"why <service> <capability>"])?;
        emit_line(
            &mut visit,
            &[b"  proves policy requirement, capability object, graph edge, and live cap"],
        )?;
        emit_line(
            &mut visit,
            &[b"  example: why svc:echo-server cap:log.sink"],
        )?;
        emit_line(
            &mut visit,
            &[b"  discover IDs with: services and capabilities"],
        )?;
        return Ok(());
    }
    if bytes_eq(topic, b"who-can") {
        emit_line(&mut visit, &[b"who-can <object>"])?;
        emit_line(
            &mut visit,
            &[b"  lists graph-authorized writers or capability consumers"],
        )?;
        emit_line(
            &mut visit,
            &[b"  examples: who-can state:counter | who-can cap:log.sink"],
        )?;
        return Ok(());
    }
    if bytes_eq(topic, b"generations") || bytes_eq(topic, b"generation") {
        emit_line(&mut visit, &[b"generations"])?;
        emit_line(&mut visit, &[b"generation-status"])?;
        emit_line(&mut visit, &[b"diff-generation <from> <to>"])?;
        emit_line(&mut visit, &[b"planned-authority-delta <from> <to>"])?;
        return Ok(());
    }
    if bytes_eq(topic, b"states") || bytes_eq(topic, b"state") {
        emit_line(&mut visit, &[b"states"])?;
        emit_line(&mut visit, &[b"state <state-id>"])?;
        emit_line(&mut visit, &[b"  example: state state:counter"])?;
        return Ok(());
    }
    if bytes_eq(topic, b"devices") || bytes_eq(topic, b"device") {
        emit_line(&mut visit, &[b"devices"])?;
        emit_line(&mut visit, &[b"device <device-id>"])?;
        emit_line(&mut visit, &[b"  lists graph device nodes when present"])?;
        return Ok(());
    }
    if bytes_eq(topic, b"package-list") {
        emit_line(&mut visit, &[b"package-list"])?;
        emit_line(
            &mut visit,
            &[b"  lists explicit package graph facts for the active generation"],
        )?;
        return Ok(());
    }
    if bytes_eq(topic, b"verify-system") {
        emit_line(&mut visit, &[b"verify-system"])?;
        emit_line(
            &mut visit,
            &[b"  checks active graph, state health, packages, objects, and caps"],
        )?;
        return Ok(());
    }
    if bytes_eq(topic, b"activate")
        || bytes_eq(topic, b"rollback")
        || bytes_eq(topic, b"mark-known-good")
    {
        emit_line(&mut visit, &[topic, b" <generation>"])?;
        emit_line(
            &mut visit,
            &[b"  generation changes are verified and recorded by generation-manager"],
        )?;
        return Ok(());
    }
    if bytes_eq(topic, b"counter") || bytes_eq(topic, b"increment") {
        emit_line(&mut visit, &[topic])?;
        emit_line(
            &mut visit,
            &[b"  reads or advances the graph-authorized state counter"],
        )?;
        return Ok(());
    }
    if bytes_eq(topic, b"state-health") {
        emit_line(&mut visit, &[b"state-health"])?;
        emit_line(
            &mut visit,
            &[b"  reports state policy, migration, backend, and writeback health"],
        )?;
        return Ok(());
    }
    if bytes_eq(topic, b"halt") {
        emit_line(&mut visit, &[b"halt"])?;
        emit_line(
            &mut visit,
            &[b"  drains state clients, stops services, and powers off cleanly"],
        )?;
        return Ok(());
    }

    Err(Error::new(b"operator help rejected: unknown command"))
}

pub fn overview<F>(report: &[u8], mut visit: F) -> Result<()>
where
    F: FnMut(&[u8]),
{
    let generation = active_generation(report)?;
    let generation_line = require_operator_generation(report, generation)?;
    emit_line(&mut visit, &[b"overview generation=", generation])?;
    emit_line(&mut visit, &[b"policy_hash=", active_policy_hash(report)?])?;
    emit_line(&mut visit, &[b"graph_hash=", active_graph_hash(report)?])?;
    emit_line(
        &mut visit,
        &[
            b"inventory services=",
            required_field(generation_line, b"services=", b"overview missing services")?,
            b" capabilities=",
            required_field(
                generation_line,
                b"capabilities=",
                b"overview missing capabilities",
            )?,
            b" states=",
            required_field(generation_line, b"states=", b"overview missing states")?,
            b" devices=",
            required_field(generation_line, b"devices=", b"overview missing devices")?,
        ],
    )?;
    emit_line(
        &mut visit,
        &[b"try: services | capabilities | service svc:console-shell"],
    )?;
    Ok(())
}

pub fn services<F>(report: &[u8], mut visit: F) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let generation = active_generation(report)?;
    emit_line(&mut visit, &[b"services generation=", generation])?;
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-service[") && field_eq(line, b"generation=", generation) {
            match emit_service_summary(report, line, &mut visit) {
                Ok(()) => count += 1,
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if count == 0 {
        return Err(Error::new(b"operator services rejected: no services"));
    }
    Ok(count)
}

pub fn service_detail<F>(report: &[u8], command: &[u8], mut visit: F) -> Result<()>
where
    F: FnMut(&[u8]),
{
    let selector = word_or_error(command, 1, b"operator service missing selector")?;
    if word_at(command, 2).is_some() {
        return Err(Error::new(b"operator service rejected: too many arguments"));
    }
    let generation = active_generation(report)?;
    let service_line = operator_service_line_for_selector(report, generation, selector)?;
    let service = required_field(service_line, b"id=", b"operator service missing id")?;
    let process = required_field(
        service_line,
        b"process=",
        b"operator service missing process",
    )?;
    let process_line = find_process_line(report, process).ok_or(Error::new(
        b"operator service rejected: live process missing",
    ))?;
    emit_line(
        &mut visit,
        &[b"service ", service, b" generation=", generation],
    )?;
    emit_line(
        &mut visit,
        &[
            b"process=",
            process,
            b" state=",
            required_field(
                process_line,
                b"state=",
                b"operator service missing process state",
            )?,
            b" restart=",
            required_field(
                service_line,
                b"restart=",
                b"operator service missing restart",
            )?,
            b" mount_root=",
            required_field(
                service_line,
                b"mount_root=",
                b"operator service missing mount root",
            )?,
        ],
    )?;
    emit_service_requirements(report, generation, service, &mut visit)?;
    emit_service_state_paths(report, generation, service, &mut visit)?;
    Ok(())
}

pub fn capabilities<F>(report: &[u8], command: &[u8], mut visit: F) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let generation = active_generation(report)?;
    if word_at(command, 1).is_none() {
        emit_line(&mut visit, &[b"capabilities generation=", generation])?;
        return emit_capability_list(report, generation, &mut visit);
    }

    let keyword = word_or_error(command, 1, b"operator capabilities missing selector")?;
    if !bytes_eq(keyword, b"for") {
        return Err(Error::new(
            b"operator capabilities usage: capabilities for <service>",
        ));
    }
    let selector = word_or_error(command, 2, b"operator capabilities missing service")?;
    if word_at(command, 3).is_some() {
        return Err(Error::new(
            b"operator capabilities rejected: too many arguments",
        ));
    }
    let service_line = operator_service_line_for_selector(report, generation, selector)?;
    let service = required_field(service_line, b"id=", b"operator service missing id")?;
    emit_line(
        &mut visit,
        &[b"capabilities for ", service, b" generation=", generation],
    )?;
    emit_service_capabilities(report, generation, service, &mut visit)
}

pub fn capability_detail<F>(report: &[u8], command: &[u8], mut visit: F) -> Result<()>
where
    F: FnMut(&[u8]),
{
    let capability = word_or_error(command, 1, b"operator capability missing id")?;
    if word_at(command, 2).is_some() {
        return Err(Error::new(
            b"operator capability rejected: too many arguments",
        ));
    }
    let generation = active_generation(report)?;
    let line = require_operator_capability(report, generation, capability)?;
    emit_line(
        &mut visit,
        &[b"capability ", capability, b" generation=", generation],
    )?;
    emit_line(
        &mut visit,
        &[
            b"provider=",
            required_field(line, b"provider=", b"operator capability missing provider")?,
            b" rights=",
            required_field(line, b"rights=", b"operator capability missing rights")?,
        ],
    )?;
    emit_line(
        &mut visit,
        &[
            b"object_kind=",
            required_field(
                line,
                b"object_kind=",
                b"operator capability missing object kind",
            )?,
            b" object=",
            required_field(line, b"object=", b"operator capability missing object")?,
        ],
    )?;
    emit_capability_consumers(report, generation, capability, &mut visit)?;
    Ok(())
}

pub fn states<F>(report: &[u8], mut visit: F) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let generation = active_generation(report)?;
    emit_line(&mut visit, &[b"states generation=", generation])?;
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-state[") && field_eq(line, b"generation=", generation) {
            match emit_state_summary(line, &mut visit) {
                Ok(()) => count += 1,
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    Ok(count)
}

pub fn state_detail<F>(report: &[u8], command: &[u8], mut visit: F) -> Result<()>
where
    F: FnMut(&[u8]),
{
    let state = word_or_error(command, 1, b"operator state missing id")?;
    if word_at(command, 2).is_some() {
        return Err(Error::new(b"operator state rejected: too many arguments"));
    }
    let generation = active_generation(report)?;
    let line = operator_state_line(report, generation, state)
        .ok_or(Error::new(b"operator rejected: unknown state"))?;
    emit_line(&mut visit, &[b"state ", state, b" generation=", generation])?;
    emit_state_summary(line, &mut visit)?;
    emit_state_paths(report, generation, state, &mut visit)?;
    Ok(())
}

pub fn devices<F>(report: &[u8], mut visit: F) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let generation = active_generation(report)?;
    let generation_line = require_operator_generation(report, generation)?;
    emit_line(
        &mut visit,
        &[
            b"devices generation=",
            generation,
            b" declared=",
            required_field(
                generation_line,
                b"devices=",
                b"operator devices missing count",
            )?,
        ],
    )?;
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-node[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"kind=", b"device")
        {
            match emit_device_summary(line, &mut visit) {
                Ok(()) => count += 1,
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if count == 0 {
        emit_line(&mut visit, &[b"device graph nodes: <none>"])?;
    }
    Ok(count)
}

pub fn device_detail<F>(report: &[u8], command: &[u8], mut visit: F) -> Result<()>
where
    F: FnMut(&[u8]),
{
    let device = word_or_error(command, 1, b"operator device missing id")?;
    if word_at(command, 2).is_some() {
        return Err(Error::new(b"operator device rejected: too many arguments"));
    }
    let generation = active_generation(report)?;
    let line = operator_node_line(report, generation, b"device", device)
        .ok_or(Error::new(b"operator rejected: unknown device"))?;
    emit_line(
        &mut visit,
        &[b"device ", device, b" generation=", generation],
    )?;
    emit_device_summary(line, &mut visit)?;
    emit_device_capabilities(report, generation, device, &mut visit)?;
    Ok(())
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
        last_failed: required_field(
            manager,
            b"last_failed=",
            b"generation-status missing last-failed",
        )?,
        transaction: required_field(
            manager,
            b"transaction=",
            b"generation-status missing transaction",
        )?,
        target: required_field(manager, b"target=", b"generation-status missing target")?,
        failure_reason: required_field(
            manager,
            b"failure_reason=",
            b"generation-status missing failure reason",
        )?,
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

pub fn package_list<F>(report: &[u8], mut visit: F) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let generation = active_generation(report)?;
    let generation_line = require_operator_generation(report, generation)?;
    let facts = required_field(
        generation_line,
        b"package_facts=",
        b"operator package-list missing facts",
    )?;
    if bytes_eq(facts, b"absent") {
        return Err(Error::new(
            b"operator package-list unavailable: no native package facts",
        ));
    }
    if !bytes_eq(facts, b"graph-v1") {
        return Err(Error::new(
            b"operator package-list rejected: unsupported package fact encoding",
        ));
    }
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-package[") && field_eq(line, b"generation=", generation) {
            let package = match required_field(line, b"id=", b"operator package missing id") {
                Ok(value) => value,
                Err(err) => {
                    error = Some(err);
                    return;
                }
            };
            let graph_hash = match required_field(
                line,
                b"graph_hash=",
                b"operator package missing graph hash",
            ) {
                Ok(value) => value,
                Err(err) => {
                    error = Some(err);
                    return;
                }
            };
            match emit_line(
                &mut visit,
                &[b"package ", package, b" generation=", generation],
            ) {
                Ok(()) => match emit_line(
                    &mut visit,
                    &[b"package-hash ", package, b" graph_hash=", graph_hash],
                ) {
                    Ok(()) => count += 1,
                    Err(err) => error = Some(err),
                },
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if count == 0 {
        return Err(Error::new(
            b"operator package-list rejected: package facts empty",
        ));
    }
    Ok(count)
}

pub fn verify_system(report: &[u8]) -> Result<SystemVerification<'_>> {
    let operator_report = operator_report_line(report)?;
    let generation = active_generation(report)?;
    let generation_line = require_operator_generation(report, generation)?;
    require_field_value(
        generation_line,
        b"active=",
        b"yes",
        b"operator verifier rejected: generation is not active",
    )?;
    require_field_value(
        generation_line,
        b"selected=",
        b"yes",
        b"operator verifier rejected: selected generation mismatch",
    )?;
    require_field_value(
        generation_line,
        b"package_facts=",
        b"graph-v1",
        b"operator verifier rejected: missing package facts",
    )?;
    let policy_hash = required_field(
        generation_line,
        b"policy_hash=",
        b"operator verifier missing policy hash",
    )?;
    let graph_hash = required_field(
        generation_line,
        b"graph_hash=",
        b"operator verifier missing graph hash",
    )?;
    require_field_value(
        operator_report,
        b"policy_hash=",
        policy_hash,
        b"operator verifier rejected: operator policy hash mismatch",
    )?;
    require_field_value(
        operator_report,
        b"graph_hash=",
        graph_hash,
        b"operator verifier rejected: operator graph hash mismatch",
    )?;
    let manager = generation_manager_line(report)?;
    require_field_value(
        manager,
        b"selected=",
        generation,
        b"operator verifier rejected: manager selected generation mismatch",
    )?;
    let graph_store = find_line_contains_all(report, &[b"graph-store v=1", b"generation="]).ok_or(
        Error::new(b"operator verifier rejected: graph store missing"),
    )?;
    require_field_value(
        graph_store,
        b"generation=",
        generation,
        b"operator verifier rejected: graph store generation mismatch",
    )?;
    require_field_value(
        graph_store,
        b"hash=",
        graph_hash,
        b"operator verifier rejected: graph store hash mismatch",
    )?;
    let policy = find_line_contains_all(report, &[b"policy-validation v=1", b"generation="])
        .ok_or(Error::new(
            b"operator verifier rejected: policy report missing",
        ))?;
    require_field_value(
        policy,
        b"generation=",
        generation,
        b"operator verifier rejected: policy generation mismatch",
    )?;
    require_field_value(
        policy,
        b"status=",
        b"accepted",
        b"operator verifier rejected: policy not accepted",
    )?;
    require_field_value(
        policy,
        b"hash=",
        policy_hash,
        b"operator verifier rejected: policy hash mismatch",
    )?;
    require_field_value(
        policy,
        b"capabilities=",
        required_field(
            generation_line,
            b"capabilities=",
            b"operator verifier missing capability count",
        )?,
        b"operator verifier rejected: policy capability count mismatch",
    )?;
    require_zero_field(
        report,
        b"objects_unreachable=",
        b"operator verifier rejected: unreachable objects",
    )?;
    verify_package_facts(report, generation, generation_line, graph_hash)?;
    verify_active_services(report, generation)?;
    verify_active_capabilities(report, generation)?;
    verify_state_health(report, generation)?;
    Ok(SystemVerification {
        generation,
        policy_hash,
        graph_hash,
        services: parse_u64_field(
            generation_line,
            b"services=",
            b"operator verifier missing service count",
        )?,
        capabilities: parse_u64_field(
            generation_line,
            b"capabilities=",
            b"operator verifier missing capability count",
        )?,
        states: parse_u64_field(
            generation_line,
            b"states=",
            b"operator verifier missing state count",
        )?,
        devices: parse_u64_field(
            generation_line,
            b"devices=",
            b"operator verifier missing device count",
        )?,
        packages: parse_u64_field(
            generation_line,
            b"packages=",
            b"operator verifier missing package count",
        )?,
    })
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

fn emit_service_summary<F>(report: &[u8], line: &[u8], visit: &mut F) -> Result<()>
where
    F: FnMut(&[u8]),
{
    let id = required_field(line, b"id=", b"operator service missing id")?;
    let process = required_field(line, b"process=", b"operator service missing process")?;
    let process_line = find_process_line(report, process).ok_or(Error::new(
        b"operator services rejected: live process missing",
    ))?;
    emit_line(
        visit,
        &[
            id,
            b" process=",
            process,
            b" state=",
            required_field(
                process_line,
                b"state=",
                b"operator services missing process state",
            )?,
            b" restart=",
            required_field(line, b"restart=", b"operator services missing restart")?,
        ],
    )
}

fn emit_service_requirements<F>(
    report: &[u8],
    generation: &[u8],
    service: &[u8],
    visit: &mut F,
) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-requirement[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"service=", service)
        {
            match emit_requirement_with_provider(report, generation, line, visit) {
                Ok(()) => count += 1,
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if count == 0 {
        emit_line(visit, &[b"requires <none>"])?;
    }
    Ok(count)
}

fn emit_requirement_with_provider<F>(
    report: &[u8],
    generation: &[u8],
    line: &[u8],
    visit: &mut F,
) -> Result<()>
where
    F: FnMut(&[u8]),
{
    let capability = required_field(
        line,
        b"capability=",
        b"operator requirement missing capability",
    )?;
    let capability_line = require_operator_capability(report, generation, capability)?;
    emit_line(
        visit,
        &[
            b"requires ",
            capability,
            b" rights=",
            required_field(line, b"rights=", b"operator requirement missing rights")?,
            b" provider=",
            required_field(
                capability_line,
                b"provider=",
                b"operator capability missing provider",
            )?,
        ],
    )
}

fn emit_service_state_paths<F>(
    report: &[u8],
    generation: &[u8],
    service: &[u8],
    visit: &mut F,
) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-state-path[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"service=", service)
        {
            match emit_line(
                visit,
                &[
                    b"state ",
                    match required_field(line, b"state=", b"operator state path missing state") {
                        Ok(value) => value,
                        Err(err) => {
                            error = Some(err);
                            return;
                        }
                    },
                    b" root=",
                    match required_field(line, b"root=", b"operator state path missing root") {
                        Ok(value) => value,
                        Err(err) => {
                            error = Some(err);
                            return;
                        }
                    },
                    b" rights=",
                    match required_field(line, b"rights=", b"operator state path missing rights") {
                        Ok(value) => value,
                        Err(err) => {
                            error = Some(err);
                            return;
                        }
                    },
                ],
            ) {
                Ok(()) => count += 1,
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if count == 0 {
        emit_line(visit, &[b"state <none>"])?;
    }
    Ok(count)
}

fn emit_capability_list<F>(report: &[u8], generation: &[u8], visit: &mut F) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-capability[") && field_eq(line, b"generation=", generation)
        {
            match emit_capability_summary(report, generation, line, visit) {
                Ok(()) => count += 1,
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    Ok(count)
}

fn emit_capability_summary<F>(
    report: &[u8],
    generation: &[u8],
    line: &[u8],
    visit: &mut F,
) -> Result<()>
where
    F: FnMut(&[u8]),
{
    let capability = required_field(line, b"id=", b"operator capability missing id")?;
    let mut buffer = [0u8; 128];
    let mut len = 0;
    append_output(&mut buffer, &mut len, capability)?;
    append_output(&mut buffer, &mut len, b" provider=")?;
    append_output(
        &mut buffer,
        &mut len,
        required_field(line, b"provider=", b"operator capability missing provider")?,
    )?;
    append_output(&mut buffer, &mut len, b" rights=")?;
    append_output(
        &mut buffer,
        &mut len,
        required_field(line, b"rights=", b"operator capability missing rights")?,
    )?;
    append_output(&mut buffer, &mut len, b" consumers=")?;
    append_u64_output(
        &mut buffer,
        &mut len,
        count_capability_consumers(report, generation, capability),
    )?;
    visit(&buffer[..len]);
    Ok(())
}

fn emit_service_capabilities<F>(
    report: &[u8],
    generation: &[u8],
    service: &[u8],
    visit: &mut F,
) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-requirement[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"service=", service)
        {
            match emit_requirement_with_provider(report, generation, line, visit) {
                Ok(()) => count += 1,
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if count == 0 {
        emit_line(visit, &[b"capabilities <none>"])?;
    }
    Ok(count)
}

fn emit_capability_consumers<F>(
    report: &[u8],
    generation: &[u8],
    capability: &[u8],
    visit: &mut F,
) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-requirement[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"capability=", capability)
        {
            match emit_line(
                visit,
                &[
                    b"consumer ",
                    match required_field(line, b"service=", b"operator requirement missing service")
                    {
                        Ok(value) => value,
                        Err(err) => {
                            error = Some(err);
                            return;
                        }
                    },
                    b" rights=",
                    match required_field(line, b"rights=", b"operator requirement missing rights") {
                        Ok(value) => value,
                        Err(err) => {
                            error = Some(err);
                            return;
                        }
                    },
                ],
            ) {
                Ok(()) => count += 1,
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if count == 0 {
        emit_line(visit, &[b"consumers <none>"])?;
    }
    Ok(count)
}

fn emit_state_summary<F>(line: &[u8], visit: &mut F) -> Result<()>
where
    F: FnMut(&[u8]),
{
    emit_line(
        visit,
        &[
            required_field(line, b"id=", b"operator state missing id")?,
            b" owner=",
            required_field(line, b"owner=", b"operator state missing owner")?,
            b" schema=",
            required_field(line, b"schema=", b"operator state missing schema")?,
            b" storage=",
            required_field(line, b"storage=", b"operator state missing storage")?,
        ],
    )
}

fn emit_state_paths<F>(report: &[u8], generation: &[u8], state: &[u8], visit: &mut F) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-state-path[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"state=", state)
        {
            match emit_line(
                visit,
                &[
                    b"service ",
                    match required_field(line, b"service=", b"operator state path missing service")
                    {
                        Ok(value) => value,
                        Err(err) => {
                            error = Some(err);
                            return;
                        }
                    },
                    b" root=",
                    match required_field(line, b"root=", b"operator state path missing root") {
                        Ok(value) => value,
                        Err(err) => {
                            error = Some(err);
                            return;
                        }
                    },
                    b" rights=",
                    match required_field(line, b"rights=", b"operator state path missing rights") {
                        Ok(value) => value,
                        Err(err) => {
                            error = Some(err);
                            return;
                        }
                    },
                ],
            ) {
                Ok(()) => count += 1,
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if count == 0 {
        emit_line(visit, &[b"paths <none>"])?;
    }
    Ok(count)
}

fn emit_device_summary<F>(line: &[u8], visit: &mut F) -> Result<()>
where
    F: FnMut(&[u8]),
{
    emit_line(
        visit,
        &[
            required_field(line, b"id=", b"operator device missing id")?,
            b" object_kind=",
            required_field(
                line,
                b"object_kind=",
                b"operator device missing object kind",
            )?,
            b" label=",
            required_field(line, b"label=", b"operator device missing label")?,
        ],
    )
}

fn emit_device_capabilities<F>(
    report: &[u8],
    generation: &[u8],
    device: &[u8],
    visit: &mut F,
) -> Result<u64>
where
    F: FnMut(&[u8]),
{
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-capability[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"provider=", device)
        {
            match emit_line(
                visit,
                &[
                    b"capability ",
                    match required_field(line, b"id=", b"operator capability missing id") {
                        Ok(value) => value,
                        Err(err) => {
                            error = Some(err);
                            return;
                        }
                    },
                    b" rights=",
                    match required_field(line, b"rights=", b"operator capability missing rights") {
                        Ok(value) => value,
                        Err(err) => {
                            error = Some(err);
                            return;
                        }
                    },
                ],
            ) {
                Ok(()) => count += 1,
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if count == 0 {
        emit_line(visit, &[b"capabilities <none>"])?;
    }
    Ok(count)
}

fn count_capability_consumers(report: &[u8], generation: &[u8], capability: &[u8]) -> u64 {
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-requirement[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"capability=", capability)
        {
            count += 1;
        }
    });
    count
}

fn operator_service_line_for_selector<'a>(
    report: &'a [u8],
    generation: &[u8],
    selector: &[u8],
) -> Result<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-service[")
            && field_eq(line, b"generation=", generation)
            && if starts_with(selector, b"svc:") {
                field_eq(line, b"id=", selector)
            } else {
                field_eq(line, b"process=", selector)
            }
    })
    .ok_or(Error::new(b"operator rejected: unknown service"))
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

fn active_graph_hash(report: &[u8]) -> Result<&[u8]> {
    required_field(
        operator_report_line(report)?,
        b"graph_hash=",
        b"operator report missing graph hash",
    )
}

fn require_operator_generation<'a>(report: &'a [u8], generation: &[u8]) -> Result<&'a [u8]> {
    find_line_where(report, |line| {
        starts_with(line, b"operator-generation[") && field_eq(line, b"id=", generation)
    })
    .ok_or(Error::new(b"operator rejected: unknown generation"))
}

fn generation_manager_line(report: &[u8]) -> Result<&[u8]> {
    find_line_contains_all(report, &[b"generation-manager v=1", b"selected="]).ok_or(Error::new(
        b"operator verifier rejected: generation manager missing",
    ))
}

fn require_field_value(
    line: &[u8],
    prefix: &[u8],
    expected: &[u8],
    message: &'static [u8],
) -> Result<()> {
    if field_eq(line, prefix, expected) {
        Ok(())
    } else {
        Err(Error::new(message))
    }
}

fn require_zero_field(report: &[u8], prefix: &[u8], message: &'static [u8]) -> Result<()> {
    let line = find_line_contains_all(report, &[prefix]).ok_or(Error::new(message))?;
    let value = parse_u64_field(line, prefix, message)?;
    if value == 0 {
        Ok(())
    } else {
        Err(Error::new(message))
    }
}

fn verify_package_facts(
    report: &[u8],
    generation: &[u8],
    generation_line: &[u8],
    graph_hash: &[u8],
) -> Result<()> {
    let declared = parse_u64_field(
        generation_line,
        b"packages=",
        b"operator verifier missing package count",
    )?;
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-package[") && field_eq(line, b"generation=", generation) {
            let id = match required_field(
                line,
                b"id=",
                b"operator verifier rejected: package fact missing id",
            ) {
                Ok(value) => value,
                Err(err) => {
                    error = Some(err);
                    return;
                }
            };
            if required_field(
                line,
                b"label=",
                b"operator verifier rejected: package fact missing label",
            )
            .is_err()
            {
                error = Some(Error::new(
                    b"operator verifier rejected: package fact missing label",
                ));
                return;
            }
            if !field_eq(line, b"graph_hash=", graph_hash) {
                error = Some(Error::new(
                    b"operator verifier rejected: package graph hash mismatch",
                ));
                return;
            }
            if operator_package_id_count(report, generation, id) != 1 {
                error = Some(Error::new(
                    b"operator verifier rejected: duplicate package fact",
                ));
                return;
            }
            count += 1;
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if declared == 0 || count == 0 {
        return Err(Error::new(b"operator verifier rejected: no package facts"));
    }
    if count != declared {
        return Err(Error::new(
            b"operator verifier rejected: package count mismatch",
        ));
    }
    Ok(())
}

fn operator_package_id_count(report: &[u8], generation: &[u8], id: &[u8]) -> u64 {
    let mut count = 0;
    for_each_line(report, |line| {
        if starts_with(line, b"operator-package[")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"id=", id)
        {
            count += 1;
        }
    });
    count
}

fn verify_active_services(report: &[u8], generation: &[u8]) -> Result<()> {
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-service[") && field_eq(line, b"generation=", generation) {
            let process = match required_field(
                line,
                b"process=",
                b"operator verifier rejected: service missing process",
            ) {
                Ok(value) => value,
                Err(err) => {
                    error = Some(err);
                    return;
                }
            };
            let Some(process_line) = find_process_line(report, process) else {
                error = Some(Error::new(
                    b"operator verifier rejected: graph service has no process",
                ));
                return;
            };
            if !field_eq(process_line, b"generation=", generation) {
                error = Some(Error::new(
                    b"operator verifier rejected: process generation mismatch",
                ));
                return;
            }
            let state = match required_field(
                process_line,
                b"state=",
                b"operator verifier rejected: process missing state",
            ) {
                Ok(value) => value,
                Err(err) => {
                    error = Some(err);
                    return;
                }
            };
            if !process_state_is_live(state) {
                return;
            }
            if !field_eq(process_line, b"context_reaped=", b"no") {
                error = Some(Error::new(
                    b"operator verifier rejected: service process context reaped",
                ));
                return;
            }
            count += 1;
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if count == 0 {
        return Err(Error::new(
            b"operator verifier rejected: no active graph services",
        ));
    }
    Ok(())
}

fn process_state_is_live(state: &[u8]) -> bool {
    bytes_eq(state, b"ready")
        || bytes_eq(state, b"running")
        || bytes_eq(state, b"blocked")
        || bytes_eq(state, b"blocked-irq")
        || bytes_eq(state, b"blocked-vfs")
        || bytes_eq(state, b"blocked-vfs-state")
        || bytes_eq(state, b"blocked-vertexfs-sync")
        || bytes_eq(state, b"blocked-net")
        || bytes_eq(state, b"sleeping")
}

fn verify_active_capabilities(report: &[u8], generation: &[u8]) -> Result<()> {
    verify_live_capability_provenance(report, generation)?;
    verify_required_capabilities(report, generation)
}

fn verify_live_capability_provenance(report: &[u8], generation: &[u8]) -> Result<()> {
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"space=")
            && field_eq(line, b"generation=", generation)
            && field_eq(line, b"revoked=", b"no")
            && live_capability_has_graph_backed_object(line)
            && (field_eq(line, b"graph_from=", b"<unknown>")
                || field_eq(line, b"graph_target=", b"<unknown>"))
        {
            error = Some(Error::new(
                b"operator verifier rejected: live cap missing graph provenance",
            ));
        }
    });
    if let Some(err) = error {
        Err(err)
    } else {
        Ok(())
    }
}

fn verify_required_capabilities(report: &[u8], generation: &[u8]) -> Result<()> {
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"operator-requirement[") && field_eq(line, b"generation=", generation)
        {
            match verify_required_capability(report, generation, line) {
                Ok(true) => count += 1,
                Ok(false) => {}
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if count == 0 {
        return Err(Error::new(
            b"operator verifier rejected: no active policy requirements",
        ));
    }
    Ok(())
}

fn verify_required_capability(
    report: &[u8],
    generation: &[u8],
    requirement: &[u8],
) -> Result<bool> {
    let service = required_field(
        requirement,
        b"service=",
        b"operator verifier rejected: requirement missing service",
    )?;
    let Some(process) = live_service_process(report, generation, service)? else {
        return Ok(false);
    };
    let capability = required_field(
        requirement,
        b"capability=",
        b"operator verifier rejected: requirement missing capability",
    )?;
    let requirement_rights = required_field(
        requirement,
        b"rights=",
        b"operator verifier rejected: requirement missing rights",
    )?;
    let capability_line = require_operator_capability(report, generation, capability)
        .map_err(|_| Error::new(b"operator verifier rejected: missing policy capability"))?;
    let capability_rights = required_field(
        capability_line,
        b"rights=",
        b"operator verifier rejected: capability missing rights",
    )?;
    if !rights_cover(capability_rights, requirement_rights) {
        return Err(Error::new(
            b"operator verifier rejected: requirement rights exceed capability rights",
        ));
    }
    let object = required_field(
        capability_line,
        b"object=",
        b"operator verifier rejected: capability missing object",
    )?;
    if !operator_edge_exists(report, generation, object, requirement_rights) {
        return Err(Error::new(
            b"operator verifier rejected: missing graph capability edge",
        ));
    }
    if live_capability_matches(
        report,
        generation,
        process,
        service,
        object,
        requirement_rights,
    ) {
        Ok(true)
    } else {
        Err(Error::new(
            b"operator verifier rejected: required live capability missing",
        ))
    }
}

fn live_service_process<'a>(
    report: &'a [u8],
    generation: &[u8],
    service: &[u8],
) -> Result<Option<&'a [u8]>> {
    let process = operator_service_process(report, generation, service)
        .map_err(|_| Error::new(b"operator verifier rejected: requirement service missing"))?;
    let Some(process_line) = find_process_line(report, process) else {
        return Err(Error::new(
            b"operator verifier rejected: graph service has no process",
        ));
    };
    if !field_eq(process_line, b"generation=", generation) {
        return Err(Error::new(
            b"operator verifier rejected: process generation mismatch",
        ));
    }
    let state = required_field(
        process_line,
        b"state=",
        b"operator verifier rejected: process missing state",
    )?;
    if !process_state_is_live(state) {
        return Ok(None);
    }
    if !field_eq(process_line, b"context_reaped=", b"no") {
        return Err(Error::new(
            b"operator verifier rejected: service process context reaped",
        ));
    }
    Ok(Some(process))
}

fn live_capability_has_graph_backed_object(line: &[u8]) -> bool {
    contains_all(line, &[b"endpoint="])
        || contains_all(line, &[b"store-object="])
        || contains_all(line, &[b"config="])
        || contains_all(line, &[b"state-volume="])
        || contains_all(line, &[b"timer="])
        || contains_all(line, &[b"network-port="])
        || contains_all(line, &[b"io-port="])
        || contains_all(line, &[b"mmio="])
        || contains_all(line, &[b"framebuffer="])
        || contains_all(line, &[b"interrupt-line="])
        || contains_all(line, &[b"dma-region="])
        || contains_all(line, &[b"pci-device="])
        || contains_all(line, &[b"virtio-device="])
        || contains_all(line, &[b"namespace="])
        || contains_all(line, &[b"vfs-root="])
        || contains_all(line, &[b"secret="])
}

fn verify_state_health(report: &[u8], generation: &[u8]) -> Result<()> {
    let mut count = 0;
    let mut error = None;
    for_each_line(report, |line| {
        if error.is_some() {
            return;
        }
        if starts_with(line, b"state-health[") && field_eq(line, b"generation=", generation) {
            if !field_eq(line, b"migration_status=", b"clean")
                || !field_eq(line, b"last_error=", b"none")
            {
                error = Some(Error::new(
                    b"operator verifier rejected: state health is not clean",
                ));
                return;
            }
            count += 1;
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    if count == 0 {
        return Err(Error::new(
            b"operator verifier rejected: no state health records",
        ));
    }
    Ok(())
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
        operator_edge_line_matches(line, generation, object, required_rights)
    })
    .ok_or(Error::new(
        b"operator rejected: missing graph capability edge",
    ))
}

fn operator_edge_exists(
    report: &[u8],
    generation: &[u8],
    object: &[u8],
    required_rights: &[u8],
) -> bool {
    find_line_where(report, |line| {
        operator_edge_line_matches(line, generation, object, required_rights)
    })
    .is_some()
}

fn operator_edge_line_matches(
    line: &[u8],
    generation: &[u8],
    object: &[u8],
    required_rights: &[u8],
) -> bool {
    if !(starts_with(line, b"operator-edge[")
        && field_eq(line, b"generation=", generation)
        && field_eq(line, b"kind=", b"capability")
        && field_eq(line, b"to=", object))
    {
        return false;
    }
    field_slice(line, b"rights=")
        .is_some_and(|edge_rights| rights_cover(edge_rights, required_rights))
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
    if live_capability_matches(
        report,
        generation,
        process,
        service,
        object,
        required_rights,
    ) {
        Ok(())
    } else {
        Err(Error::new(
            b"operator rejected: live capability missing or insufficient",
        ))
    }
}

fn live_capability_matches(
    report: &[u8],
    generation: &[u8],
    process: &[u8],
    service: &[u8],
    object: &[u8],
    required_rights: &[u8],
) -> bool {
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
    accepted
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

fn parse_u64_field(line: &[u8], prefix: &[u8], message: &'static [u8]) -> Result<u64> {
    let value = required_field(line, prefix, message)?;
    let mut parsed = 0u64;
    let mut index = 0;
    if value.is_empty() {
        return Err(Error::new(message));
    }
    while index < value.len() {
        let byte = value[index];
        if !byte.is_ascii_digit() {
            return Err(Error::new(message));
        }
        parsed = parsed
            .checked_mul(10)
            .and_then(|current| current.checked_add((byte - b'0') as u64))
            .ok_or(Error::new(message))?;
        index += 1;
    }
    Ok(parsed)
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

fn is_command_space(byte: u8) -> bool {
    byte == b' ' || byte == b'\t' || byte == b'\r' || byte == b'\n'
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

fn emit_line<F>(visit: &mut F, parts: &[&[u8]]) -> Result<()>
where
    F: FnMut(&[u8]),
{
    let mut buffer = [0u8; 128];
    let mut len = 0;
    let mut index = 0;
    while index < parts.len() {
        append_output(&mut buffer, &mut len, parts[index])?;
        index += 1;
    }
    visit(&buffer[..len]);
    Ok(())
}

fn append_output(buffer: &mut [u8], len: &mut usize, value: &[u8]) -> Result<()> {
    let mut index = 0;
    while index < value.len() {
        if *len >= buffer.len() {
            return Err(Error::new(b"operator output line too large"));
        }
        buffer[*len] = value[index];
        *len += 1;
        index += 1;
    }
    Ok(())
}

fn append_u64_output(buffer: &mut [u8], len: &mut usize, value: u64) -> Result<()> {
    if value == 0 {
        return append_output(buffer, len, b"0");
    }
    let mut digits = [0u8; 20];
    let mut digit_count = 0;
    let mut remaining = value;
    while remaining > 0 {
        digits[digit_count] = b'0' + (remaining % 10) as u8;
        digit_count += 1;
        remaining /= 10;
    }
    while digit_count > 0 {
        digit_count -= 1;
        if *len >= buffer.len() {
            return Err(Error::new(b"operator output line too large"));
        }
        buffer[*len] = digits[digit_count];
        *len += 1;
    }
    Ok(())
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

    const DISCOVERY_REPORT: &[u8] = b"operator-report v=1 active=gen:a registered=1 policy_hash=hash:policy graph_hash=hash:graph
operator-generation[0] id=gen:a active=yes selected=yes previous=no known_good=yes policy_hash=hash:policy graph_hash=hash:graph services=1 capabilities=1 states=1 devices=1 packages=0 package_facts=absent
operator-service[0] generation=gen:a id=svc:echo-server process=echo restart=on-failure mount_root=/state
process[0] name=echo pid=2 state=running restart_policy=on-failure mount_root=/state context_reaped=no cr3=0 generation=gen:a graph_node=svc:echo-server
operator-requirement[0] generation=gen:a service=svc:echo-server capability=cap:log.sink rights=send
operator-capability[0] generation=gen:a id=cap:log.sink provider=svc:logd object_kind=endpoint object=log-sink rights=send
operator-state[0] generation=gen:a id=state:counter owner=svc:echo-server schema=counter.v1 storage=vertexdisk-v1 migration=preserve retention=retain-while-referenced sharing=explicit
operator-state-path[0] generation=gen:a service=svc:echo-server state=state:counter root=/state rights=read|write|resolve
operator-node[0] generation=gen:a kind=device id=device:virtio-blk0 object_kind=virtio-device label=device:virtio-blk0
";

    const VERIFY_OK_REPORT: &[u8] = b"native-runtime-report v=1
generation=gen:a
generation-manager v=1 selected=gen:a previous=none known_good=gen:a last_failed=none transaction=idle target=none failure_reason=none
graph-store v=1 generation=gen:a hash=hash:graph checksum=1 nodes=1 edges=1 source=test
policy-validation v=1 generation=gen:a status=accepted version=1 hash=hash:policy capabilities=1 requirements=1 provides=0 mounts=0 state_paths=0 bootstraps=0
operator-report v=1 active=gen:a registered=1 policy_hash=hash:policy graph_hash=hash:graph
operator-generation[0] id=gen:a active=yes selected=yes previous=no known_good=yes policy_hash=hash:policy graph_hash=hash:graph services=1 capabilities=1 states=1 devices=0 packages=1 package_facts=graph-v1
operator-package[0.0] generation=gen:a id=pkg:logd label=pkg:logd graph_hash=hash:graph
operator-service[0] generation=gen:a id=svc:echo-server process=echo restart=on-failure mount_root=/state
process[0] name=echo pid=2 state=running restart_policy=on-failure mount_root=/state context_reaped=no cr3=0 generation=gen:a graph_node=svc:echo-server
operator-requirement[0] generation=gen:a service=svc:echo-server capability=cap:log.sink rights=send
operator-capability[0] generation=gen:a id=cap:log.sink provider=svc:logd object_kind=endpoint object=log-sink rights=send
operator-edge[0] generation=gen:a kind=capability id=edge:echo-log from=svc:echo-server to=log-sink rights=send
objects_unreachable=0
state-health[0] generation=gen:a state=state:counter owner=svc:echo-server schema=counter.v1 migration_status=clean last_error=none
space=initial proc=echo cap[0] endpoint=log-sink rights=send cap_id=1 parent_cap_id=0 generation=gen:a graph_from=svc:echo-server graph_target=log-sink graph_edge=edge:echo-log owner_pid=2 owner=echo delegated_by_pid=1 delegated_by=vertex-init revoked=no
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

    #[test]
    fn command_normalization_trims_collapses_and_lowercases_only_the_verb() {
        let mut output = [0u8; 64];
        let command = normalize_command(b" \tHeLP   Service:Mixed\t ", &mut output).unwrap();
        assert_eq!(command, b"help Service:Mixed");
    }

    #[test]
    fn help_is_grouped_without_duplicate_command_inventories() {
        let mut lines = std::vec::Vec::<std::vec::Vec<u8>>::new();
        help(b"help", |line| lines.push(line.to_vec())).unwrap();

        assert_eq!(
            lines.first().map(std::vec::Vec::as_slice),
            Some(b"Vertex OS operator console commands".as_slice())
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with(b"discover  overview"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with(b"change    activate"))
        );
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.starts_with(b"utility   generation"))
                .count(),
            1
        );
    }

    #[test]
    fn discovery_renderers_emit_operator_inventory() {
        let mut lines = std::vec::Vec::<std::vec::Vec<u8>>::new();
        overview(DISCOVERY_REPORT, |line| lines.push(line.to_vec())).unwrap();
        services(DISCOVERY_REPORT, |line| lines.push(line.to_vec())).unwrap();
        service_detail(DISCOVERY_REPORT, b"service svc:echo-server", |line| {
            lines.push(line.to_vec())
        })
        .unwrap();
        capabilities(DISCOVERY_REPORT, b"capabilities", |line| {
            lines.push(line.to_vec())
        })
        .unwrap();
        capabilities(DISCOVERY_REPORT, b"capabilities for echo", |line| {
            lines.push(line.to_vec())
        })
        .unwrap();
        capability_detail(DISCOVERY_REPORT, b"capability cap:log.sink", |line| {
            lines.push(line.to_vec())
        })
        .unwrap();
        states(DISCOVERY_REPORT, |line| lines.push(line.to_vec())).unwrap();
        state_detail(DISCOVERY_REPORT, b"state state:counter", |line| {
            lines.push(line.to_vec())
        })
        .unwrap();
        devices(DISCOVERY_REPORT, |line| lines.push(line.to_vec())).unwrap();
        device_detail(DISCOVERY_REPORT, b"device device:virtio-blk0", |line| {
            lines.push(line.to_vec())
        })
        .unwrap();

        assert!(
            lines
                .iter()
                .any(|line| line == b"overview generation=gen:a")
        );
        assert!(
            lines.iter().any(
                |line| line == b"svc:echo-server process=echo state=running restart=on-failure"
            )
        );
        assert!(
            lines
                .iter()
                .any(|line| line == b"requires cap:log.sink rights=send provider=svc:logd")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == b"cap:log.sink provider=svc:logd rights=send consumers=1")
        );
        assert!(lines.iter().any(|line| line
            == b"state:counter owner=svc:echo-server schema=counter.v1 storage=vertexdisk-v1"));
        assert!(
            lines.iter().any(|line| line
                == b"device:virtio-blk0 object_kind=virtio-device label=device:virtio-blk0")
        );
    }

    #[test]
    fn verify_system_accepts_fully_bound_report() {
        let answer = verify_system(VERIFY_OK_REPORT).expect("bound report should verify");
        assert_eq!(answer.generation, b"gen:a");
        assert_eq!(answer.policy_hash, b"hash:policy");
        assert_eq!(answer.graph_hash, b"hash:graph");
        assert_eq!(answer.packages, 1);
    }

    #[test]
    fn verify_system_rejects_missing_required_live_capability() {
        let report = std::str::from_utf8(VERIFY_OK_REPORT)
            .unwrap()
            .replace("graph_target=log-sink", "graph_target=other-log");
        let error = verify_system(report.as_bytes())
            .expect_err("wrong live graph_target must not satisfy active requirement");
        assert_eq!(
            error.message,
            b"operator verifier rejected: required live capability missing"
        );
    }

    #[test]
    fn verify_system_rejects_dead_service_process() {
        let report = std::str::from_utf8(VERIFY_OK_REPORT)
            .unwrap()
            .replace("state=running", "state=exited");
        let error = verify_system(report.as_bytes())
            .expect_err("exited process must not verify as active service");
        assert_eq!(
            error.message,
            b"operator verifier rejected: no active graph services"
        );
    }

    #[test]
    fn verify_system_rejects_reaped_service_process() {
        let report = std::str::from_utf8(VERIFY_OK_REPORT)
            .unwrap()
            .replace("context_reaped=no", "context_reaped=yes");
        let error = verify_system(report.as_bytes())
            .expect_err("reaped process context must not verify as active service");
        assert_eq!(
            error.message,
            b"operator verifier rejected: service process context reaped"
        );
    }

    #[test]
    fn verify_system_rejects_graph_hash_mismatch() {
        let report = std::str::from_utf8(VERIFY_OK_REPORT).unwrap().replace(
            "graph-store v=1 generation=gen:a hash=hash:graph checksum=1",
            "graph-store v=1 generation=gen:a hash=hash:wrong checksum=1",
        );
        let error =
            verify_system(report.as_bytes()).expect_err("graph-store hash must bind to generation");
        assert_eq!(
            error.message,
            b"operator verifier rejected: graph store hash mismatch"
        );
    }

    #[test]
    fn verify_system_rejects_package_count_mismatch() {
        let report = std::str::from_utf8(VERIFY_OK_REPORT).unwrap().replace(
            "packages=1 package_facts=graph-v1",
            "packages=2 package_facts=graph-v1",
        );
        let error =
            verify_system(report.as_bytes()).expect_err("package count must match package rows");
        assert_eq!(
            error.message,
            b"operator verifier rejected: package count mismatch"
        );
    }
}
