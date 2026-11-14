//! Shows what JoyrIDE would find in a project: `cargo run --offline --example detect_project -- <folder>`.
//! The libraries it uses that are in the list, and the ones it would offer to add. Reads files only.
use joyride_core::{detect, library::Library};

fn main() {
    let dir = std::env::args().nth(1).expect("usage: detect_project <project folder>");
    let d = detect::detect(std::path::Path::new(&dir), &Library::bundled()).expect("detect");
    println!("in the list ({}):", d.used.len());
    for u in &d.used {
        println!("  {}{}  [{}]", u.name, if u.installed { " (in love.toml)" } else { "" }, u.how);
    }
    for u in &d.unresolved {
        println!("  not provided (not offered): {u}");
    }
    println!("would be offered ({}):", d.new.len());
    for f in &d.new {
        println!("  {}  {}  {}  {}  [{}]{}", f.name, f.licence.as_deref().unwrap_or("licence unknown"), f.version.as_deref().unwrap_or(""), f.git.as_deref().unwrap_or("no address"), f.how.join("; "), if f.missing { "  MISSING" } else { "" });
    }
}
