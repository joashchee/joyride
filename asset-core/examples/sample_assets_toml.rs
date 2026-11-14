//! Writes `.joyride/assets.toml` for a sample project from its `assets.tsv` (path, kind, licence, author, source, recipe, note), through
//! the Asset Library's own catalog: each file is hashed (BLAKE3), classified, given its licence (confirmed by "JoyrIDE maintainers",
//! since the maintainers made or sourced it), and recorded as used by the project. The sample is then a real project the library has
//! already seen, and its export gate and CREDITS.txt have something true to work from.
//!
//!     cargo run --offline --manifest-path asset-core/Cargo.toml --example sample_assets_toml -- samples/io-checker
//!
//! Every file under `assets/` must be in the table (a file with no row is an error: nothing is used with an unknown licence).

use asset_core::{catalog::NewAsset, dedup, kind, project, Catalog, Licence};
use std::path::{Path, PathBuf};

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("usage: sample_assets_toml <project folder>"));
    let name = root.file_name().unwrap().to_string_lossy().to_string();
    let tsv = std::fs::read_to_string(root.join("assets.tsv")).expect("assets.tsv (run the sample's tools/make-assets.mjs first)");
    let mut cat = Catalog::in_memory().expect("catalog");
    let mut listed = std::collections::BTreeSet::new();
    for line in tsv.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let (path, spdx, author, source, recipe, note) = (f[0], f[2], f[3], f[4], f.get(5).copied().unwrap_or(""), f.get(6).copied().unwrap_or(""));
        let full = root.join(path);
        let bytes = std::fs::read(&full).unwrap_or_else(|e| panic!("{path}: {e}"));
        let hash = dedup::hash_bytes(&bytes);
        let k = kind::classify(Path::new(path));
        let file_name = Path::new(path).file_name().unwrap().to_string_lossy().to_string();
        let ext = Path::new(path).extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
        let (id, _) = cat.add_asset(&NewAsset { hash, size: bytes.len() as u64, kind: k, name: file_name, ext, stored: String::new() }).expect("add");
        let licence = Licence::from_spdx(spdx);
        cat.confirm_licence(id, &licence, "JoyrIDE maintainers", recipe).expect("confirm");
        cat.set_credit(id, author, source, note).expect("credit");
        cat.record_origin(id, if recipe.starts_with("copied") { "copied" } else { "generated" }, recipe).expect("origin");
        cat.record_use(id, &name, path).expect("use");
        listed.insert(path.to_string());
    }
    // Everything under assets/ must have a row.
    let mut missing = Vec::new();
    for e in walkdir::WalkDir::new(root.join("assets")).into_iter().filter_map(|e| e.ok()).filter(|e| e.file_type().is_file()) {
        let rel = e.path().strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        if !listed.contains(&rel) {
            missing.push(rel);
        }
    }
    if !missing.is_empty() {
        eprintln!("files with no licence row in assets.tsv: {missing:?}");
        std::process::exit(1);
    }
    project::write_assets_toml(&cat, &name, &root).expect("write");
    println!("{} assets recorded in {}/.joyride/assets.toml", listed.len(), root.display());
}
