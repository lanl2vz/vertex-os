mod capability;

use capability::{
    HostedCapability, build_hosted_capabilities, capability_grants_for_service,
    capability_grants_json, encode_capability_grants, encode_state_grants,
    provided_capability_grants_for_service, sanitize_id, state_grants_for_service,
    state_grants_json,
};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use vertex_ir::{GenerationManifest, HealthCheck, Service, load_manifest, validate_manifest};

const READINESS_TIMEOUT: Duration = Duration::from_secs(2);
const READINESS_POLL: Duration = Duration::from_millis(25);

#[derive(Debug, Clone)]
struct Args {
    dry_run: bool,
    run_once: bool,
    state_root: PathBuf,
    manifest_path: String,
}

struct ChildInfo {
    service_id: String,
    child: Child,
    exited: bool,
}

struct ActivationRuntime<'a> {
    manifest: &'a GenerationManifest,
    args: &'a Args,
    state_root: &'a Path,
    runtime_dir: &'a Path,
    capabilities: &'a BTreeMap<String, HostedCapability>,
    children: Vec<ChildInfo>,
    started: BTreeSet<String>,
    ready: BTreeSet<String>,
    starting: BTreeSet<String>,
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

    let state_root = canonicalize_state_root(&args.state_root)?;
    let runtime_dir = create_runtime_dir(manifest)?;
    let capabilities = build_hosted_capabilities(manifest, &runtime_dir);
    println!("runtime dir {}", runtime_dir.display());
    println!("state root {}", state_root.display());

    if !args.dry_run {
        append_runtime_event(
            &state_root,
            serde_json::json!({
                "event": "activationStart",
                "generationId": manifest.generation.id,
                "rootService": manifest.activation.root_service,
                "utcMs": unix_millis()?
            }),
        )?;
    }

    let result = activate_services(manifest, args, &state_root, &runtime_dir, &capabilities);
    if !args.dry_run {
        match &result {
            Ok(()) => append_runtime_event(
                &state_root,
                serde_json::json!({
                    "event": "activationSuccess",
                    "generationId": manifest.generation.id,
                    "utcMs": unix_millis()?
                }),
            )?,
            Err(error) => append_runtime_event(
                &state_root,
                serde_json::json!({
                    "event": "activationFailure",
                    "generationId": manifest.generation.id,
                    "error": error,
                    "utcMs": unix_millis()?
                }),
            )?,
        }
    }

    result
}

fn activate_services(
    manifest: &GenerationManifest,
    args: &Args,
    state_root: &Path,
    runtime_dir: &Path,
    capabilities: &BTreeMap<String, HostedCapability>,
) -> Result<(), String> {
    let mut runtime = ActivationRuntime::new(manifest, args, state_root, runtime_dir, capabilities);
    if let Err(error) = runtime.activate_start_order() {
        if !args.dry_run {
            runtime.terminate_running()?;
        }
        return Err(error);
    }

    if args.dry_run {
        println!("dry run complete");
        return Ok(());
    }

    runtime.wait_for_children()
}

impl<'a> ActivationRuntime<'a> {
    fn new(
        manifest: &'a GenerationManifest,
        args: &'a Args,
        state_root: &'a Path,
        runtime_dir: &'a Path,
        capabilities: &'a BTreeMap<String, HostedCapability>,
    ) -> Self {
        Self {
            manifest,
            args,
            state_root,
            runtime_dir,
            capabilities,
            children: Vec::new(),
            started: BTreeSet::new(),
            ready: BTreeSet::new(),
            starting: BTreeSet::new(),
        }
    }

    fn activate_start_order(&mut self) -> Result<(), String> {
        for service_id in &self.manifest.activation.start_order {
            self.start_service_tree(service_id)?;
        }
        Ok(())
    }

    fn start_service_tree(&mut self, service_id: &str) -> Result<(), String> {
        if self.started.contains(service_id) {
            return Ok(());
        }
        if !self.starting.insert(service_id.to_owned()) {
            return Err(format!("service dependency cycle includes {service_id}"));
        }

        let result = self.start_service_tree_inner(service_id);
        self.starting.remove(service_id);
        result
    }

    fn start_service_tree_inner(&mut self, service_id: &str) -> Result<(), String> {
        let service = self
            .manifest
            .service(service_id)
            .ok_or_else(|| format!("activation references unknown service {service_id}"))?;
        let dependencies = self.service_dependencies(service)?;

        for dependency in dependencies {
            self.start_service_tree(&dependency)?;
            self.ensure_service_ready(&dependency)?;
        }

        self.start_service(service_id)
    }

    fn service_dependencies(&self, service: &Service) -> Result<Vec<String>, String> {
        let mut dependencies = Vec::new();
        let mut seen = BTreeSet::new();
        for dependency in &service.lifecycle.start_after {
            push_dependency(&mut dependencies, &mut seen, &service.id, dependency);
        }
        for requirement in &service.requires {
            let capability = self
                .manifest
                .capability(&requirement.capability)
                .ok_or_else(|| {
                    format!(
                        "service {} requires unknown capability {}",
                        service.id, requirement.capability
                    )
                })?;
            if self.manifest.service(&capability.provider).is_some() {
                push_dependency(
                    &mut dependencies,
                    &mut seen,
                    &service.id,
                    &capability.provider,
                );
            }
        }
        Ok(dependencies)
    }

    fn start_service(&mut self, service_id: &str) -> Result<(), String> {
        if self.started.contains(service_id) {
            return Ok(());
        }
        let service = self
            .manifest
            .service(service_id)
            .ok_or_else(|| format!("activation references unknown service {service_id}"))?;

        if service.id == self.manifest.activation.root_service {
            println!(
                "root service {} is represented by this supervisor process",
                service.id
            );
            self.started.insert(service.id.clone());
            self.ready.insert(service.id.clone());
            if !self.args.dry_run {
                append_runtime_event(
                    self.state_root,
                    serde_json::json!({
                        "event": "serviceSkippedRoot",
                        "serviceId": service.id,
                        "health": service_health_json(service.health.as_ref()),
                        "utcMs": unix_millis()?
                    }),
                )?;
            }
            return Ok(());
        }

        let executable = resolve_service_executable(self.manifest, service)?;
        let grants = capability_grants_for_service(service, self.capabilities)?;
        let provides = provided_capability_grants_for_service(service, self.capabilities)?;
        let state_grants = state_grants_for_service(service, self.manifest, self.state_root)?;
        let grants_env = encode_capability_grants(&grants);
        let provides_env = encode_capability_grants(&provides);
        let state_env = encode_state_grants(&state_grants);

        if self.args.dry_run {
            println!(
                "would start {} as {} with grants [{}] provides [{}] state [{}]",
                service.id,
                executable.display(),
                grants_env,
                provides_env,
                state_env
            );
            self.started.insert(service.id.clone());
            return Ok(());
        }

        println!("starting {} as {}", service.id, executable.display());
        let mut command = Command::new(&executable);
        command.args(&service.args);
        command.envs(&service.env);
        command.env("VERTEX_SERVICE_ID", &service.id);
        command.env("VERTEX_GRANTED_CAPS", &grants_env);
        command.env("VERTEX_PROVIDED_CAPS", &provides_env);
        command.env("VERTEX_STATE_VOLUMES", &state_env);
        command.env("VERTEX_RUNTIME_DIR", self.runtime_dir);
        if self.args.run_once {
            command.env("VERTEX_DEMO_RUN_ONCE", "1");
        }

        let child = command
            .spawn()
            .map_err(|source| format!("failed to start {}: {source}", service.id))?;
        append_runtime_event(
            self.state_root,
            serde_json::json!({
                "event": "serviceStart",
                "serviceId": service.id,
                "executable": executable,
                "health": service_health_json(service.health.as_ref()),
                "grantedCapabilities": capability_grants_json(&grants),
                "providedCapabilities": capability_grants_json(&provides),
                "stateVolumes": state_grants_json(&state_grants),
                "utcMs": unix_millis()?
            }),
        )?;
        for grant in &grants {
            append_runtime_event(
                self.state_root,
                serde_json::json!({
                    "event": "capabilityGrant",
                    "serviceId": service.id,
                    "capabilityId": grant.id,
                    "kind": grant.kind,
                    "rights": grant.rights,
                    "provider": grant.provider,
                    "consumer": grant.consumer,
                    "endpoint": grant.endpoint,
                    "utcMs": unix_millis()?
                }),
            )?;
        }
        for grant in &state_grants {
            append_runtime_event(
                self.state_root,
                serde_json::json!({
                    "event": "stateVolumeGrant",
                    "serviceId": service.id,
                    "stateId": grant.id,
                    "kind": grant.kind,
                    "owner": grant.owner,
                    "consumer": grant.consumer,
                    "rights": grant.rights,
                    "path": grant.path,
                    "endpoint": grant.endpoint,
                    "utcMs": unix_millis()?
                }),
            )?;
        }

        self.children.push(ChildInfo {
            service_id: service.id.clone(),
            child,
            exited: false,
        });
        self.started.insert(service.id.clone());
        Ok(())
    }

    fn ensure_service_ready(&mut self, service_id: &str) -> Result<(), String> {
        if self.ready.contains(service_id) {
            return Ok(());
        }
        if self.args.dry_run {
            self.ready.insert(service_id.to_owned());
            return Ok(());
        }

        let health = self
            .manifest
            .service(service_id)
            .ok_or_else(|| format!("readiness references unknown service {service_id}"))?
            .health
            .clone();
        let result = self.check_service_health(service_id, health.as_ref());
        match result {
            Ok(details) => {
                self.ready.insert(service_id.to_owned());
                append_runtime_event(
                    self.state_root,
                    serde_json::json!({
                        "event": "serviceReady",
                        "serviceId": service_id,
                        "health": service_health_json(health.as_ref()),
                        "details": details,
                        "utcMs": unix_millis()?
                    }),
                )
            }
            Err(error) => {
                append_runtime_event(
                    self.state_root,
                    serde_json::json!({
                        "event": "serviceReadinessFailure",
                        "serviceId": service_id,
                        "health": service_health_json(health.as_ref()),
                        "error": error,
                        "utcMs": unix_millis()?
                    }),
                )?;
                Err(error)
            }
        }
    }

    fn check_service_health(
        &mut self,
        service_id: &str,
        health: Option<&HealthCheck>,
    ) -> Result<serde_json::Value, String> {
        let kind = health.map(|health| health.kind.as_str()).unwrap_or("none");
        match kind {
            "none" => Ok(serde_json::json!({ "kind": "none" })),
            "process-alive" => {
                self.ensure_child_alive(service_id)?;
                Ok(serde_json::json!({ "kind": "process-alive" }))
            }
            "ipc-ping" => {
                let target = health
                    .and_then(|health| health.target.as_deref())
                    .ok_or_else(|| format!("ipc-ping health for {service_id} requires target"))?;
                let path = self
                    .capabilities
                    .get(target)
                    .and_then(|capability| capability.endpoint.unix_socket_path())
                    .map(Path::to_path_buf)
                    .ok_or_else(|| {
                        format!("ipc-ping target {target} is not a hosted Unix socket capability")
                    })?;
                self.wait_for_unix_socket(service_id, target, &path)
            }
            "tcp-listen" => {
                let target = health
                    .and_then(|health| health.target.as_deref())
                    .ok_or_else(|| format!("tcp-listen health for {service_id} requires target"))?;
                let (host, port) = self
                    .capabilities
                    .get(target)
                    .and_then(|capability| capability.endpoint.tcp_target())
                    .map(|(host, port)| (host.to_owned(), port))
                    .ok_or_else(|| {
                        format!("tcp-listen target {target} is not a hosted TCP capability")
                    })?;
                self.wait_for_tcp_listener(service_id, target, &host, port)
            }
            other => Err(format!(
                "service {service_id} has unsupported hosted health kind {other}"
            )),
        }
    }

    fn wait_for_unix_socket(
        &mut self,
        service_id: &str,
        target: &str,
        path: &Path,
    ) -> Result<serde_json::Value, String> {
        let deadline = Instant::now() + READINESS_TIMEOUT;
        loop {
            self.ensure_child_alive(service_id)?;
            if path.exists() && UnixStream::connect(path).is_ok() {
                return Ok(serde_json::json!({
                    "kind": "ipc-ping",
                    "target": target,
                    "path": path
                }));
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "service {service_id} did not become ready on Unix socket {}",
                    path.display()
                ));
            }
            thread::sleep(READINESS_POLL);
        }
    }

    fn wait_for_tcp_listener(
        &mut self,
        service_id: &str,
        target: &str,
        host: &str,
        port: u16,
    ) -> Result<serde_json::Value, String> {
        let deadline = Instant::now() + READINESS_TIMEOUT;
        loop {
            self.ensure_child_alive(service_id)?;
            if TcpStream::connect((host, port)).is_ok() {
                return Ok(serde_json::json!({
                    "kind": "tcp-listen",
                    "target": target,
                    "host": host,
                    "port": port
                }));
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "service {service_id} did not become ready on TCP {host}:{port}"
                ));
            }
            thread::sleep(READINESS_POLL);
        }
    }

    fn ensure_child_alive(&mut self, service_id: &str) -> Result<(), String> {
        if let Some(status) = self.poll_child_by_service(service_id)? {
            return Err(format!(
                "service {service_id} exited before readiness with {status}"
            ));
        }
        Ok(())
    }

    fn poll_child_by_service(&mut self, service_id: &str) -> Result<Option<ExitStatus>, String> {
        let Some(index) = self
            .children
            .iter()
            .position(|child| child.service_id == service_id)
        else {
            return Err(format!("service {service_id} has no hosted child process"));
        };
        self.poll_child_by_index(index)
    }

    fn poll_child_by_index(&mut self, index: usize) -> Result<Option<ExitStatus>, String> {
        if self.children[index].exited {
            return Ok(None);
        }

        let service_id = self.children[index].service_id.clone();
        let status = self.children[index]
            .child
            .try_wait()
            .map_err(|source| format!("failed to poll {service_id}: {source}"))?;
        if let Some(status) = status {
            self.children[index].exited = true;
            self.record_service_exit(&service_id, &status)?;
            Ok(Some(status))
        } else {
            Ok(None)
        }
    }

    fn wait_for_children(&mut self) -> Result<(), String> {
        let mut failed = Vec::new();

        while self.children.iter().any(|child| !child.exited) {
            let mut saw_exit = false;
            for index in 0..self.children.len() {
                if self.children[index].exited {
                    continue;
                }
                let service_id = self.children[index].service_id.clone();
                if let Some(status) = self.poll_child_by_index(index)? {
                    saw_exit = true;
                    if !status.success() {
                        failed.push(format!("{service_id} exited with {status}"));
                    }
                }
            }

            if !failed.is_empty() {
                self.terminate_running()?;
                return Err(failed.join("; "));
            }
            if !saw_exit {
                thread::sleep(READINESS_POLL);
            }
        }

        Ok(())
    }

    fn terminate_running(&mut self) -> Result<(), String> {
        for index in 0..self.children.len() {
            if self.children[index].exited {
                continue;
            }

            let service_id = self.children[index].service_id.clone();
            let _ = self.children[index].child.kill();
            let status = self.children[index]
                .child
                .wait()
                .map_err(|source| format!("failed to terminate {service_id}: {source}"))?;
            self.children[index].exited = true;
            self.record_service_exit(&service_id, &status)?;
        }
        Ok(())
    }

    fn record_service_exit(&self, service_id: &str, status: &ExitStatus) -> Result<(), String> {
        append_runtime_event(
            self.state_root,
            serde_json::json!({
                "event": "serviceExit",
                "serviceId": service_id,
                "success": status.success(),
                "status": status.to_string(),
                "utcMs": unix_millis()?
            }),
        )
    }
}

fn push_dependency(
    dependencies: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
    service_id: &str,
    dependency: &str,
) {
    if dependency != service_id && seen.insert(dependency.to_owned()) {
        dependencies.push(dependency.to_owned());
    }
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

fn canonicalize_state_root(path: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(path)
        .map_err(|source| format!("failed to create state root {}: {source}", path.display()))?;
    path.canonicalize().map_err(|source| {
        format!(
            "failed to canonicalize state root {}: {source}",
            path.display()
        )
    })
}

fn service_health_json(health: Option<&HealthCheck>) -> serde_json::Value {
    match health {
        Some(health) => serde_json::json!({
            "kind": health.kind,
            "target": health.target
        }),
        None => serde_json::Value::Null,
    }
}

fn append_runtime_event(state_root: &Path, value: serde_json::Value) -> Result<(), String> {
    use std::io::Write;

    fs::create_dir_all(state_root).map_err(|source| {
        format!(
            "failed to create state root {}: {source}",
            state_root.display()
        )
    })?;
    let path = state_root.join("runtime-events.jsonl");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|source| format!("failed to open {}: {source}", path.display()))?;
    let line = serde_json::to_string(&value).map_err(|source| source.to_string())?;
    writeln!(file, "{line}")
        .map_err(|source| format!("failed to append {}: {source}", path.display()))
}

fn unix_millis() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|source| format!("host clock moved backwards: {source}"))
}

fn parse_args(args: Vec<String>) -> Result<Args, String> {
    let mut dry_run = false;
    let mut run_once = false;
    let mut state_root = PathBuf::from(".vertex");
    let mut manifest_path = None;
    let mut idx = 0;

    while idx < args.len() {
        match args[idx].as_str() {
            "--dry-run" => dry_run = true,
            "--run-once" => run_once = true,
            "--state-root" => {
                idx += 1;
                let Some(path) = args.get(idx) else {
                    return Err("--state-root requires a directory".to_owned());
                };
                state_root = PathBuf::from(path);
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}"));
            }
            _ => {
                if manifest_path.replace(args[idx].clone()).is_some() {
                    return Err(
                        "usage: vertex-supervisor [--state-root <dir>] [--dry-run] [--run-once] <manifest>"
                            .to_owned(),
                    );
                }
            }
        }
        idx += 1;
    }

    Ok(Args {
        dry_run,
        run_once,
        state_root,
        manifest_path: manifest_path.ok_or_else(|| {
            "usage: vertex-supervisor [--state-root <dir>] [--dry-run] [--run-once] <manifest>"
                .to_owned()
        })?,
    })
}

fn print_usage() {
    println!(
        "vertex-supervisor\n\n\
         usage:\n\
           vertex-supervisor --state-root <dir> --dry-run <manifest>\n\
           vertex-supervisor --state-root <dir> --run-once <manifest>\n\
           vertex-supervisor <manifest>"
    );
}
