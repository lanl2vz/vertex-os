use crate::model::GenerationManifest;

pub fn render_graph_text(manifest: &GenerationManifest) -> String {
    let mut out = String::new();

    out.push_str(&format!("generation {}\n", manifest.generation.id));
    out.push_str(&format!("  kernel: {}\n", manifest.kernel.id));
    out.push_str(&format!("  init: {}\n", manifest.init.id));
    out.push_str(&format!(
        "  activation root: {}\n\n",
        manifest.activation.root_service
    ));

    out.push_str("store objects\n");
    for store in &manifest.store {
        out.push_str(&format!(
            "  {} ({}) -> {}\n",
            store.id, store.kind, store.path
        ));
    }

    out.push_str("\nexecutables\n");
    for executable in &manifest.executables {
        out.push_str(&format!(
            "  {} -> {}:{}\n",
            executable.id, executable.store_object, executable.entrypoint
        ));
    }

    out.push_str("\ncapabilities\n");
    for capability in &manifest.capabilities {
        out.push_str(&format!(
            "  {} ({}) provider={} rights=[{}]\n",
            capability.id,
            capability.kind,
            capability.provider,
            capability.rights.join(", ")
        ));
    }

    out.push_str("\nservices\n");
    for service in &manifest.services {
        out.push_str(&format!(
            "  {} -> executable {}\n",
            service.id, service.executable
        ));
        if !service.lifecycle.start_after.is_empty() {
            out.push_str(&format!(
                "    starts_after: {}\n",
                service.lifecycle.start_after.join(", ")
            ));
        }
        for requirement in &service.requires {
            out.push_str(&format!(
                "    requires: {} [{}]\n",
                requirement.capability,
                requirement.rights.join(", ")
            ));
        }
        for provided in &service.provides {
            out.push_str(&format!("    provides: {provided}\n"));
        }
    }

    out.push_str("\nactivation start order\n");
    for (idx, service) in manifest.activation.start_order.iter().enumerate() {
        out.push_str(&format!("  {}. {service}\n", idx + 1));
    }

    out
}
