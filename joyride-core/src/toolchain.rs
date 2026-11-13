//! LÖVE versions and the toolchain (spec 3A, 3B): which version a project
//! targets, the `.luarc.json` that points the language server at that
//! version's types, and verifying a signed toolchain manifest.

use crate::{err, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// A LÖVE release a project can target: 11.5 (bundled) or 12.0 and later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoveVersion {
    pub major: u32,
    pub minor: u32,
}

pub const BUNDLED: LoveVersion = LoveVersion { major: 11, minor: 5 };

impl LoveVersion {
    pub fn parse(s: &str) -> Option<LoveVersion> {
        let (a, b) = s.trim().split_once('.')?;
        let v = LoveVersion { major: a.parse().ok()?, minor: b.parse().ok()? };
        (v == BUNDLED || v >= LoveVersion { major: 12, minor: 0 }).then_some(v)
    }
    pub fn is_12_or_later(self) -> bool {
        self.major >= 12
    }
}

impl std::fmt::Display for LoveVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// The `.luarc.json` for a project: LuaJIT runtime, `love` global, and the
/// active version's LuaCATS types plus `./libs` in the workspace library.
/// `types_dir` is where that version's definitions live (the bundled
/// resources for 11.5, the toolchain cache for 12.0+).
pub fn luarc(version: LoveVersion, types_dir: &str, has_libs: bool) -> String {
    let mut lib = vec![types_dir.to_string()];
    if has_libs {
        lib.push("./libs".into());
    }
    let v = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
        "runtime.version": "LuaJIT",
        "workspace.library": lib,
        "workspace.checkThirdParty": false,
        "diagnostics.globals": ["love"],
        "joyride.loveVersion": version.to_string(),
    });
    serde_json::to_string_pretty(&v).unwrap() + "\n"
}

/// The LÖVE version a project folder targets: `love_version` in its
/// `love.toml`, 11.5 when there is no `love.toml`. A version that is not 11.5
/// or 12.0+ is an error naming it.
pub fn project_love_version(root: &std::path::Path) -> Result<LoveVersion> {
    let Ok(text) = std::fs::read_to_string(root.join("love.toml")) else { return Ok(BUNDLED) };
    let m = crate::manifest::Manifest::parse(&text)?;
    LoveVersion::parse(&m.project.love_version).ok_or_else(|| crate::Error::Msg(format!("love.toml asks for LÖVE {}, which JoyrIDE does not support (11.5, or 12.0 and later)", m.project.love_version)))
}

/// The `conf.lua` version line to enforce for a target.
pub fn conf_version_line(version: LoveVersion) -> String {
    format!("t.version = \"{version}\"")
}

/// The signed toolchain manifest (`manifest.json`, hosted on GitHub): which
/// runtimes, API definitions and templates exist for each LÖVE version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolchainManifest {
    pub schema: u32,
    pub issued: String,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// `runtime`, `love-api`, `love.js`, `ios-template`, `android-template`...
    pub kind: String,
    pub love_version: String,
    /// `macos`, `windows`, `linux` or `any`.
    pub platform: String,
    pub url: String,
    /// Hex BLAKE3 of the download, checked after fetching.
    pub blake3: String,
    pub size: u64,
}

/// Parses a manifest only if its detached signature (hex Ed25519 over the
/// exact bytes) verifies against the pinned public key. An unsigned or
/// tampered manifest is refused: nothing below is trusted before this.
pub fn verify_manifest(bytes: &[u8], signature_hex: &str, public_key: &[u8; 32]) -> Result<ToolchainManifest> {
    let key = VerifyingKey::from_bytes(public_key).map_err(|e| crate::Error::Msg(format!("bad public key: {e}")))?;
    let sig_bytes = hex::decode(signature_hex.trim()).map_err(|_| crate::Error::Msg("signature is not hex".into()))?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|_| crate::Error::Msg("signature has the wrong length".into()))?;
    if key.verify(bytes, &sig).is_err() {
        return err("toolchain manifest signature does not match: refusing it");
    }
    let m: ToolchainManifest = serde_json::from_slice(bytes).map_err(|e| crate::Error::Msg(format!("manifest: {e}")))?;
    if m.schema != 1 {
        return err(format!("manifest schema {} is not supported", m.schema));
    }
    Ok(m)
}

impl ToolchainManifest {
    pub fn find(&self, kind: &str, v: LoveVersion, platform: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.kind == kind && e.love_version == v.to_string() && (e.platform == platform || e.platform == "any"))
    }
    /// Versions the manifest offers that this IDE does not have installed.
    pub fn versions(&self) -> Vec<LoveVersion> {
        let mut v: Vec<_> = self.entries.iter().filter_map(|e| LoveVersion::parse(&e.love_version)).collect();
        v.sort();
        v.dedup();
        v
    }
}

/// Checks a downloaded file against its manifest entry. Call before using it.
pub fn check_download(bytes: &[u8], e: &Entry) -> Result<()> {
    if bytes.len() as u64 != e.size {
        return err(format!("{} is {} bytes, the manifest says {}", e.url, bytes.len(), e.size));
    }
    // Same hash the asset library uses; hex.
    let got = asset_core::dedup::hash_bytes(bytes);
    if got != e.blake3.to_lowercase() {
        return err(format!("{} does not match its hash", e.url));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn project_version_comes_from_love_toml() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(super::project_love_version(d.path()).unwrap(), super::BUNDLED, "no love.toml");
        let toml = |v: &str| format!("[project]\nname = \"g\"\nbundle_id = \"com.x.g\"\nversion = \"0.1.0\"\nlove_version = \"{v}\"\n");
        std::fs::write(d.path().join("love.toml"), toml("12.0")).unwrap();
        assert_eq!(super::project_love_version(d.path()).unwrap().to_string(), "12.0");
        std::fs::write(d.path().join("love.toml"), toml("0.10")).unwrap();
        assert!(super::project_love_version(d.path()).unwrap_err().to_string().contains("0.10"));
        std::fs::write(d.path().join("love.toml"), "not toml [").unwrap();
        assert!(super::project_love_version(d.path()).is_err());
    }

    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn versions() {
        assert_eq!(LoveVersion::parse("11.5"), Some(BUNDLED));
        assert!(LoveVersion::parse("12.0").unwrap().is_12_or_later());
        assert!(LoveVersion::parse("12.3").is_some());
        assert_eq!(LoveVersion::parse("11.4"), None);
        assert_eq!(LoveVersion::parse("0.10"), None);
        assert_eq!(LoveVersion::parse("twelve"), None);
        assert!(LoveVersion::parse("12.0").unwrap() > BUNDLED);
    }

    #[test]
    fn luarc_points_at_the_active_version() {
        let j: serde_json::Value = serde_json::from_str(&luarc(LoveVersion::parse("12.0").unwrap(), "/tc/12.0/love-api", true)).unwrap();
        assert_eq!(j["workspace.library"], serde_json::json!(["/tc/12.0/love-api", "./libs"]));
        assert_eq!(j["runtime.version"], "LuaJIT");
        assert_eq!(j["diagnostics.globals"][0], "love");
        let none: serde_json::Value = serde_json::from_str(&luarc(BUNDLED, "t", false)).unwrap();
        assert_eq!(none["workspace.library"].as_array().unwrap().len(), 1);
    }

    fn keypair() -> (SigningKey, [u8; 32]) {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let pk = sk.verifying_key().to_bytes();
        (sk, pk)
    }

    fn manifest_json() -> Vec<u8> {
        let data = b"runtime bytes";
        serde_json::to_vec(&serde_json::json!({
            "schema": 1, "issued": "2026-11-13",
            "entries": [{"kind":"runtime","love_version":"12.0","platform":"any","url":"https://x/l.zip",
                         "blake3": asset_core::dedup::hash_bytes(data), "size": data.len()}]
        }))
        .unwrap()
    }

    #[test]
    fn a_signed_manifest_verifies_and_tampering_is_refused() {
        let (sk, pk) = keypair();
        let bytes = manifest_json();
        let sig = hex::encode(sk.sign(&bytes).to_bytes());
        let m = verify_manifest(&bytes, &sig, &pk).unwrap();
        assert_eq!(m.versions(), vec![LoveVersion::parse("12.0").unwrap()]);
        let e = m.find("runtime", LoveVersion::parse("12.0").unwrap(), "macos").unwrap();
        assert!(check_download(b"runtime bytes", e).is_ok());
        assert!(check_download(b"runtime bytez", e).is_err());
        assert!(check_download(b"short", e).is_err());
        let mut tampered = bytes.clone();
        tampered[10] ^= 1;
        assert!(verify_manifest(&tampered, &sig, &pk).is_err());
        assert!(verify_manifest(&bytes, "00", &pk).is_err());
        let other = SigningKey::from_bytes(&[9u8; 32]).verifying_key().to_bytes();
        assert!(verify_manifest(&bytes, &sig, &other).is_err());
    }
}
