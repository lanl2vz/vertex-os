use std::env;
use std::thread;
use std::time::Duration;

fn main() {
    let service_id = env::var("VERTEX_SERVICE_ID").unwrap_or_else(|_| "svc:netstack".to_owned());
    let grants = env::var("VERTEX_GRANTED_CAPS").unwrap_or_default();

    println!("{service_id}: hosted netstack placeholder started");
    println!("{service_id}: granted capabilities [{grants}]");

    if env::var("VERTEX_DEMO_STAY_ALIVE").as_deref() == Ok("1") {
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }
}
