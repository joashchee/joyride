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
| Finding `gh` | `PATH`, then `/opt/homebrew/bin`, `/usr/local/bin` (a Dock-launched app has a short `PATH`); login is `gh`'s own keychain entry | `PATH`, `/usr/bin` | `PATH`, `gh.exe` |
| File preview: sound and MP4/WebM video | Played by WKWebView's own decoders (MP3, WAV, AAC, FLAC, H.264); Ogg Vorbis may not play there (the preview says so). Theora `.ogv` and MIDI/tracker music do not depend on it (`ogv`, `nttplay`) | WebKitGTK through GStreamer: which codecs play depends on the installed plugins (untested) | WebView2: MP3, WAV, AAC, H.264, WebM, Ogg Vorbis (untested) |
| Delete from the file tree | Moves to `~/.Trash` (a rename under a free name; no "Put Back" entry; another volume falls back to a permanent delete, which the person is told) | Permanent delete after a confirm (the XDG Trash is not used yet) | Permanent delete after a confirm (the Recycle Bin is not used yet) |
