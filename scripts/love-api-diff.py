#!/usr/bin/env python3
"""Lists which LÖVE functions exist in one version and not another, measured on
real binaries (the source of joyride-core's lint tables ONLY_IN_12 and
REMOVED_IN_12, src/lint.rs). Runs each LÖVE headless with SDL's dummy drivers
and every module that can load without a window; where a display is
available, pass --graphics to include love.graphics and the rest.

  scripts/love-api-diff.py                      # 11.5 vs 12.0, r3i6nos's builds
  scripts/love-api-diff.py --old PATH --new PATH [--graphics]

Builds are found as in joyride-core/tests/e2e_love.rs: JOYRIDE_LOVE_11_5 and
JOYRIDE_LOVE_12_0, then ../r3i6nos/vendor/love-mac/<version>/love.app, then
/Applications. Output: removed (in old, not in new) and added (in new, not in
old), internal names (`_x`, `handlers`) left out.
"""
import argparse, os, subprocess, sys, tempfile
from pathlib import Path

CONF = """function love.conf(t)
  t.identity = "api-dump"
  for _, m in ipairs({%s}) do t.modules[m] = false end
end
"""
MAIN = r"""
local out = {}
local function walk(prefix, t, depth)
  local keys = {}
  for k in pairs(t) do keys[#keys + 1] = k end
  table.sort(keys, function(a, b) return tostring(a) < tostring(b) end)
  for _, k in ipairs(keys) do
    local v, name = t[k], prefix .. "." .. tostring(k)
    if type(v) == "function" then out[#out + 1] = name
    elseif type(v) == "table" and depth < 1 and not tostring(k):match("^_") then walk(name, v, depth + 1) end
  end
end
walk("love", love, 0)
local f = io.open(os.getenv("DUMP_OUT"), "w")
f:write(table.concat(out, "\n"), "\n")
f:close()
love.event.quit(0)
"""

def find(version, explicit):
    if explicit:
        return Path(explicit)
    var = "JOYRIDE_LOVE_" + version.replace(".", "_")
    here = Path(__file__).resolve().parent.parent
    for p in [os.environ.get(var), here.parent / f"r3i6nos/vendor/love-mac/{version}/love.app/Contents/MacOS/love",
              f"/Applications/love-{version}.app/Contents/MacOS/love", f"/Applications/love-{version}a.app/Contents/MacOS/love"]:
        if p and Path(p).exists():
            return Path(p)
    sys.exit(f"no LÖVE {version} found (set {var})")

def dump(love, graphics):
    mods = ["window", "graphics", "video", "joystick", "touch", "mouse", "keyboard", "audio"]
    if graphics:
        mods = ["video"]
    with tempfile.TemporaryDirectory() as d:
        (Path(d) / "conf.lua").write_text(CONF % ", ".join(f'"{m}"' for m in mods))
        (Path(d) / "main.lua").write_text(MAIN)
        out = Path(d) / "out.txt"
        env = dict(os.environ, DUMP_OUT=str(out), SDL_AUDIODRIVER="dummy", ALSOFT_DRIVERS="null")
        if not graphics:
            env["SDL_VIDEODRIVER"] = "dummy"
        r = subprocess.run([str(love), d], env=env, capture_output=True, text=True, timeout=60)
        if not out.exists():
            sys.exit(f"{love} produced nothing (exit {r.returncode}):\n{r.stdout}{r.stderr}")
        return {l for l in out.read_text().split("\n") if l and ".handlers." not in l and "._" not in l}

ap = argparse.ArgumentParser()
ap.add_argument("--old", help="old LÖVE binary (default 11.5)")
ap.add_argument("--new", help="new LÖVE binary (default 12.0)")
ap.add_argument("--graphics", action="store_true", help="keep graphics and window (needs a display)")
a = ap.parse_args()
old, new = dump(find("11.5", a.old), a.graphics), dump(find("12.0", a.new), a.graphics)
print(f"## removed ({len(old - new)}): in the old version, not the new")
print("\n".join(sorted(old - new)))
print(f"\n## added ({len(new - old)}): in the new version, not the old")
print("\n".join(sorted(new - old)))
if not a.graphics:
    print("\n(love.graphics, window, input and audio were not loaded; rerun with --graphics on a machine with a display)", file=sys.stderr)
