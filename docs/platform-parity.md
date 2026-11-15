# Platform parity

One row per macOS-specific mechanism (CLAUDE.md rule 8): what it does on the
Mac, and what the Linux and Windows builds do instead.

| Mechanism | macOS | Linux | Windows |
|---|---|---|---|
| Bundled LÖVE 11.5 for Run | `love.app` in `Resources/resources/toolchains/love-11.5/`, run as `Contents/MacOS/love` (`runner::find_love`) | Not bundled yet: `love` from the distribution or the toolchains folder | Not bundled yet: `love.exe` from the toolchains folder |
| `lua-language-server`, StyLua | Both `arm64` and `x64` builds under `toolchains/<tool>/<arch>/`, so the universal app runs the right one | Same layout, `linux-*` assets (fetch script to extend) | Same layout, `win32-*` assets (fetch script to extend) |
| Nested binaries and signing | `scripts/sign-macos.sh` signs every Mach-O in the app's resources (LÖVE with `src-tauri/macos/love.entitlements`, StyLua, lua-language-server, nttplay) with the Developer ID, hardened runtime and secure timestamp, restores the symlinks Tauri's resource copy flattens, re-signs the app and audits each executable; notarization refuses the app otherwise (**not yet run with a real identity**) | none | none (Authenticode later) |
| PortMaster export | Not LÖVE-related: the zip never carries a LÖVE binary, it sources PortMaster's own `love_<ver>` runtime (see CLAUDE.md rule 12) | same | same |
| Folder picker | Native `NSOpenPanel` through `tauri-plugin-dialog` | GTK portal | Windows dialog |
| Finding `gh` | `PATH`, then `/opt/homebrew/bin`, `/usr/local/bin` (a Dock-launched app has a short `PATH`); login is `gh`'s own keychain entry | `PATH`, `/usr/bin` | `PATH`, `gh.exe` |
| File preview: sound and MP4/WebM video | Played by WKWebView's own decoders (MP3, WAV, AAC, FLAC, H.264); Ogg Vorbis (which WKWebView does not play) is decoded by Symphonia and played as WAV, as is a Theora `.ogv`'s Vorbis track (same on every platform). Theora `.ogv` and MIDI/tracker music do not depend on it (`ogv`, `nttplay`) | WebKitGTK through GStreamer: which codecs play depends on the installed plugins (untested) | WebView2: MP3, WAV, AAC, H.264, WebM, Ogg Vorbis (untested) |
| Delete from the file tree | Moves to `~/.Trash` (a rename under a free name; no "Put Back" entry; another volume falls back to a permanent delete, which the person is told) | Permanent delete after a confirm (the XDG Trash is not used yet) | Permanent delete after a confirm (the Recycle Bin is not used yet) |
| App icon in an exported game | `.icns` in `Contents/Resources`; `love.app`'s `CFBundleIconName` (asset catalog `Assets.car`) is removed so `CFBundleIconFile` wins (`joyride-core/src/icons.rs`, `export::macos`) | none yet (the AppImage's own icon is inside its squashfs) | `.ico` is written by `icons::ico` but is **not yet put into the exe's resources** |
| `nttplay` (MIDI, tracker and text-music preview) | One universal Mach-O in `resources/toolchains/nttplay/`, built by `scripts/build-nttplay.sh` and signed with the rest by `scripts/sign-macos.sh` | not built yet (the script builds this machine's, `nttplay` in the same folder) | not built yet (`nttplay.exe` is looked for) |
| Resource files in the app | Tauri copies them as plain files (symlinks flattened); `sign-macos.sh` restores them (a `.framework` needs `Versions/Current`) | symlinks kept by the `.deb`/`.rpm` | not applicable |
| Error log, crash reports, drafts | Log in `~/Library/Logs/com.ansiapps.joyride/joyride.log`; reports and drafts under `~/Library/Application Support/com.ansiapps.joyride/`; "Show" opens the folder with `open_path` | Tauri's `app_log_dir` / `app_data_dir` (untested) | same (untested); the panic hook and session marker are plain files |
