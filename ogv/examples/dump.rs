//! Prints a few pixels of a video's first frame: `cargo run --example dump FILE.ogv`.
fn main() {
    let d = std::fs::read(std::env::args().nth(1).expect("a file")).unwrap();
    let f = ogv::Decoder::new(&d[..]).unwrap().next_frame().unwrap().unwrap();
    let y = (f.height / 2).min(f.height - 1) as usize;
    let row: Vec<_> = (0..7)
        .map(|i| {
            let x = (f.width as usize * (2 * i + 1)) / 14;
            let o = (y * f.width as usize + x) * 4;
            (f.rgba[o], f.rgba[o + 1], f.rgba[o + 2])
        })
        .collect();
    println!("{row:?} t={} i={}", f.time, f.index);
}
