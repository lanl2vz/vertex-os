use std::env;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct HostedCap {
    id: String,
    kind: String,
    rights: Vec<String>,
    endpoint: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("echo-server error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let service_id = env::var("VERTEX_SERVICE_ID").unwrap_or_else(|_| "svc:echo-server".to_owned());
    let grants = env::var("VERTEX_GRANTED_CAPS").unwrap_or_default();
    let provided = env::var("VERTEX_PROVIDED_CAPS").unwrap_or_default();
    let caps = parse_caps(&grants);

    println!("{service_id}: echo-server started");
    println!("{service_id}: granted capabilities [{grants}]");
    println!("{service_id}: provided capabilities [{provided}]");

    if let Some(log_sink) = find_cap(&caps, "cap:log.sink", "send") {
        println!("{service_id}: can send to cap:log.sink");
        send_log_message(&service_id, log_sink)?;
    } else {
        println!("{service_id}: cannot send to cap:log.sink");
        if env::var("VERTEX_REQUIRE_LOG_SINK").as_deref() == Ok("1") {
            return Err("missing required capability cap:log.sink".to_owned());
        }
    }

    if find_cap(&caps, "cap:net.udp.9000", "listen").is_some() {
        println!("{service_id}: can listen on cap:net.udp.9000");
    } else {
        println!("{service_id}: cannot listen on cap:net.udp.9000");
    }

    Ok(())
}

fn send_log_message(service_id: &str, capability: &HostedCap) -> Result<(), String> {
    if capability.kind != "ipc-endpoint" {
        return Err(format!(
            "cap:log.sink should be ipc-endpoint, found {}",
            capability.kind
        ));
    }

    let Some(path) = capability.endpoint.strip_prefix("unix:") else {
        return Err(format!(
            "cap:log.sink endpoint must be unix:<path>, found {}",
            capability.endpoint
        ));
    };

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match UnixStream::connect(path) {
            Ok(mut stream) => {
                writeln!(stream, "hello from {service_id} via cap:log.sink")
                    .map_err(|source| format!("failed to send log message: {source}"))?;
                println!("{service_id}: sent log message via {path}");
                return Ok(());
            }
            Err(source) if Instant::now() < deadline => {
                let _ = source;
                thread::sleep(Duration::from_millis(25));
            }
            Err(source) => {
                return Err(format!("failed to connect to log socket {path}: {source}"));
            }
        }
    }
}

fn find_cap<'a>(caps: &'a [HostedCap], id: &str, right: &str) -> Option<&'a HostedCap> {
    caps.iter().find(|capability| {
        capability.id == id && capability.rights.iter().any(|item| item == right)
    })
}

fn parse_caps(input: &str) -> Vec<HostedCap> {
    input
        .split(';')
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            let mut parts = entry.splitn(4, '|');
            let id = parts.next()?.to_owned();
            let kind = parts.next()?.to_owned();
            let rights = parts
                .next()?
                .split(',')
                .filter(|right| !right.is_empty())
                .map(str::to_owned)
                .collect();
            let endpoint = parts.next()?.to_owned();

            Some(HostedCap {
                id,
                kind,
                rights,
                endpoint,
            })
        })
        .collect()
}
