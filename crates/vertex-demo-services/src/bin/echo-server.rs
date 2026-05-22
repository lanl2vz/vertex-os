use std::env;
use std::thread;
use std::time::Duration;

fn main() {
    let service_id = env::var("VERTEX_SERVICE_ID").unwrap_or_else(|_| "svc:echo-server".to_owned());
    let grants = env::var("VERTEX_GRANTED_CAPS").unwrap_or_default();

    println!("{service_id}: echo-server started");
    println!("{service_id}: granted capabilities [{grants}]");

    if has_capability(&grants, "cap:log.sink", "send") {
        println!("{service_id}: can send to cap:log.sink");
    } else {
        println!("{service_id}: cannot send to cap:log.sink");
    }

    if has_capability(&grants, "cap:net.tcp.8080", "listen") {
        println!("{service_id}: can listen on cap:net.tcp.8080");
    } else {
        println!("{service_id}: cannot listen on cap:net.tcp.8080");
    }

    if env::var("VERTEX_DEMO_STAY_ALIVE").as_deref() == Ok("1") {
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }
}

fn has_capability(grants: &str, capability: &str, right: &str) -> bool {
    grants.split(';').any(|grant| {
        let Some((granted_capability, rights)) = grant.split_once('=') else {
            return false;
        };
        granted_capability == capability
            && rights
                .split(',')
                .any(|granted_right| granted_right == right)
    })
}
