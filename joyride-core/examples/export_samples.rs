//! Prints what the PortMaster exporter generates, as JSON, for
//! scripts/check-portmaster.py to compare with PortMaster's real ports.
use joyride_core::export::{launch_script, port_json, Port, PORTMASTER_LOVE_VERSIONS};

fn main() {
    let port = Port { slug: "my-game".into(), title: "My Game".into(), description: "A game.".into(), love_version: PORTMASTER_LOVE_VERSIONS[0].into() };
    let pj: serde_json::Value = serde_json::from_str(&port_json(&port, "My Game.sh")).unwrap();
    println!("{}", serde_json::json!({ "launch_script": launch_script(&port), "port_json": pj, "love_versions": PORTMASTER_LOVE_VERSIONS }));
}
