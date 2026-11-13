//! The Asset Library's core (docs/asset-library.md): what a usable asset is,
//! what its licence says, how to tell two files are the same, how a catalog
//! finds them again, and how a project's assets come in safely.
//!
//! Written clean-room for JoyrIDE from the *ideas* in ansiapps' Diskette and
//! Crunchy (staged duplicate elimination, a hash cache valid only while size
//! and modification time match, an FTS-indexed catalog); no code from either.
//!
//! Rules it keeps (CLAUDE.md rule 11): nothing is imported, and nothing is
//! "usable", until its licence is confirmed; imports never touch or delete
//! the originals; identical content is stored once.

pub mod add;
pub mod audio;
pub mod catalog;
pub mod dedup;
pub mod font;
pub mod generated;
pub mod import;
pub mod kind;
pub mod licence;
pub mod preview;
pub mod project;

pub use catalog::{Asset, Catalog, Filter, NewAsset, Preview};
pub use kind::Kind;
pub use licence::{Licence, Status, Tri};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("catalog: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
