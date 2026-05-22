use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};
use vertex_ir::{Capability, GenerationManifest, Service, load_manifest, validate_manifest};

#[derive(Debug, Clone)]
struct Args {
    dry_run: bool,
    run_once: bool,
    manifest_path: String,
}

#[derive(Debug, Clone)]
struct HostedCapability {
    id: String,
    kind: String,
    rights: Vec<String>,
    endpoint: String,
}

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

    let args = parse_args(args)?;
    let manifest = load_manifest(&args.manifest_path).map_err(|error| error.to_string())?;
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

    activate(&manifest, &args)
}

fn activate(manifest: &GenerationManifest, args: &Args) -> Result<(), String> {
    println!("activating generation {}", manifest.generation.id);
    println!("root service {}", manifest.activation.root_service);

    let runtime_dir = create_runtime_dir(manifest)?;
    let capabilities = build_hosted_capabilities(manifest, &runtime_dir);
    println!("runtime dir {}", runtime_dir.display());

    let mut children = Vec::new();
    for service_id in &manifest.activation.start_order {
        let service = manifest
            .service(service_id)
            .ok_or_else(|| format!("activation references unknown service {service_id}"))?;

        if service.id == manifest.activation.root_service {
            println!(
                "root service {} is represented by this supervisor process",
                service.id
            );
            continue;
        }

        let executable = resolve_service_executable(manifest, service)?;
        let grants = granted_capabilities(service, &capabilities)?;
        let provides = provided_capabilities(service, &capabilities)?;

        if args.dry_run {
            println!(
                "would start {} as {} with grants [{}] provides [{}]",
                service.id,
                executable.display(),
                grants,
                provides
            );
            continue;
        }

        println!("starting {} as {}", service.id, executable.display());
        let mut command = Command::new(&executable);
        command.args(&service.args);
        command.envs(&service.env);
        command.env("VERTEX_SERVICE_ID", &service.id);
        command.env("VERTEX_GRANTED_CAPS", &grants);
        command.env("VERTEX_PROVIDED_CAPS", &provides);
        command.env("VERTEX_RUNTIME_DIR", &runtime_dir);
        if args.run_once {
            command.env("VERTEX_DEMO_RUN_ONCE", "1");
        }

        let child = command
            .spawn()
            .map_err(|source| format!("failed to start {}: {source}", service.id))?;
        children.push((service.id.clone(), child));
    }

    if args.dry_run {
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

fn granted_capabilities(
    service: &Service,
    capabilities: &BTreeMap<String, HostedCapability>,
) -> Result<String, String> {
    let mut grants = Vec::new();

    for requirement in &service.requires {
        let Some(capability) = capabilities.get(&requirement.capability) else {
            return Err(format!(
                "service {} requires unknown hosted capability {}",
                service.id, requirement.capability
            ));
        };
        grants.push(encode_capability(
            capability,
            Some(requirement.rights.as_slice()),
        ));
    }

    Ok(grants.join(";"))
}

fn provided_capabilities(
    service: &Service,
    capabilities: &BTreeMap<String, HostedCapability>,
) -> Result<String, String> {
    let mut provided = Vec::new();

    for capability_id in &service.provides {
        let Some(capability) = capabilities.get(capability_id) else {
            return Err(format!(
                "service {} provides unknown hosted capability {}",
                service.id, capability_id
            ));
        };
        provided.push(encode_capability(capability, None));
    }

    Ok(provided.join(";"))
}

fn encode_capability(capability: &HostedCapability, rights_override: Option<&[String]>) -> String {
    let rights = rights_override.unwrap_or(&capability.rights).join(",");

    format!(
        "{}|{}|{}|{}",
        capability.id, capability.kind, rights, capability.endpoint
    )
}

fn build_hosted_capabilities(
    manifest: &GenerationManifest,
    runtime_dir: &Path,
) -> BTreeMap<String, HostedCapability> {
    manifest
        .capabilities
        .iter()
        .map(|capability| {
            (
                capability.id.clone(),
                HostedCapability {
                    id: capability.id.clone(),
                    kind: capability.kind.clone(),
                    rights: capability.rights.clone(),
                    endpoint: hosted_endpoint(capability, runtime_dir),
                },
            )
        })
        .collect()
}

fn hosted_endpoint(capability: &Capability, runtime_dir: &Path) -> String {
    match capability.kind.as_str() {
        "ipc-endpoint" | "log-sink" => {
            let file_name = format!("{}.sock", sanitize_id(&capability.id));
            format!("unix:{}", runtime_dir.join(file_name).display())
        }
        "network-port" => {
            let port = capability
                .properties
                .get("port")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            format!("tcp:127.0.0.1:{port}")
        }
        "clock" => {
            let name = capability
                .properties
                .get("clock")
                .and_then(|value| value.as_str())
                .unwrap_or("host");
            format!("host-clock:{name}")
        }
        _ => format!("opaque:{}", capability.id),
    }
}

fn create_runtime_dir(manifest: &GenerationManifest) -> Result<PathBuf, String> {
    let mut path = runtime_root();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|source| format!("host clock moved backwards: {source}"))?
        .as_millis();
    path.push(format!(
        "vertex-{}-{now}",
        sanitize_id(&manifest.generation.id)
    ));
    fs::create_dir_all(&path)
        .map_err(|source| format!("failed to create runtime dir {}: {source}", path.display()))?;
    Ok(path)
}

fn runtime_root() -> PathBuf {
    if let Some(path) = env::var_os("VERTEX_RUNTIME_ROOT") {
        return PathBuf::from(path);
    }

    let private_tmp = PathBuf::from("/private/tmp");
    if private_tmp.is_dir() {
        private_tmp
    } else {
        env::temp_dir()
    }
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
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

fn parse_args(args: Vec<String>) -> Result<Args, String> {
    let mut dry_run = false;
    let mut run_once = false;
    let mut manifest_path = None;

    for arg in args {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--run-once" => run_once = true,
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}"));
            }
            _ => {
                if manifest_path.replace(arg).is_some() {
                    return Err(
                        "usage: vertex-supervisor [--dry-run] [--run-once] <manifest>".to_owned(),
                    );
                }
            }
        }
    }

    Ok(Args {
        dry_run,
        run_once,
        manifest_path: manifest_path.ok_or_else(|| {
            "usage: vertex-supervisor [--dry-run] [--run-once] <manifest>".to_owned()
        })?,
    })
}

fn print_usage() {
    println!(
        "vertex-supervisor\n\n\
         usage:\n\
           vertex-supervisor --dry-run <manifest>\n\
           vertex-supervisor --run-once <manifest>\n\
           vertex-supervisor <manifest>"
    );
}
