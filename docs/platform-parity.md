# Platform parity

One row per macOS-specific mechanism (CLAUDE.md rule 8): what it does on the
Mac, and what the Linux and Windows builds do instead.

| Mechanism | macOS | Linux | Windows |
|---|---|---|---|
| Bundled LÖVE 11.5 for Run | `love.app` in `Resources/resources/toolchains/love-11.5/`, run as `Contents/MacOS/love` (`runner::find_love`) | Not bundled yet: `love` from the distribution or the toolchains folder | Not bundled yet: `love.exe` from the toolchains folder |
| `lua-language-server`, StyLua | Both `arm64` and `x64` builds under `toolchains/<tool>/<arch>/`, so the universal app runs the right one | Same layout, `linux-*` assets (fetch script to extend) | Same layout, `win32-*` assets (fetch script to extend) |
| Nested binaries and signing | Every bundled executable is signed with the Developer ID and the hardened runtime by `scripts/build-release.sh`, or notarization fails (**check at the first release build**) | none | none (Authenticode later) |
| PortMaster export | Not LÖVE-related: the zip never carries a LÖVE binary, it sources PortMaster's own `love_<ver>` runtime (see CLAUDE.md rule 12) | same | same |
| Folder picker | Native `NSOpenPanel` through `tauri-plugin-dialog` | GTK portal | Windows dialog |
