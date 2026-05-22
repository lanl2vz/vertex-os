use crate::model::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    pub errors: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    fn error(&mut self, message: impl Into<String>) {
        self.errors.push(Diagnostic {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
        });
    }

    fn warning(&mut self, message: impl Into<String>) {
        self.warnings.push(Diagnostic {
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
        });
    }
}

pub fn validate_manifest(manifest: &GenerationManifest) -> ValidationReport {
    let mut report = ValidationReport::default();
    let ids = collect_ids(manifest, &mut report);

    validate_schema_version(manifest, &mut report);
    validate_policy(manifest, &mut report);
    validate_kernel_and_init(manifest, &mut report);
    validate_executables(manifest, &mut report);
    validate_capabilities(manifest, &ids, &mut report);
    validate_services(manifest, &mut report);
    validate_state_volumes(manifest, &ids, &mut report);
    validate_devices(manifest, &mut report);
    validate_activation(manifest, &mut report);
    add_warnings(manifest, &mut report);

    report
}

fn validate_schema_version(manifest: &GenerationManifest, report: &mut ValidationReport) {
    if manifest.schema != "vertex.ir.v0" {
        report.error(format!(
            "schema must be vertex.ir.v0, found {}",
            manifest.schema
        ));
    }
}

fn validate_policy(manifest: &GenerationManifest, report: &mut ValidationReport) {
    let policies = &manifest.policies;

    if policies.default_authority != "deny" {
        report.error("policies.defaultAuthority must be deny");
    }
    if policies.capability_delegation != "explicit-only" {
        report.error("policies.capabilityDelegation must be explicit-only");
    }
    if policies.unknown_references != "reject" && policies.unknown_references != "warn" {
        report.error("policies.unknownReferences must be reject or warn");
    }
}

fn validate_kernel_and_init(manifest: &GenerationManifest, report: &mut ValidationReport) {
    if manifest
        .store_object(&manifest.kernel.store_object)
        .is_none()
    {
        report.error(format!(
            "kernel {} references unknown store object {}",
            manifest.kernel.id, manifest.kernel.store_object
        ));
    }

    if manifest.executable(&manifest.init.executable).is_none() {
        report.error(format!(
            "init {} references unknown executable {}",
            manifest.init.id, manifest.init.executable
        ));
    }

    if manifest.init.mode != "hosted-linux" && manifest.init.mode != "krust-native" {
        report.error(format!(
            "init {} has unsupported mode {}",
            manifest.init.id, manifest.init.mode
        ));
    }
}

fn validate_executables(manifest: &GenerationManifest, report: &mut ValidationReport) {
    for executable in &manifest.executables {
        if manifest.store_object(&executable.store_object).is_none() {
            report.error(format!(
                "executable {} references unknown store object {}",
                executable.id, executable.store_object
            ));
        }
    }
}

fn validate_capabilities(
    manifest: &GenerationManifest,
    ids: &BTreeSet<String>,
    report: &mut ValidationReport,
) {
    for capability in &manifest.capabilities {
        if capability.rights.is_empty() {
            report.error(format!("capability {} has no rights", capability.id));
        }

        if !ids.contains(&capability.provider) {
            report.error(format!(
                "capability {} has unknown provider {}",
                capability.id, capability.provider
            ));
        }
    }
}

fn validate_services(manifest: &GenerationManifest, report: &mut ValidationReport) {
    for service in &manifest.services {
        if manifest.executable(&service.executable).is_none() {
            report.error(format!(
                "service {} references unknown executable {}",
                service.id, service.executable
            ));
        }

        for provided in &service.provides {
            match manifest.capability(provided) {
                Some(capability) if capability.provider == service.id => {}
                Some(capability) => report.error(format!(
                    "service {} provides {}, but the capability provider is {}",
                    service.id, provided, capability.provider
                )),
                None => report.error(format!(
                    "service {} provides unknown capability {}",
                    service.id, provided
                )),
            }
        }

        for requirement in &service.requires {
            let Some(capability) = manifest.capability(&requirement.capability) else {
                report.error(format!(
                    "service {} requires unknown capability {}",
                    service.id, requirement.capability
                ));
                continue;
            };

            if requirement.rights.is_empty() {
                report.error(format!(
                    "service {} requires {} with no rights",
                    service.id, requirement.capability
                ));
            }

            let available: BTreeSet<&str> = capability.rights.iter().map(String::as_str).collect();
            for right in &requirement.rights {
                if !available.contains(right.as_str()) {
                    report.error(format!(
                        "service {} requires right {} on {}, but the capability only grants [{}]",
                        service.id,
                        right,
                        requirement.capability,
                        capability.rights.join(", ")
                    ));
                }
            }
        }

        for state in &service.state {
            if manifest.state_volume(state).is_none() {
                report.error(format!(
                    "service {} references unknown state volume {}",
                    service.id, state
                ));
            }
        }

        for secret in &service.secrets {
            if manifest.secret(secret).is_none() {
                report.error(format!(
                    "service {} references unknown secret {}",
                    service.id, secret
                ));
            }
        }

        for dependency in &service.lifecycle.start_after {
            if manifest.service(dependency).is_none() {
                report.error(format!(
                    "service {} startAfter references unknown service {}",
                    service.id, dependency
                ));
            }
        }

        for dependency in &service.lifecycle.stop_before {
            if manifest.service(dependency).is_none() {
                report.error(format!(
                    "service {} stopBefore references unknown service {}",
                    service.id, dependency
                ));
            }
        }
    }
}

fn validate_state_volumes(
    manifest: &GenerationManifest,
    ids: &BTreeSet<String>,
    report: &mut ValidationReport,
) {
    for state in &manifest.state_volumes {
        if !ids.contains(&state.owner) {
            report.error(format!(
                "state volume {} has unknown owner {}",
                state.id, state.owner
            ));
        }
    }
}

fn validate_devices(manifest: &GenerationManifest, report: &mut ValidationReport) {
    for device in &manifest.devices {
        if manifest.service(&device.driver).is_none() {
            report.error(format!(
                "device {} references unknown driver service {}",
                device.id, device.driver
            ));
        }
    }
}

fn validate_activation(manifest: &GenerationManifest, report: &mut ValidationReport) {
    if manifest
        .service(&manifest.activation.root_service)
        .is_none()
    {
        report.error(format!(
            "activation.rootService references unknown service {}",
            manifest.activation.root_service
        ));
    }

    for service in &manifest.activation.start_order {
        if manifest.service(service).is_none() {
            report.error(format!(
                "activation.startOrder references unknown service {}",
                service
            ));
        }
    }

    match manifest.activation.on_failure.as_str() {
        "stop-activation" | "continue" | "rollback" => {}
        other => report.error(format!(
            "activation.onFailure must be stop-activation, continue, or rollback; found {other}"
        )),
    }
}

fn add_warnings(manifest: &GenerationManifest, report: &mut ValidationReport) {
    warn_for_omitted_services(manifest, report);
    warn_for_unused_capabilities(manifest, report);
    warn_for_unreachable_store_objects(manifest, report);
    warn_for_services_without_health(manifest, report);
    warn_for_state_without_snapshot_policy(manifest, report);
}

fn warn_for_omitted_services(manifest: &GenerationManifest, report: &mut ValidationReport) {
    let start_order: BTreeSet<&str> = manifest
        .activation
        .start_order
        .iter()
        .map(String::as_str)
        .collect();

    for service in &manifest.services {
        if service.id != manifest.activation.root_service
            && !start_order.contains(service.id.as_str())
        {
            report.warning(format!(
                "service {} is not included in activation.startOrder",
                service.id
            ));
        }
    }
}

fn warn_for_unused_capabilities(manifest: &GenerationManifest, report: &mut ValidationReport) {
    let mut used = BTreeSet::new();

    for service in &manifest.services {
        for requirement in &service.requires {
            used.insert(requirement.capability.as_str());
        }
        if let Some(health) = &service.health
            && let Some(target) = &health.target
        {
            used.insert(target.as_str());
        }
    }

    for capability in &manifest.capabilities {
        if !used.contains(capability.id.as_str()) {
            report.warning(format!(
                "capability {} is declared but unused",
                capability.id
            ));
        }
    }
}

fn warn_for_unreachable_store_objects(
    manifest: &GenerationManifest,
    report: &mut ValidationReport,
) {
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::new();

    queue.push_back(manifest.kernel.store_object.as_str());
    for executable in &manifest.executables {
        queue.push_back(executable.store_object.as_str());
    }

    while let Some(id) = queue.pop_front() {
        if !reachable.insert(id.to_owned()) {
            continue;
        }
        if let Some(store) = manifest.store_object(id) {
            for reference in &store.references {
                queue.push_back(reference.as_str());
            }
        }
    }

    for store in &manifest.store {
        if !reachable.contains(store.id.as_str()) {
            report.warning(format!(
                "store object {} is unreachable from the kernel or any executable",
                store.id
            ));
        }
    }
}

fn warn_for_services_without_health(manifest: &GenerationManifest, report: &mut ValidationReport) {
    for service in &manifest.services {
        if service.health.is_none() {
            report.warning(format!("service {} has no health check", service.id));
        }
    }
}

fn warn_for_state_without_snapshot_policy(
    manifest: &GenerationManifest,
    report: &mut ValidationReport,
) {
    for state in &manifest.state_volumes {
        if is_empty_json_object(&state.snapshot_policy) {
            report.warning(format!(
                "state volume {} has an empty snapshot policy",
                state.id
            ));
        }
    }
}

fn is_empty_json_object(value: &Value) -> bool {
    matches!(value, Value::Object(map) if map.is_empty()) || value.is_null()
}

fn collect_ids(manifest: &GenerationManifest, report: &mut ValidationReport) -> BTreeSet<String> {
    let mut owners = BTreeMap::new();
    let mut ids = BTreeSet::new();

    record_id(
        &mut owners,
        &mut ids,
        &manifest.generation.id,
        "generation",
        report,
    );
    record_id(&mut owners, &mut ids, &manifest.kernel.id, "kernel", report);
    record_id(&mut owners, &mut ids, &manifest.init.id, "init", report);

    for store in &manifest.store {
        record_id(&mut owners, &mut ids, &store.id, "store object", report);
    }
    for executable in &manifest.executables {
        record_id(&mut owners, &mut ids, &executable.id, "executable", report);
    }
    for device in &manifest.devices {
        record_id(&mut owners, &mut ids, &device.id, "device", report);
    }
    for state in &manifest.state_volumes {
        record_id(&mut owners, &mut ids, &state.id, "state volume", report);
    }
    for secret in &manifest.secrets {
        record_id(&mut owners, &mut ids, &secret.id, "secret", report);
    }
    for capability in &manifest.capabilities {
        record_id(&mut owners, &mut ids, &capability.id, "capability", report);
    }
    for service in &manifest.services {
        record_id(&mut owners, &mut ids, &service.id, "service", report);
    }

    ids
}

fn record_id(
    owners: &mut BTreeMap<String, String>,
    ids: &mut BTreeSet<String>,
    id: &str,
    owner: &str,
    report: &mut ValidationReport,
) {
    if !is_valid_id(id) {
        report.error(format!("{owner} id {id} is not a valid typed Vertex ID"));
    }

    if let Some(previous_owner) = owners.insert(id.to_owned(), owner.to_owned()) {
        report.error(format!(
            "id {id} is duplicated by {previous_owner} and {owner}"
        ));
    }

    ids.insert(id.to_owned());
}

fn is_valid_id(id: &str) -> bool {
    let Some((prefix, suffix)) = id.split_once(':') else {
        return false;
    };

    if prefix.is_empty() || suffix.is_empty() {
        return false;
    }

    let mut prefix_chars = prefix.chars();
    let Some(first) = prefix_chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    if !prefix_chars
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return false;
    }

    suffix.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '@' | '/' | '+' | '*' | '-')
    })
}
