//! Prints the generated PortMaster launch script (for shellcheck in
//! scripts/check-linux.sh).
fn main() {
    let port = joyride_core::export::Port { slug: "my-game".into(), title: "My Game".into(), description: String::new(), love_version: "11.5".into() };
    print!("{}", joyride_core::export::launch_script(&port));
}
