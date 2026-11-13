//! Test files for r3i6nos's corpus (one file of each format the R36S's apps read; r3i6nos `corpus/`). The tiny songs some format
//! tests build by hand are written out by those tests when `NEUMETIK_CORPUS_DIR` names a folder, so they stay in one place:
//! `NEUMETIK_CORPUS_DIR=/some/dir cargo test -p neumetik-tracker corpus_file`. Without it they write nothing.

/// Writes `bytes` as `name` in `$NEUMETIK_CORPUS_DIR`, when it is set.
pub(crate) fn write(name: &str, bytes: &[u8]) {
    if let Some(dir) = std::env::var_os("NEUMETIK_CORPUS_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("the corpus folder");
        std::fs::write(dir.join(name), bytes).expect("a corpus file");
    }
}
