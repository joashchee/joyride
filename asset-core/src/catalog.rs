//! The catalog: one SQLite file holding every asset, its licence, tags, where
//! it came from and which projects use it, with full-text search.
//!
//! An asset is identified by the BLAKE3 hash of its content, so the same
//! bytes are one record however many projects or paths hold them.

use crate::audio::{AudioInfo, Levels};
use crate::dedup;
use crate::kind::Kind;
use crate::licence::{Licence, Status, Tri};
use crate::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

const SCHEMA: &str = "
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS assets (
    id INTEGER PRIMARY KEY,
    hash TEXT NOT NULL UNIQUE,
    size INTEGER NOT NULL,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    ext TEXT NOT NULL,
    stored TEXT NOT NULL,
    created INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS licences (
    asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    spdx TEXT NOT NULL,
    commercial TEXT NOT NULL,
    modification TEXT NOT NULL,
    redistribution TEXT NOT NULL,
    attribution INTEGER NOT NULL,
    share_alike INTEGER NOT NULL,
    author TEXT NOT NULL DEFAULT '',
    source_url TEXT NOT NULL DEFAULT '',
    attribution_text TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    confirmed_by TEXT NOT NULL DEFAULT '',
    confirmed_at INTEGER NOT NULL DEFAULT 0,
    note TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS tags (
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY (asset_id, tag)
);
CREATE TABLE IF NOT EXISTS uses (
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    project TEXT NOT NULL,
    path TEXT NOT NULL,
    PRIMARY KEY (asset_id, project, path)
);
CREATE TABLE IF NOT EXISTS origins (
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    detail TEXT NOT NULL,
    PRIMARY KEY (asset_id, kind, detail)
);
CREATE TABLE IF NOT EXISTS audio (
    asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    codec TEXT NOT NULL,
    container TEXT NOT NULL,
    sample_rate INTEGER NOT NULL,
    channels INTEGER NOT NULL,
    bit_depth INTEGER,
    duration_ms INTEGER NOT NULL,
    lossless INTEGER NOT NULL,
    peak_dbfs REAL,
    rms_dbfs REAL
);
CREATE TABLE IF NOT EXISTS rights (
    asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    ai_involvement TEXT NOT NULL DEFAULT 'none',
    licensor TEXT NOT NULL DEFAULT '',
    territory TEXT NOT NULL DEFAULT 'worldwide',
    expires TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS hash_cache (
    path TEXT PRIMARY KEY,
    size INTEGER NOT NULL,
    mtime_ns INTEGER NOT NULL,
    hash TEXT NOT NULL
);
CREATE VIRTUAL TABLE IF NOT EXISTS asset_search USING fts5(name, tags, kind, spdx, author);
";

#[derive(Debug, Clone)]
pub struct NewAsset {
    pub hash: String,
    pub size: u64,
    pub kind: Kind,
    pub name: String,
    pub ext: String,
    /// Where the library keeps the bytes.
    pub stored: String,
}

#[derive(Debug, Clone)]
pub struct Asset {
    pub id: i64,
    pub hash: String,
    pub size: u64,
    pub kind: Kind,
    pub name: String,
    pub ext: String,
    pub stored: String,
    pub created: i64,
    pub licence: Option<Licence>,
    pub status: Status,
    pub author: String,
    pub source_url: String,
    pub attribution_text: String,
    pub tags: Vec<String>,
}

impl Asset {
    /// Usable in a project only once its licence is confirmed.
    pub fn is_usable(&self) -> bool {
        self.status.is_confirmed()
    }
}

#[derive(Debug, Default, Clone)]
pub struct Filter {
    pub kind: Option<Kind>,
    pub confirmed_only: bool,
    /// Only assets whose licence permits commercial use.
    pub commercial_only: bool,
    /// A tag, or a branch of the hierarchy: `game:area` matches
    /// `game:area/forest` and `game:area`.
    pub tag: Option<String>,
    /// Inclusive beats per minute, from `bpm:` tags.
    pub bpm: Option<(u32, u32)>,
    /// Inclusive length in milliseconds (audio only).
    pub duration_ms: Option<(u64, u64)>,
}

/// How much of an asset's making was a machine's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiInvolvement {
    None,
    Assisted,
    Generated,
}

impl AiInvolvement {
    pub fn as_str(self) -> &'static str {
        match self {
            AiInvolvement::None => "none",
            AiInvolvement::Assisted => "assisted",
            AiInvolvement::Generated => "generated",
        }
    }
    pub fn parse(s: &str) -> AiInvolvement {
        match s {
            "assisted" => AiInvolvement::Assisted,
            "generated" => AiInvolvement::Generated,
            _ => AiInvolvement::None,
        }
    }
}

/// Rights beyond the licence (after Neumetik's §16): who licensed it, where
/// it is valid, when it lapses, and whether a model made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rights {
    pub ai_involvement: AiInvolvement,
    pub licensor: String,
    pub territory: String,
    /// `YYYY-MM-DD`, or empty for no expiry.
    pub expires: String,
}

pub struct Catalog {
    conn: Connection,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Catalog {
    pub fn open(path: &Path) -> Result<Catalog> {
        Self::init(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Catalog> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Catalog> {
        conn.execute_batch(SCHEMA)?;
        Ok(Catalog { conn })
    }

    /// Adds an asset, or returns the one already holding these bytes.
    /// The bool is true when a new record was made.
    pub fn add_asset(&mut self, a: &NewAsset) -> Result<(i64, bool)> {
        if let Some(id) = self.find_by_hash(&a.hash)? {
            return Ok((id, false));
        }
        self.conn.execute(
            "INSERT INTO assets (hash, size, kind, name, ext, stored, created) VALUES (?,?,?,?,?,?,?)",
            params![a.hash, a.size as i64, a.kind.as_str(), a.name, a.ext, a.stored, now()],
        )?;
        let id = self.conn.last_insert_rowid();
        self.reindex(id)?;
        Ok((id, true))
    }

    pub fn find_by_hash(&self, hash: &str) -> Result<Option<i64>> {
        Ok(self.conn.query_row("SELECT id FROM assets WHERE hash = ?", [hash], |r| r.get(0)).optional()?)
    }

    /// Sets an asset's licence as a guess, or unknown: it is not usable yet.
    pub fn set_licence(&mut self, id: i64, l: &Licence, guessed: bool) -> Result<()> {
        let status = if guessed { "guessed" } else { "unknown" };
        self.write_licence(id, l, status, "", 0, "")
    }

    /// Records that a person confirmed this licence. Required before use.
    pub fn confirm_licence(&mut self, id: i64, l: &Licence, by: &str, note: &str) -> Result<()> {
        if by.trim().is_empty() {
            return Err(Error::Other("a licence needs to be confirmed by someone".into()));
        }
        self.write_licence(id, l, "confirmed", by, now(), note)
    }

    fn write_licence(&mut self, id: i64, l: &Licence, status: &str, by: &str, at: i64, note: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO licences (asset_id, spdx, commercial, modification, redistribution, attribution, share_alike, status, confirmed_by, confirmed_at, note)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(asset_id) DO UPDATE SET spdx=?2, commercial=?3, modification=?4, redistribution=?5,
               attribution=?6, share_alike=?7, status=?8, confirmed_by=?9, confirmed_at=?10, note=?11",
            params![
                id,
                l.spdx,
                l.commercial_use.as_str(),
                l.modification.as_str(),
                l.redistribution.as_str(),
                l.attribution_required,
                l.share_alike,
                status,
                by,
                at,
                note
            ],
        )?;
        self.reindex(id)
    }

    /// Author, source and the attribution line a licence may require.
    pub fn set_credit(&mut self, id: i64, author: &str, source_url: &str, attribution_text: &str) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE licences SET author=?, source_url=?, attribution_text=? WHERE asset_id=?",
            params![author, source_url, attribution_text, id],
        )?;
        if n == 0 {
            return Err(Error::Other("set a licence before its credit".into()));
        }
        self.reindex(id)
    }

    pub fn add_tag(&mut self, id: i64, tag: &str) -> Result<()> {
        let tag = normalise_tag(tag);
        if tag.is_empty() {
            return Ok(());
        }
        self.conn.execute("INSERT OR IGNORE INTO tags (asset_id, tag) VALUES (?,?)", params![id, tag])?;
        self.reindex(id)
    }

    pub fn remove_tag(&mut self, id: i64, tag: &str) -> Result<()> {
        self.conn.execute("DELETE FROM tags WHERE asset_id=? AND tag=?", params![id, normalise_tag(tag)])?;
        self.reindex(id)
    }

    pub fn record_use(&mut self, id: i64, project: &str, path: &str) -> Result<()> {
        self.conn.execute("INSERT OR IGNORE INTO uses (asset_id, project, path) VALUES (?,?,?)", params![id, project, path])?;
        Ok(())
    }

    /// Every asset a project uses, with the project-relative path.
    pub fn project_assets(&self, project: &str) -> Result<Vec<(String, Asset)>> {
        let mut st = self.conn.prepare("SELECT asset_id, path FROM uses WHERE project = ? ORDER BY path")?;
        let rows: Vec<(i64, String)> = st.query_map([project], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        let mut out = Vec::new();
        for (id, path) in rows {
            if let Some(a) = self.get(id)? {
                out.push((path, a));
            }
        }
        Ok(out)
    }

    pub fn uses(&self, id: i64) -> Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare("SELECT project, path FROM uses WHERE asset_id=? ORDER BY project, path")?;
        let rows = st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Where an asset came from: a project path, a URL, or a generator
    /// recipe with its seed (so it can be made again).
    pub fn record_origin(&mut self, id: i64, kind: &str, detail: &str) -> Result<()> {
        self.conn.execute("INSERT OR IGNORE INTO origins (asset_id, kind, detail) VALUES (?,?,?)", params![id, kind, detail])?;
        Ok(())
    }

    pub fn origins(&self, id: i64) -> Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare("SELECT kind, detail FROM origins WHERE asset_id=? ORDER BY kind, detail")?;
        let rows = st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn set_audio(&mut self, id: i64, a: &AudioInfo, l: Option<Levels>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO audio (asset_id, codec, container, sample_rate, channels, bit_depth, duration_ms, lossless, peak_dbfs, rms_dbfs)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(asset_id) DO UPDATE SET codec=?2, container=?3, sample_rate=?4, channels=?5, bit_depth=?6,
               duration_ms=?7, lossless=?8, peak_dbfs=?9, rms_dbfs=?10",
            params![id, a.codec, a.container, a.sample_rate, a.channels, a.bit_depth, a.duration_ms as i64, a.lossless, l.map(|l| l.peak_dbfs), l.map(|l| l.rms_dbfs)],
        )?;
        Ok(())
    }

    /// The audio facts of an asset, if it is audio and has been read.
    pub fn audio(&self, id: i64) -> Result<Option<(AudioInfo, Option<Levels>)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT codec, container, sample_rate, channels, bit_depth, duration_ms, lossless, peak_dbfs, rms_dbfs FROM audio WHERE asset_id=?",
                [id],
                |r| {
                    let intern = |s: String, list: &[&'static str]| list.iter().copied().find(|x| *x == s).unwrap_or("other");
                    let info = AudioInfo {
                        codec: intern(r.get(0)?, &["pcm", "adpcm", "flac", "vorbis", "opus", "mp3", "wav-other"]),
                        container: intern(r.get(1)?, &["wav", "flac", "ogg", "mp3"]),
                        sample_rate: r.get(2)?,
                        channels: r.get(3)?,
                        bit_depth: r.get(4)?,
                        duration_ms: r.get::<_, i64>(5)? as u64,
                        lossless: r.get::<_, i64>(6)? != 0,
                    };
                    let lv = match (r.get::<_, Option<f64>>(7)?, r.get::<_, Option<f64>>(8)?) {
                        (Some(peak_dbfs), Some(rms_dbfs)) => Some(Levels { peak_dbfs, rms_dbfs }),
                        _ => None,
                    };
                    Ok((info, lv))
                },
            )
            .optional()?)
    }

    pub fn set_rights(&mut self, id: i64, r: &Rights) -> Result<()> {
        if !(r.expires.is_empty() || valid_date(&r.expires)) {
            return Err(Error::Other(format!("`{}` is not a YYYY-MM-DD date", r.expires)));
        }
        self.conn.execute(
            "INSERT INTO rights (asset_id, ai_involvement, licensor, territory, expires) VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(asset_id) DO UPDATE SET ai_involvement=?2, licensor=?3, territory=?4, expires=?5",
            params![id, r.ai_involvement.as_str(), r.licensor, r.territory, r.expires],
        )?;
        Ok(())
    }

    pub fn rights(&self, id: i64) -> Result<Rights> {
        Ok(self
            .conn
            .query_row("SELECT ai_involvement, licensor, territory, expires FROM rights WHERE asset_id=?", [id], |r| {
                Ok(Rights { ai_involvement: AiInvolvement::parse(&r.get::<_, String>(0)?), licensor: r.get(1)?, territory: r.get(2)?, expires: r.get(3)? })
            })
            .optional()?
            .unwrap_or(Rights { ai_involvement: AiInvolvement::None, licensor: String::new(), territory: "worldwide".into(), expires: String::new() }))
    }

    /// Ids of assets whose rights lapse on or before `date` (`YYYY-MM-DD`):
    /// the expiry alert. Compute `date` as today plus the warning window.
    pub fn expiring_by(&self, date: &str) -> Result<Vec<i64>> {
        let mut st = self.conn.prepare("SELECT asset_id FROM rights WHERE expires != '' AND expires <= ? ORDER BY expires, asset_id")?;
        let ids = st.query_map([date], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        Ok(ids)
    }

    /// The tags under a prefix with how many assets carry each: the browse
    /// list for a facet. `genre` lists every `genre:…` tag.
    pub fn facet_counts(&self, namespace: &str) -> Result<Vec<(String, u64)>> {
        let mut st = self.conn.prepare("SELECT tag, COUNT(*) FROM tags WHERE tag LIKE ? ESCAPE '\\' GROUP BY tag ORDER BY COUNT(*) DESC, tag")?;
        let pat = format!("{}:%", normalise_tag(namespace).replace('_', "\\_"));
        let rows = st.query_map([pat], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)))?.collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn get(&self, id: i64) -> Result<Option<Asset>> {
        let mut st = self.conn.prepare(&format!("{SELECT} WHERE a.id = ?"))?;
        let mut rows = st.query_map([id], row_to_asset)?;
        match rows.next() {
            Some(r) => Ok(Some(self.with_tags(r?)?)),
            None => Ok(None),
        }
    }

    /// Searches by words (each matches as a prefix, all must match) over
    /// name, tags, kind, licence id and author. An empty query lists all.
    pub fn search(&self, query: &str, f: &Filter) -> Result<Vec<Asset>> {
        let fts = fts_query(query);
        let mut sql = SELECT.to_string();
        let mut args: Vec<String> = Vec::new();
        let mut wh: Vec<String> = Vec::new();
        if let Some(q) = &fts {
            wh.push("a.id IN (SELECT rowid FROM asset_search WHERE asset_search MATCH ?)".into());
            args.push(q.clone());
        }
        if let Some(k) = f.kind {
            wh.push("a.kind = ?".into());
            args.push(k.as_str().into());
        }
        if f.confirmed_only {
            wh.push("l.status = 'confirmed'".into());
        }
        if f.commercial_only {
            wh.push("l.commercial = 'yes'".into());
        }
        if let Some(t) = &f.tag {
            wh.push("a.id IN (SELECT asset_id FROM tags WHERE tag = ? OR tag LIKE ? ESCAPE '\\')".into());
            args.push(normalise_tag(t));
            args.push(format!("{}/%", normalise_tag(t).replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
        }
        if let Some((lo, hi)) = f.bpm {
            wh.push(format!("a.id IN (SELECT asset_id FROM tags WHERE tag LIKE 'bpm:%' AND CAST(substr(tag, 5) AS INTEGER) BETWEEN {lo} AND {hi})"));
        }
        if let Some((lo, hi)) = f.duration_ms {
            wh.push(format!("a.id IN (SELECT asset_id FROM audio WHERE duration_ms BETWEEN {lo} AND {hi})"));
        }
        if !wh.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&wh.join(" AND "));
        }
        sql.push_str(" ORDER BY a.name COLLATE NOCASE, a.id");
        let mut st = self.conn.prepare(&sql)?;
        let rows: Vec<Asset> = st
            .query_map(rusqlite::params_from_iter(args.iter()), row_to_asset)?
            .collect::<std::result::Result<_, _>>()?;
        rows.into_iter().map(|a| self.with_tags(a)).collect()
    }

    fn with_tags(&self, mut a: Asset) -> Result<Asset> {
        let mut st = self.conn.prepare("SELECT tag FROM tags WHERE asset_id=? ORDER BY tag")?;
        a.tags = st.query_map([a.id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        Ok(a)
    }

    fn reindex(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM asset_search WHERE rowid = ?", [id])?;
        self.conn.execute(
            "INSERT INTO asset_search (rowid, name, tags, kind, spdx, author)
             SELECT a.id, a.name,
                    COALESCE((SELECT group_concat(tag, ' ') FROM tags WHERE asset_id = a.id), ''),
                    a.kind, COALESCE(l.spdx, ''), COALESCE(l.author, '')
             FROM assets a LEFT JOIN licences l ON l.asset_id = a.id WHERE a.id = ?",
            [id],
        )?;
        Ok(())
    }

    /// The hash of a file, reusing the last answer while its size and
    /// modification time are unchanged. A file changed since is read again.
    pub fn hash_cached(&mut self, path: &Path) -> Result<String> {
        let h = self.hash_many(&[path.to_path_buf()])?.pop().flatten();
        h.ok_or_else(|| Error::Other(format!("could not read {}", path.display())))
    }

    /// Hashes many files: cached answers are reused, the rest are read on
    /// several threads and remembered. An unreadable file gives `None`.
    pub fn hash_many(&mut self, paths: &[std::path::PathBuf]) -> Result<Vec<Option<String>>> {
        let mut out: Vec<Option<String>> = vec![None; paths.len()];
        let mut misses: Vec<(usize, i64, i64)> = Vec::new();
        for (i, p) in paths.iter().enumerate() {
            let Ok(meta) = std::fs::metadata(p) else { continue };
            let (size, mtime) = (meta.len() as i64, mtime_ns(&meta));
            let hit: Option<String> = self
                .conn
                .query_row(
                    "SELECT hash FROM hash_cache WHERE path=? AND size=? AND mtime_ns=?",
                    params![p.to_string_lossy(), size, mtime],
                    |r| r.get(0),
                )
                .optional()?;
            match hit {
                Some(h) => out[i] = Some(h),
                None => misses.push((i, size, mtime)),
            }
        }
        let hashed = dedup::par_map(&misses, |&(i, _, _)| dedup::hash_file(&paths[i]).ok());
        for (&(i, size, mtime), h) in misses.iter().zip(hashed) {
            if let Some(h) = h {
                self.conn.execute(
                    "INSERT INTO hash_cache (path, size, mtime_ns, hash) VALUES (?,?,?,?)
                     ON CONFLICT(path) DO UPDATE SET size=?2, mtime_ns=?3, hash=?4",
                    params![paths[i].to_string_lossy(), size, mtime, h],
                )?;
                out[i] = Some(h);
            }
        }
        Ok(out)
    }

    /// Forgets cached hashes of files that no longer exist.
    pub fn prune_hash_cache(&mut self) -> Result<usize> {
        let paths: Vec<String> = {
            let mut st = self.conn.prepare("SELECT path FROM hash_cache")?;
            let rows: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
            rows
        };
        let mut n = 0;
        for p in paths {
            if !Path::new(&p).exists() {
                self.conn.execute("DELETE FROM hash_cache WHERE path=?", [p])?;
                n += 1;
            }
        }
        Ok(n)
    }
}

const SELECT: &str = "SELECT a.id, a.hash, a.size, a.kind, a.name, a.ext, a.stored, a.created,
    l.spdx, l.commercial, l.modification, l.redistribution, l.attribution, l.share_alike,
    l.status, l.confirmed_by, l.confirmed_at, l.note, l.author, l.source_url, l.attribution_text
    FROM assets a LEFT JOIN licences l ON l.asset_id = a.id";

fn row_to_asset(r: &rusqlite::Row) -> rusqlite::Result<Asset> {
    let spdx: Option<String> = r.get(8)?;
    let licence = spdx.map(|spdx| Licence {
        spdx,
        commercial_use: Tri::parse(&r.get::<_, String>(9).unwrap_or_default()),
        modification: Tri::parse(&r.get::<_, String>(10).unwrap_or_default()),
        redistribution: Tri::parse(&r.get::<_, String>(11).unwrap_or_default()),
        attribution_required: r.get::<_, i64>(12).unwrap_or(0) != 0,
        share_alike: r.get::<_, i64>(13).unwrap_or(0) != 0,
    });
    let status = match r.get::<_, Option<String>>(14)?.as_deref() {
        Some("confirmed") => Status::Confirmed {
            by: r.get::<_, String>(15)?,
            at: r.get::<_, i64>(16)?,
            note: r.get::<_, String>(17)?,
        },
        Some("guessed") => Status::Guessed,
        _ => Status::Unknown,
    };
    Ok(Asset {
        id: r.get(0)?,
        hash: r.get(1)?,
        size: r.get::<_, i64>(2)? as u64,
        kind: Kind::parse(&r.get::<_, String>(3)?),
        name: r.get(4)?,
        ext: r.get(5)?,
        stored: r.get(6)?,
        created: r.get(7)?,
        licence,
        status,
        author: r.get::<_, Option<String>>(18)?.unwrap_or_default(),
        source_url: r.get::<_, Option<String>>(19)?.unwrap_or_default(),
        attribution_text: r.get::<_, Option<String>>(20)?.unwrap_or_default(),
        tags: Vec::new(),
    })
}

fn mtime_ns(m: &std::fs::Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

fn valid_date(d: &str) -> bool {
    let b = d.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && d[..4].parse::<u32>().is_ok()
        && d[5..7].parse::<u32>().is_ok_and(|m| (1..=12).contains(&m))
        && d[8..].parse::<u32>().is_ok_and(|x| (1..=31).contains(&x))
}

/// Lowercase, words joined by hyphens: `Sci Fi` and `sci-fi` are one tag.
/// `:` keeps a namespace and `/` a hierarchy (`genre:electronic/dnb`).
pub fn normalise_tag(t: &str) -> String {
    t.split(|c: char| !(c.is_alphanumeric() || c == ':' || c == '/' || c == '_'))
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

/// Turns what a person typed into a safe FTS5 query: every word a quoted
/// prefix term, so punctuation can never be read as query syntax.
fn fts_query(q: &str) -> Option<String> {
    let terms: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"*", w.replace('"', "")))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(hash: &str, name: &str, kind: Kind) -> NewAsset {
        NewAsset { hash: hash.into(), size: 10, kind, name: name.into(), ext: "png".into(), stored: format!("ab/{hash}.png") }
    }

    #[test]
    fn same_hash_is_one_asset() {
        let mut c = Catalog::in_memory().unwrap();
        let (a, fresh) = c.add_asset(&new("h1", "hero.png", Kind::Image)).unwrap();
        let (b, fresh2) = c.add_asset(&new("h1", "copy-of-hero.png", Kind::Image)).unwrap();
        assert!(fresh && !fresh2);
        assert_eq!(a, b);
    }

    #[test]
    fn unconfirmed_is_not_usable_until_confirmed() {
        let mut c = Catalog::in_memory().unwrap();
        let (id, _) = c.add_asset(&new("h1", "hero.png", Kind::Image)).unwrap();
        assert!(!c.get(id).unwrap().unwrap().is_usable());
        c.set_licence(id, &Licence::from_spdx("CC0-1.0"), true).unwrap();
        let a = c.get(id).unwrap().unwrap();
        assert_eq!(a.status, Status::Guessed);
        assert!(!a.is_usable());
        assert!(c.confirm_licence(id, &Licence::from_spdx("CC0-1.0"), " ", "").is_err());
        c.confirm_licence(id, &Licence::from_spdx("CC0-1.0"), "me", "checked the page").unwrap();
        assert!(c.get(id).unwrap().unwrap().is_usable());
    }

    #[test]
    fn search_finds_by_name_tag_licence_and_filters() {
        let mut c = Catalog::in_memory().unwrap();
        let (a, _) = c.add_asset(&new("h1", "dragon-sprite.png", Kind::Image)).unwrap();
        let (b, _) = c.add_asset(&new("h2", "boom.ogg", Kind::Audio)).unwrap();
        c.add_tag(a, "Sci Fi").unwrap();
        c.add_tag(a, "boss").unwrap();
        c.confirm_licence(a, &Licence::from_spdx("CC-BY-4.0"), "me", "").unwrap();
        c.set_licence(b, &Licence::from_spdx("CC-BY-NC-4.0"), true).unwrap();
        let all = Filter::default();
        assert_eq!(c.search("drag", &all).unwrap().len(), 1);
        assert_eq!(c.search("sci-fi", &all).unwrap().len(), 1);
        assert_eq!(c.search("CC-BY", &all).unwrap().len(), 2);
        assert_eq!(c.search("", &all).unwrap().len(), 2);
        assert_eq!(c.search("\"quoted\" OR (", &all).unwrap().len(), 0);
        assert_eq!(c.search("", &Filter { kind: Some(Kind::Audio), ..Default::default() }).unwrap()[0].id, b);
        assert_eq!(c.search("", &Filter { confirmed_only: true, ..Default::default() }).unwrap().len(), 1);
        assert_eq!(c.search("", &Filter { commercial_only: true, ..Default::default() }).unwrap().len(), 1);
        c.remove_tag(a, "boss").unwrap();
        assert!(c.search("boss", &all).unwrap().is_empty());
        assert_eq!(c.get(a).unwrap().unwrap().tags, vec!["sci-fi"]);
    }

    #[test]
    fn uses_origins_and_credit() {
        let mut c = Catalog::in_memory().unwrap();
        let (id, _) = c.add_asset(&new("h1", "x.png", Kind::Image)).unwrap();
        assert!(c.set_credit(id, "A", "u", "t").is_err());
        c.set_licence(id, &Licence::from_spdx("MIT"), false).unwrap();
        c.set_credit(id, "Ada", "https://x", "by Ada").unwrap();
        c.record_use(id, "game", "img/x.png").unwrap();
        c.record_use(id, "game", "img/x.png").unwrap();
        c.record_origin(id, "generator", "painter:item sword seed=7").unwrap();
        assert_eq!(c.uses(id).unwrap().len(), 1);
        assert_eq!(c.origins(id).unwrap()[0].0, "generator");
        assert_eq!(c.search("ada", &Filter::default()).unwrap().len(), 1);
    }

    #[test]
    fn hash_cache_rereads_only_changed_files() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("f.bin");
        std::fs::write(&p, b"one").unwrap();
        let mut c = Catalog::in_memory().unwrap();
        let h1 = c.hash_cached(&p).unwrap();
        // Same size and mtime: the cached answer is used even if we lie about content on disk.
        let mtime = std::fs::metadata(&p).unwrap().modified().unwrap();
        std::fs::write(&p, b"two").unwrap();
        std::fs::File::options().write(true).open(&p).unwrap().set_modified(mtime).unwrap();
        assert_eq!(c.hash_cached(&p).unwrap(), h1);
        // A different size always forces a re-read.
        std::fs::write(&p, b"three").unwrap();
        assert_ne!(c.hash_cached(&p).unwrap(), h1);
        std::fs::remove_file(&p).unwrap();
        assert_eq!(c.prune_hash_cache().unwrap(), 1);
    }

    #[test]
    fn hierarchy_bpm_duration_filters_facets() {
        use crate::audio::AudioInfo;
        let mut c = Catalog::in_memory().unwrap();
        let (a, _) = c.add_asset(&new("h1", "MUS_Forest_Explore_96.wav", Kind::Audio)).unwrap();
        let (b, _) = c.add_asset(&new("h2", "MUS_Cave_Fight_150.wav", Kind::Audio)).unwrap();
        for t in ["game:area/forest", "bpm:96", "genre:ambient/dark"] { c.add_tag(a, t).unwrap(); }
        for t in ["game:area/cave", "bpm:150"] { c.add_tag(b, t).unwrap(); }
        let info = |ms| AudioInfo { codec: "pcm", container: "wav", sample_rate: 44100, channels: 2, bit_depth: Some(16), duration_ms: ms, lossless: true };
        c.set_audio(a, &info(30_000), Some(Levels { peak_dbfs: -1.0, rms_dbfs: -14.0 })).unwrap();
        c.set_audio(b, &info(90_000), None).unwrap();
        let f = |f: Filter| c.search("", &f).unwrap().into_iter().map(|a| a.id).collect::<Vec<_>>();
        assert_eq!(f(Filter { tag: Some("game:area".into()), ..Default::default() }), vec![b, a]); // by name
        assert_eq!(f(Filter { tag: Some("game:area/forest".into()), ..Default::default() }), vec![a]);
        assert_eq!(f(Filter { tag: Some("game:are".into()), ..Default::default() }), Vec::<i64>::new());
        assert_eq!(f(Filter { bpm: Some((90, 100)), ..Default::default() }), vec![a]);
        assert_eq!(f(Filter { bpm: Some((100, 200)), ..Default::default() }), vec![b]);
        assert_eq!(f(Filter { duration_ms: Some((60_000, 120_000)), ..Default::default() }), vec![b]);
        assert_eq!(c.audio(a).unwrap().unwrap().1.unwrap().peak_dbfs, -1.0);
        assert!(c.audio(b).unwrap().unwrap().1.is_none());
        let facets = c.facet_counts("game").unwrap();
        assert_eq!(facets.len(), 2);
        assert_eq!(c.facet_counts("bpm").unwrap().len(), 2);
    }

    #[test]
    fn rights_and_expiry_alerts() {
        let mut c = Catalog::in_memory().unwrap();
        let (a, _) = c.add_asset(&new("h1", "a.ogg", Kind::Audio)).unwrap();
        let (b, _) = c.add_asset(&new("h2", "b.ogg", Kind::Audio)).unwrap();
        assert_eq!(c.rights(a).unwrap().ai_involvement, AiInvolvement::None);
        let mut r = c.rights(a).unwrap();
        r.expires = "2026-12-01".into();
        r.ai_involvement = AiInvolvement::Assisted;
        r.licensor = "Stock Co".into();
        c.set_rights(a, &r).unwrap();
        r.expires = "2027-06-01".into();
        c.set_rights(b, &r).unwrap();
        assert_eq!(c.rights(a).unwrap().licensor, "Stock Co");
        assert_eq!(c.expiring_by("2026-12-31").unwrap(), vec![a]);
        assert_eq!(c.expiring_by("2027-12-31").unwrap(), vec![a, b]);
        r.expires = "next week".into();
        assert!(c.set_rights(a, &r).is_err());
    }

    #[test]
    fn tag_normalisation() {
        assert_eq!(normalise_tag("  Sci  Fi! "), "sci-fi");
        assert_eq!(normalise_tag("genre:Pixel Art"), "genre:pixel-art");
        assert_eq!(normalise_tag("Genre:Electronic/Drum and Bass"), "genre:electronic/drum-and-bass");
    }
}
