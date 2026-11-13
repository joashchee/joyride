// Compiles the vendored libogg and libtheora decoder (../vendor/xiph) and the
// small C shim that drives them.
fn main() {
    let x = std::path::Path::new("../vendor/xiph");
    let mut b = cc::Build::new();
    b.include(x.join("ogg/include")).include(x.join("theora/include")).include(x.join("theora/lib")).warnings(false).opt_level(2);
    for dir in ["ogg/src", "theora/lib"] {
        for e in std::fs::read_dir(x.join(dir)).expect("vendor/xiph is missing: run scripts/vendor-xiph.sh") {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|e| e == "c") {
                b.file(p);
            }
        }
    }
    b.file("csrc/shim.c");
    b.compile("ogv_c");
    println!("cargo:rerun-if-changed=csrc/shim.c");
    println!("cargo:rerun-if-changed=../vendor/xiph");
}
