use std::env;
use std::path::PathBuf;
use std::process::{Child, Command, ExitCode};
use vertex_ir::{GenerationManifest, Service, load_manifest, validate_manifest};

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    if args.is_empty() || args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_usage();
        return Ok(());
    }

    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    let manifest_path = args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .ok_or_else(|| "usage: vertex-supervisor [--dry-run] <manifest>".to_owned())?;

    let manifest = load_manifest(manifest_path).map_err(|error| error.to_string())?;
    let report = validate_manifest(&manifest);

    for diagnostic in &report.errors {
        eprintln!("error: {}", diagnostic.message);
    }
    for diagnostic in &report.warnings {
        eprintln!("warning: {}", diagnostic.message);
    }

    if !report.is_valid() {
        return Err(format!(
            "manifest {} is invalid; activation aborted",
            manifest.generation.id
        ));
    }

    activate(&manifest, dry_run)
}

fn activate(manifest: &GenerationManifest, dry_run: bool) -> Result<(), String> {
    println!("activating generation {}", manifest.generation.id);
    println!("root service {}", manifest.activation.root_service);

    let mut children = Vec::new();
    for service_id in &manifest.activation.start_order {
        let service = manifest
            .service(service_id)
            .ok_or_else(|| format!("activation references unknown service {service_id}"))?;
        let executable = resolve_service_executable(manifest, service)?;
        let grants = granted_capabilities(service);

        if dry_run {
            println!(
                "would start {} as {} with grants [{}]",
                service.id,
                executable.display(),
                grants
            );
            continue;
        }

        println!("starting {} as {}", service.id, executable.display());
        let mut command = Command::new(&executable);
        command.args(&service.args);
        command.envs(&service.env);
        command.env("VERTEX_SERVICE_ID", &service.id);
        command.env("VERTEX_GRANTED_CAPS", &grants);

        let child = command
            .spawn()
            .map_err(|source| format!("failed to start {}: {source}", service.id))?;
        children.push((service.id.clone(), child));
    }

    if dry_run {
        println!("dry run complete");
        return Ok(());
    }

    wait_for_children(children)
}

fn resolve_service_executable(
    manifest: &GenerationManifest,
    service: &Service,
) -> Result<PathBuf, String> {
    let executable = manifest.executable(&service.executable).ok_or_else(|| {
        format!(
            "service {} has unknown executable {}",
            service.id, service.executable
        )
    })?;
    let store = manifest
        .store_object(&executable.store_object)
        .ok_or_else(|| {
            format!(
                "executable {} has unknown store object {}",
                executable.id, executable.store_object
            )
        })?;

    let mut path = PathBuf::from(&store.path);
    path.push(&executable.entrypoint);
    Ok(path)
}

fn granted_capabilities(service: &Service) -> String {
    service
        .requires
        .iter()
        .map(|requirement| {
            format!(
                "{}={}",
                requirement.capability,
                requirement.rights.join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn wait_for_children(mut children: Vec<(String, Child)>) -> Result<(), String> {
    let mut failed = Vec::new();

    for (service_id, child) in &mut children {
        let status = child
            .wait()
            .map_err(|source| format!("failed to wait for {service_id}: {source}"))?;
        if !status.success() {
            failed.push(format!("{service_id} exited with {status}"));
        }
    }

    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join("; "))
    }
}

fn print_usage() {
    println!(
        "vertex-supervisor\n\n\
         usage:\n\
           vertex-supervisor --dry-run <manifest>\n\
           vertex-supervisor <manifest>"
    );
}
