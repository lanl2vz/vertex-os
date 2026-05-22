use std::env;

fn main() {
    let service_id = env::var("VERTEX_SERVICE_ID").unwrap_or_else(|_| "svc:netstack".to_owned());
    let grants = env::var("VERTEX_GRANTED_CAPS").unwrap_or_default();
    let provided = env::var("VERTEX_PROVIDED_CAPS").unwrap_or_default();

    println!("{service_id}: hosted netstack placeholder started");
    println!("{service_id}: granted capabilities [{grants}]");
    println!("{service_id}: provided capabilities [{provided}]");
}
