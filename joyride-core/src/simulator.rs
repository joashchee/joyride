//! The viewport simulator (spec 5.6): run the project with a target's patch
//! overlay active inside a preset screen size. The project is staged into a
//! scratch folder like an export, so the source is never touched.

use crate::overlay;
use crate::scan::{self, Target};
use crate::{err, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A simulated screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    /// The overlay this screen belongs to.
    pub target: Target,
}

/// Window sizes in points (the window LÖVE opens), not device pixels.
pub const PRESETS: &[Preset] = &[
    Preset { id: "pm-640x480", name: "PortMaster 640x480 (4:3)", width: 640, height: 480, target: Target::PortMaster },
    Preset { id: "pm-720x720", name: "PortMaster 720x720 (1:1)", width: 720, height: 720, target: Target::PortMaster },
    Preset { id: "pm-1280x720", name: "PortMaster 1280x720 (16:9)", width: 1280, height: 720, target: Target::PortMaster },
    Preset { id: "iphone", name: "iPhone landscape 844x390", width: 844, height: 390, target: Target::Mobile },
    Preset { id: "iphone-portrait", name: "iPhone portrait 390x844", width: 390, height: 844, target: Target::Mobile },
    Preset { id: "ipad", name: "iPad landscape 1180x820", width: 1180, height: 820, target: Target::Mobile },
    Preset { id: "android", name: "Android landscape 915x412", width: 915, height: 412, target: Target::Mobile },
    Preset { id: "android-portrait", name: "Android portrait 412x915", width: 412, height: 915, target: Target::Mobile },
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Stages `root` into `staging` (empty or absent) with the preset's overlay
/// and window size; returns the folder to run. Makes the overlay first if the
/// project has none yet.
pub fn prepare(root: &Path, preset_id: &str, staging: &Path) -> Result<PathBuf> {
    let Some(p) = preset(preset_id) else { return err(format!("unknown screen `{preset_id}`")) };
    if !root.join(".joyride/patches").join(overlay::target_dir_name(p.target)).join("_jr_shim.lua").is_file() {
        let s = scan::scan_project(root);
        overlay::write(root, &overlay::generate(p.target, &s))?;
    }
    let mut forced = BTreeMap::new();
    forced.insert("window.width".to_string(), toml::Value::Integer(p.width as i64));
    forced.insert("window.height".to_string(), toml::Value::Integer(p.height as i64));
    overlay::stage_forced(root, p.target, staging, &BTreeMap::new(), &forced, None)?;
    Ok(staging.to_path_buf())
}

/// The project file a path in a staged copy stands for (`_orig_main.lua` is
/// the game's `main.lua`).
pub fn source_name(rel: &str) -> &str {
    match rel {
        "_orig_main.lua" => "main.lua",
        "_orig_conf.lua" => "conf.lua",
        r => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_with_the_preset_size_and_leaves_the_source() {
        let d = std::env::temp_dir().join(format!("jr-sim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (root, stage) = (d.join("game"), d.join("stage"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("main.lua"), "function love.draw() end\n").unwrap();
        std::fs::write(root.join("conf.lua"), "function love.conf(t) t.window.width = 800 end\n").unwrap();
        let out = prepare(&root, "pm-640x480", &stage).unwrap();
        let conf = std::fs::read_to_string(out.join("conf.lua")).unwrap();
        assert!(conf.contains("t.window.width = 640") && conf.contains("t.window.height = 480"), "{conf}");
        assert!(out.join("_orig_main.lua").is_file() && out.join("_jr_shim.lua").is_file());
        assert_eq!(std::fs::read_to_string(root.join("main.lua")).unwrap(), "function love.draw() end\n");
        assert!(std::fs::read_to_string(root.join("conf.lua")).unwrap().contains("800"));
        assert!(prepare(&root, "nope", &d.join("s2")).is_err());
        assert_eq!(source_name("_orig_main.lua"), "main.lua");
        let _ = std::fs::remove_dir_all(&d);
    }
}
