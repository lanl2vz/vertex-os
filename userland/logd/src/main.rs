use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::path::Path;

#[derive(Debug, Clone)]
struct HostedCap {
    id: String,
    kind: String,
    rights: Vec<String>,
    endpoint: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("logd error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let service_id = env::var("VERTEX_SERVICE_ID").unwrap_or_else(|_| "svc:logd".to_owned());
    let grants = env::var("VERTEX_GRANTED_CAPS").unwrap_or_default();
    let provided = env::var("VERTEX_PROVIDED_CAPS").unwrap_or_default();

    println!("{service_id}: logd started");
    println!("{service_id}: granted capabilities [{grants}]");
    println!("{service_id}: provided capabilities [{provided}]");

    let Some(log_sink) = parse_caps(&provided)
        .into_iter()
        .find(|capability| capability.id == "cap:log.sink")
    else {
        println!("{service_id}: no cap:log.sink endpoint provided; exiting");
        return Ok(());
    };

    if log_sink.kind != "ipc-endpoint" || !log_sink.rights.iter().any(|right| right == "send") {
        return Err(format!(
            "cap:log.sink has unexpected kind/rights: kind={} rights={:?}",
            log_sink.kind, log_sink.rights
        ));
    }

    let Some(path) = log_sink.endpoint.strip_prefix("unix:") else {
        return Err(format!(
            "cap:log.sink endpoint must be unix:<path>, found {}",
            log_sink.endpoint
        ));
    };

    if Path::new(path).exists() {
        fs::remove_file(path)
            .map_err(|source| format!("failed to remove stale socket {path}: {source}"))?;
    }

    let listener = UnixListener::bind(path)
        .map_err(|source| format!("failed to bind log socket {path}: {source}"))?;
    println!("{service_id}: listening on {path}");

    if env::var("VERTEX_DEMO_RUN_ONCE").as_deref() == Ok("1") {
        accept_until_message(&listener, &service_id)?;
        return Ok(());
    }

    for stream in listener.incoming() {
        let stream = stream.map_err(|source| format!("failed to accept log client: {source}"))?;
        let _ = read_log_stream(stream, &service_id)?;
    }

    Ok(())
}

fn accept_until_message(listener: &UnixListener, service_id: &str) -> Result<(), String> {
    loop {
        let (stream, _) = listener
            .accept()
            .map_err(|source| format!("failed to accept log client: {source}"))?;
        if read_log_stream(stream, service_id)? {
            return Ok(());
        }
    }
}

fn read_log_stream(
    stream: std::os::unix::net::UnixStream,
    service_id: &str,
) -> Result<bool, String> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .map_err(|source| format!("failed to read log message: {source}"))?;

    if bytes == 0 {
        println!("{service_id}: client closed without a log message");
        Ok(false)
    } else {
        println!("{service_id}: received {}", line.trim_end());
        Ok(true)
    }
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
