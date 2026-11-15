//! `cargo run --example make_icons -- icon.png out_dir`: writes `AppIcon.icns`
//! and `AppIcon.ico` from one square PNG (the same code the exporters use).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 3 {
        eprintln!("usage: make_icons <icon.png> <out_dir>");
        std::process::exit(2);
    }
    let img = joyride_core::icons::load(std::path::Path::new(&a[1])).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(1) });
    let out = std::path::Path::new(&a[2]);
    std::fs::create_dir_all(out).unwrap();
    std::fs::write(out.join("AppIcon.icns"), joyride_core::icons::icns(&img).unwrap()).unwrap();
    std::fs::write(out.join("AppIcon.ico"), joyride_core::icons::ico(&img).unwrap()).unwrap();
}
