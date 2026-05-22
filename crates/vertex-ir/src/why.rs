use crate::model::GenerationManifest;
use std::collections::BTreeSet;

pub fn explain_authority(
    manifest: &GenerationManifest,
    service_id: &str,
    capability_id: &str,
) -> String {
    let Some(service) = manifest.service(service_id) else {
        return format!("{service_id} does not exist in this generation.");
    };

    let Some(capability) = manifest.capability(capability_id) else {
        return format!(
            "{service_id} cannot use {capability_id} because the capability does not exist."
        );
    };

    let Some(requirement) = service.required_capability(capability_id) else {
        return format!(
            "{service_id} cannot use {capability_id} because it does not declare a matching requires edge.\n\n\
             generation policy defaultAuthority={} grants no ambient authority.",
            manifest.policies.default_authority
        );
    };

    let granted: BTreeSet<&str> = capability.rights.iter().map(String::as_str).collect();
    let missing: Vec<&str> = requirement
        .rights
        .iter()
        .map(String::as_str)
        .filter(|right| !granted.contains(right))
        .collect();

    if !missing.is_empty() {
        return format!(
            "{service_id} cannot use {capability_id} because it requests missing right(s): {}.\n\
             {capability_id} grants [{}].",
            missing.join(", "),
            capability.rights.join(", ")
        );
    }

    format!(
        "{service_id} can use {capability_id} because:\n\n\
         1. {capability_id} exists and is kind {}.\n\
         2. {capability_id} is provided by {}.\n\
         3. {service_id} declares a requirement for {capability_id} with right(s) {}.\n\
         4. {capability_id} grants right(s) {}.\n\
         5. generation policy defaultAuthority={} does not grant anything else.",
        capability.kind,
        capability.provider,
        requirement.rights.join(", "),
        capability.rights.join(", "),
        manifest.policies.default_authority
    )
}
