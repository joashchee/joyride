// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// The NSF nsc links (`MusicFile::make_bin`, nsc 1.31, S.W., BSD-2): NSD.Lib's driver ROM (`rom/`, assembled from
// NSD.Lib's source by `synth/scripts/nsd-reference.sh`), its song table, the music data and the DPCM samples; and
// the metadata nsc puts after it, or the NSFe it makes instead (`MusicHeader`, `MetaItem`, `Meta_*`,
// `MusicFile::saveNSF` and `saveNSFe`).

use super::Output;

/// NSD.Lib's 15 driver builds, by the name `#Code` gives them.
pub(crate) const ROMS: [(&str, &[u8]); 15] = [
    ("nsd", include_bytes!("rom/nsd.bin")),
    ("nsd_pal", include_bytes!("rom/nsd_pal.bin")),
    ("nsd_se", include_bytes!("rom/nsd_se.bin")),
    ("nsd_fds", include_bytes!("rom/nsd_fds.bin")),
    ("nsd_vrc6", include_bytes!("rom/nsd_vrc6.bin")),
    ("nsd_vrc7", include_bytes!("rom/nsd_vrc7.bin")),
    ("nsd_opll", include_bytes!("rom/nsd_opll.bin")),
    ("nsd_mmc5", include_bytes!("rom/nsd_mmc5.bin")),
    ("nsd_n163", include_bytes!("rom/nsd_n163.bin")),
    ("nsd_s5b", include_bytes!("rom/nsd_s5b.bin")),
    ("nsd_all", include_bytes!("rom/nsd_all.bin")),
    ("nsd_all2", include_bytes!("rom/nsd_all2.bin")),
    ("nsd_oall", include_bytes!("rom/nsd_oall.bin")),
    ("nsd_hfe4", include_bytes!("rom/nsd_hfe4.bin")),
    ("nsd_hfx4", include_bytes!("rom/nsd_hfx4.bin")),
];

/// The driver ROM a `#Code` name means: its file name, any folder, any case, `.bin` or not.
pub fn rom(code: &[u8]) -> Option<&'static [u8]> {
    let name = String::from_utf8_lossy(code).to_ascii_lowercase();
    let name = name.rsplit(['/', '\\']).next().unwrap_or("");
    let name = name.strip_suffix(".bin").unwrap_or(name);
    ROMS.iter().find(|(n, _)| *n == name).map(|(_, r)| *r)
}

/// The NSF (version 2, its metadata after the program) nsc's `-n` makes of compiled music.
pub fn nsf(o: &Output) -> Result<Vec<u8>, String> {
    let (mut hed, data) = make_bin(o)?;
    let meta = metadata(o, false);
    // Only NEND: no metadata.
    let size: usize = meta.iter().map(|(_, d)| 8 + d.len()).sum();
    if size > 8 {
        hed[0x7c] |= 0x80;
    }
    hed.extend_from_slice(&data);
    if size > 8 {
        write_chunks(&mut hed, &meta);
    }
    Ok(hed)
}

/// The NSFe nsc's `-x` makes of compiled music.
pub fn nsfe(o: &Output) -> Result<Vec<u8>, String> {
    let (hed, data) = make_bin(o)?;
    let mut info = hed[0x08..0x0e].to_vec();
    info.extend_from_slice(&[hed[0x7a], hed[0x7b], hed[6], hed[7]]);
    let mut chunks = vec![(*b"INFO", info), (*b"NSF2", vec![hed[0x7c]])];
    if hed[0x70..0x78].iter().any(|&b| b != 0) {
        chunks.push((*b"BANK", hed[0x70..0x78].to_vec()));
    }
    chunks.push((*b"DATA", data));
    chunks.extend(metadata(o, true));
    let mut f = b"NSFE".to_vec();
    write_chunks(&mut f, &chunks);
    Ok(f)
}

fn write_chunks(f: &mut Vec<u8>, chunks: &[([u8; 4], Vec<u8>)]) {
    for (id, d) in chunks {
        f.extend_from_slice(&(d.len() as u32).to_le_bytes());
        f.extend_from_slice(id);
        f.extend_from_slice(d);
    }
}

/// The metadata chunks after the program, in nsc's order: `#VRC7`, `#plst`, `#psfx` and `#mixe` as given; `text`;
/// `auth` (always in an NSFe, else when a name is over 32 bytes or there is a `#Maker`); `NEND`; then, after it as
/// nsc puts them, `tlbl`, `taut`, `time` and `fade` when any BGM or SE has one. nsc makes `fade` when a track has
/// a `#Time` (its `isFade` tests the time), so a `#Fade` alone makes none.
fn metadata(o: &Output, nsfe: bool) -> Vec<([u8; 4], Vec<u8>)> {
    let mut m = o.chunks.clone();
    let z = |s: &[u8]| {
        let mut v = s.to_vec();
        v.push(0);
        v
    };
    if !o.text.is_empty() {
        m.push((*b"text", z(&o.text)));
    }
    if o.title.len() > 32 || o.composer.len() > 32 || o.copyright.len() > 32 || !o.maker.is_empty() || nsfe {
        let mut a = Vec::new();
        for s in [&o.title, &o.composer, &o.copyright, &o.maker] {
            a.extend(z(s));
        }
        m.push((*b"auth", a));
    }
    m.push((*b"NEND", Vec::new()));
    let t = &o.tracks;
    if t.iter().any(|t| !t.label.is_empty()) {
        m.push((*b"tlbl", t.iter().flat_map(|t| z(&t.label)).collect()));
    }
    if t.iter().any(|t| !t.author.is_empty()) {
        m.push((*b"taut", t.iter().flat_map(|t| z(&t.author)).collect()));
    }
    if t.iter().any(|t| t.time >= 0) {
        m.push((*b"time", t.iter().flat_map(|t| t.time.to_le_bytes()).collect()));
        m.push((*b"fade", t.iter().flat_map(|t| t.fade.to_le_bytes()).collect()));
    }
    m
}

/// The NSF header and the program nsc's `make_bin` makes.
fn make_bin(o: &Output) -> Result<(Vec<u8>, Vec<u8>), String> {
    let Some(rom) = rom(&o.code) else {
        return Err(format!("the driver {} isn't one of NSD.Lib's", String::from_utf8_lossy(&o.code)));
    };
    let mut hed = rom[..0x80].to_vec();
    let code = &rom[0x80..];
    for (at, s) in [(0x0e, &o.title), (0x2e, &o.composer), (0x4e, &o.copyright)] {
        let n = s.len().min(32);
        hed[at..at + 32].fill(0);
        hed[at..at + n].copy_from_slice(&s[..n]);
    }
    hed[5] = 2;
    hed[6] = (o.bgm.len() + o.se.len()) as u8;
    if let Some(e) = o.external {
        hed[0x7b] = e;
    }
    let banked = hed[0x70..0x78].iter().any(|&b| b != 0);
    if banked != o.bank {
        return Err(if banked {
            "this driver banks its DPCM: the song needs #Bank"
        } else {
            "#Bank needs a driver that banks its DPCM"
        }
        .into());
    }
    if banked {
        hed[0x7c] |= 0x10;
    }
    // The song table: BGM and SE counts, the DPCM table's address, each BGM's and SE's.
    let table = 4 + (o.bgm.len() + o.se.len()) * 2;
    let base = if banked { table } else { 0x8000 + code.len() + table };
    let mut data = code.to_vec();
    data.push(o.bgm.len() as u8);
    data.push(o.se.len() as u8);
    let addr = |off: usize| ((base + off) as u16).to_le_bytes();
    data.extend_from_slice(&o.dpcm_info.map_or([0, 0], addr));
    for &b in o.bgm.iter().chain(&o.se) {
        data.extend_from_slice(&addr(b));
    }
    data.extend_from_slice(&o.music);
    let mus_size = if banked { data.len() - code.len() } else { data.len() };
    let limit = if banked { 0x10000 } else { o.offset_pcm - 0x8000 };
    if mus_size > limit {
        return Err("the music is too big for the NSF".into());
    }
    let mus_bank = mus_size.div_ceil(0x1000);
    if o.dpcm_info.is_some() {
        let to = if banked { ((mus_bank + 3) << 12) + (o.offset_pcm - 0xc000) } else { o.offset_pcm - 0x8000 };
        data.resize(to, 0);
    }
    data.extend_from_slice(&o.dpcm);
    let pcm_size = o.dpcm.len();
    let mut pcm_bank = (pcm_size + (o.offset_pcm & 0x0fff)) >> 12;
    if pcm_size & 0x0fff != 0 {
        pcm_bank += 1;
    }
    let size = if banked {
        if mus_bank + pcm_bank + 3 > 255 {
            return Err("the music and DPCM need more than 255 banks".into());
        }
        (((mus_bank + pcm_bank + 3) << 12) as usize).max(0x8000)
    } else {
        if o.offset_pcm + pcm_size > 0x10000 {
            return Err("the DPCM samples go past $FFFF".into());
        }
        data.len()
    };
    data.resize(size, 0);
    hed[0x7d] = size as u8;
    hed[0x7e] = (size >> 8) as u8;
    hed[0x7f] = (size >> 16) as u8;
    Ok((hed, data))
}
