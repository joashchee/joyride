//! Writes every frame as raw RGBA to stdout: `cargo run --example dump_all FILE.ogv > out.rgba`.
use std::io::Write;
fn main() {
    let d = std::fs::read(std::env::args().nth(1).expect("a file")).unwrap();
    let mut out = std::io::stdout().lock();
    for f in ogv::Decoder::new(&d[..]).unwrap() {
        out.write_all(&f.unwrap().rgba).unwrap();
    }
}
