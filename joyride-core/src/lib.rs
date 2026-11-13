//! JoyrIDE's Tauri-free core (docs/JoyrIDE_LÖVE-Implementation-Spec.md): the
//! logic behind the IDE's panels, kept apart from the shell so it can be
//! tested alone and shared with a web build. `src-tauri` will wrap it in
//! commands.
//!
//! - [`manifest`]: `love.toml` and `love.lock` (spec 3D); [`pkg`] installs;
//!   [`library`] the curated list of libraries, pinned and licensed.
//! - [`toolchain`]: LÖVE versions, `.luarc.json`, the signed toolchain
//!   manifest (spec 3A, 3B).
//! - [`lua`]: a small Lua lexer; [`scan`] the auto-porter's static analysis
//!   (spec 4B); [`lint`] version-calibrated LÖVE lints (spec 3B).
//! - [`github`]: GitHub through `gh` and `git`, never a token of ours.
//! - [`format`]: formatting Lua with StyLua.
//! - [`lsp`]: `lua-language-server` over stdio, framed for the editor.
//! - [`trace`] and [`runner`]: running a game and linking its errors (3C).
//! - [`shim`], [`overlay`]: target shims and the zero-mutation patch
//!   overlay, merged into a staged copy at build time (spec 4A, 4C).
//! - [`apple_mobile`]: an Xcode project around a `.love` for iOS and iPadOS (spec 5.2).
//! - [`web`]: a love.js page around a `.love` (spec 5.4).
//! - [`android`]: a `love-android` Gradle project around a `.love` (spec 5.3).
//! - [`compat`]: the 11.5 variant of a 12.0 game, for every export.
//! - [`export`]: `.love`, desktop and PortMaster packages (spec 5).
//! - [`files`]: the open project's tree and text files, confined to the folder.
//! - [`editor`]: what Monaco's providers need (colour swatches, asset hover);
//!   [`docs`]: the offline API reference search and snippets (spec 3B).

pub mod android;
pub mod apple_mobile;
pub mod compat;
pub mod docs;
pub mod editor;
pub mod export;
pub mod files;
pub mod format;
pub mod github;
pub mod library;
pub mod lint;
pub mod lsp;
pub mod lua;
pub mod manifest;
pub mod overlay;
pub mod pipeline;
pub mod pkg;
pub mod runner;
pub mod scan;
pub mod shim;
pub mod toolchain;
pub mod web;
pub mod trace;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Msg(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn err<T>(s: impl Into<String>) -> Result<T> {
    Err(Error::Msg(s.into()))
}
