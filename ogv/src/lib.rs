//! Ogg Theora video for the file preview: [`probe`] reads a file's facts
//! without decoding it, and [`Decoder`] decodes its frames to RGBA. The
//! decoding is libogg and libtheora's decoder (`vendor/xiph`, BSD-3-Clause)
//! behind a small C shim; the probe is plain Rust over the Ogg pages.
//!
//! Only Theora video is decoded. The Vorbis track is described (channels,
//! rate, length) but not played here; the Ogg audio decoder does that.

use std::fmt;
use std::io::Read;

#[derive(Debug)]
pub enum Error {
    /// Not an Ogg file with a Theora video stream.
    NotTheora,
    /// The stream is damaged or uses something unsupported.
    Bad(&'static str),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotTheora => f.write_str("not an Ogg Theora video"),
            Error::Bad(w) => write!(f, "damaged or unsupported Theora video: {w}"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;

/// The video stream's facts.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    /// From the last page's position; None when the file has none (cut short).
    pub frames: Option<u64>,
    pub duration: Option<f64>,
}

/// The Vorbis track, when the file has one.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioInfo {
    pub channels: u8,
    pub rate: u32,
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub video: VideoInfo,
    pub audio: Option<AudioInfo>,
}

struct Page<'a> {
    flags: u8,
    granule: i64,
    serial: u32,
    /// The page's first packet's bytes (enough to name the codec).
    first: &'a [u8],
}

/// Walks the Ogg pages of `data`; stops quietly at the first byte that is not
/// a page (a cut-off file still probes).
fn pages(data: &[u8]) -> impl Iterator<Item = Page<'_>> {
    let mut at = 0usize;
    std::iter::from_fn(move || {
        let d = data.get(at..)?;
        if d.len() < 27 || &d[..4] != b"OggS" {
            return None;
        }
        let nseg = d[26] as usize;
        let table = d.get(27..27 + nseg)?;
        let body: usize = table.iter().map(|&b| b as usize).sum();
        let start = 27 + nseg;
        if d.len() < start + body {
            return None;
        }
        let page = Page {
            flags: d[5],
            granule: i64::from_le_bytes(d[6..14].try_into().unwrap()),
            serial: u32::from_le_bytes(d[14..18].try_into().unwrap()),
            first: &d[start..start + body.min(64)],
        };
        at += start + body;
        Some(page)
    })
}

/// Reads a file's facts from its Ogg pages, without decoding any video.
pub fn probe(data: &[u8]) -> Result<Info> {
    struct Th {
        serial: u32,
        width: u32,
        height: u32,
        fps_n: u32,
        fps_d: u32,
        shift: u32,
        last: i64,
    }
    struct Vo {
        serial: u32,
        channels: u8,
        rate: u32,
        last: i64,
    }
    let (mut th, mut vo): (Option<Th>, Option<Vo>) = (None, None);
    for p in pages(data) {
        if p.flags & 2 != 0 {
            // A beginning-of-stream page: name the codec from its first bytes.
            let f = p.first;
            if th.is_none() && f.len() >= 42 && f.starts_with(b"\x80theora") {
                let be24 = |o: usize| u32::from_be_bytes([0, f[o], f[o + 1], f[o + 2]]);
                let be32 = |o: usize| u32::from_be_bytes([f[o], f[o + 1], f[o + 2], f[o + 3]]);
                let v = u16::from_be_bytes([f[40], f[41]]);
                th = Some(Th { serial: p.serial, width: be24(14), height: be24(17), fps_n: be32(22), fps_d: be32(26), shift: ((v >> 5) & 0x1f) as u32, last: -1 });
            } else if vo.is_none() && f.len() >= 16 && f.starts_with(b"\x01vorbis") {
                vo = Some(Vo { serial: p.serial, channels: f[11], rate: u32::from_le_bytes([f[12], f[13], f[14], f[15]]), last: -1 });
            }
        }
        if let Some(t) = th.as_mut().filter(|t| t.serial == p.serial) {
            t.last = t.last.max(p.granule);
        }
        if let Some(v) = vo.as_mut().filter(|v| v.serial == p.serial) {
            v.last = v.last.max(p.granule);
        }
    }
    let t = th.ok_or(Error::NotTheora)?;
    if t.width == 0 || t.height == 0 || t.fps_n == 0 || t.fps_d == 0 {
        return Err(Error::Bad("the picture size or frame rate is zero"));
    }
    let fps = t.fps_n as f64 / t.fps_d as f64;
    // A Theora position is (keyframe number << shift) + frames since it; the
    // sum is the count of frames shown so far.
    let frames = (t.last >= 0).then(|| ((t.last as u64) >> t.shift) + (t.last as u64 & ((1u64 << t.shift) - 1)));
    Ok(Info {
        video: VideoInfo { width: t.width, height: t.height, fps, frames, duration: frames.map(|n| n as f64 / fps) },
        audio: vo.map(|v| AudioInfo { channels: v.channels, rate: v.rate, duration: (v.last >= 0 && v.rate > 0).then(|| v.last as f64 / v.rate as f64) }),
    })
}

#[allow(non_camel_case_types)]
type jr_ogv = std::ffi::c_void;

extern "C" {
    fn jr_ogv_new() -> *mut jr_ogv;
    fn jr_ogv_free(h: *mut jr_ogv);
    fn jr_ogv_feed(h: *mut jr_ogv, data: *const u8, n: usize) -> i32;
    fn jr_ogv_next(h: *mut jr_ogv) -> i32;
    fn jr_ogv_set_eof(h: *mut jr_ogv);
    fn jr_ogv_width(h: *const jr_ogv) -> u32;
    fn jr_ogv_height(h: *const jr_ogv) -> u32;
    fn jr_ogv_rgba(h: *const jr_ogv) -> *const u8;
    fn jr_ogv_rgba_len(h: *const jr_ogv) -> usize;
    fn jr_ogv_time(h: *const jr_ogv) -> f64;
    fn jr_ogv_index(h: *const jr_ogv) -> i64;
}

/// One decoded picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, straight alpha (always opaque).
    pub rgba: Vec<u8>,
    /// Seconds from the start.
    pub time: f64,
    pub index: u64,
}

/// Decodes the Theora frames of an Ogg stream one at a time, reading as it
/// goes (so a big file is never held whole).
pub struct Decoder<R: Read> {
    src: R,
    h: *mut jr_ogv,
    done: bool,
}

// The handle is used from one thread at a time (&mut self), and owns its memory.
unsafe impl<R: Read + Send> Send for Decoder<R> {}

const CHUNK: usize = 64 * 1024;

impl<R: Read> Decoder<R> {
    pub fn new(src: R) -> Result<Self> {
        let h = unsafe { jr_ogv_new() };
        if h.is_null() {
            return Err(Error::Bad("out of memory"));
        }
        Ok(Decoder { src, h, done: false })
    }

    /// The next frame, or None at the end of the video.
    pub fn next_frame(&mut self) -> Result<Option<Frame>> {
        if self.done {
            return Ok(None);
        }
        let mut buf = vec![0u8; 0];
        loop {
            match unsafe { jr_ogv_next(self.h) } {
                1 => {
                    let (w, h) = unsafe { (jr_ogv_width(self.h), jr_ogv_height(self.h)) };
                    let (p, n) = unsafe { (jr_ogv_rgba(self.h), jr_ogv_rgba_len(self.h)) };
                    if p.is_null() || n != w as usize * h as usize * 4 {
                        return Err(Error::Bad("no picture"));
                    }
                    let rgba = unsafe { std::slice::from_raw_parts(p, n) }.to_vec();
                    let (time, index) = unsafe { (jr_ogv_time(self.h), jr_ogv_index(self.h)) };
                    return Ok(Some(Frame { width: w, height: h, rgba, time, index: index.max(0) as u64 }));
                }
                0 => {
                    buf.resize(CHUNK, 0);
                    let n = self.src.read(&mut buf)?;
                    if n == 0 {
                        unsafe { jr_ogv_set_eof(self.h) };
                    } else if unsafe { jr_ogv_feed(self.h, buf.as_ptr(), n) } != 0 {
                        return Err(Error::Bad("could not read the Ogg stream"));
                    }
                }
                2 => {
                    self.done = true;
                    return Ok(None);
                }
                -2 => return Err(Error::NotTheora),
                -3 => return Err(Error::Bad("the stream's setup header is not usable")),
                -4 => return Err(Error::Bad("a video packet would not decode")),
                _ => return Err(Error::Bad("the decoder failed")),
            }
        }
    }
}

impl<R: Read> Iterator for Decoder<R> {
    type Item = Result<Frame>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_frame() {
            Ok(Some(f)) => Some(Ok(f)),
            Ok(None) => None,
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

impl<R: Read> Drop for Decoder<R> {
    fn drop(&mut self) {
        unsafe { jr_ogv_free(self.h) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BARS: &[u8] = include_bytes!("../tests/bars.ogv");

    #[test]
    fn probes_the_video_and_the_audio() {
        let i = probe(BARS).unwrap();
        assert_eq!((i.video.width, i.video.height), (64, 48));
        assert_eq!(i.video.fps, 10.0);
        assert_eq!(i.video.frames, Some(15));
        assert!((i.video.duration.unwrap() - 1.5).abs() < 1e-9);
        let a = i.audio.unwrap();
        assert_eq!((a.channels, a.rate), (2, 44100));
        assert!((a.duration.unwrap() - 1.5).abs() < 0.01);
    }

    #[test]
    fn decodes_every_frame_in_order() {
        let frames: Vec<Frame> = Decoder::new(BARS).unwrap().collect::<Result<_>>().unwrap();
        assert_eq!(frames.len(), 15);
        for (i, f) in frames.iter().enumerate() {
            assert_eq!((f.width, f.height, f.rgba.len()), (64, 48, 64 * 48 * 4));
            assert_eq!(f.index, i as u64);
            assert!((f.time - i as f64 / 10.0).abs() < 1e-6, "{} {}", f.time, i);
            assert!(f.rgba.chunks(4).all(|p| p[3] == 255));
        }
    }

    #[test]
    fn draws_the_colour_bars() {
        let f = Decoder::new(BARS).unwrap().next_frame().unwrap().unwrap();
        let px = |x: usize, y: usize| {
            let o = (y * 64 + x) * 4;
            [f.rgba[o] as i32, f.rgba[o + 1] as i32, f.rgba[o + 2] as i32]
        };
        let near = |a: [i32; 3], b: [i32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 8);
        // The source's top bars, checked against ffmpeg's own Theora decode.
        assert!(near(px(4, 4), [190, 191, 189]), "{:?}", px(4, 4));
        assert!(near(px(14, 4), [191, 191, 0]), "{:?}", px(14, 4));
        assert!(near(px(23, 4), [3, 190, 189]), "{:?}", px(23, 4));
        assert!(near(px(32, 4), [1, 189, 0]), "{:?}", px(32, 4));
        assert!(near(px(41, 4), [194, 0, 188]), "{:?}", px(41, 4));
        assert!(near(px(50, 4), [190, 0, 0]), "{:?}", px(50, 4));
        assert!(near(px(59, 4), [190, 0, 3]), "{:?}", px(59, 4));
    }

    /// A reader that gives one byte at a time: the decoder must not need whole reads.
    struct Dribble<'a>(&'a [u8]);
    impl Read for Dribble<'_> {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            if self.0.is_empty() || b.is_empty() {
                return Ok(0);
            }
            b[0] = self.0[0];
            self.0 = &self.0[1..];
            Ok(1)
        }
    }

    #[test]
    fn decodes_from_a_slow_reader() {
        let n = Decoder::new(Dribble(BARS)).unwrap().filter(|f| f.is_ok()).count();
        assert_eq!(n, 15);
    }

    #[test]
    fn refuses_what_is_not_theora() {
        assert!(matches!(probe(b"RIFF....WAVE"), Err(Error::NotTheora)));
        assert!(probe(&BARS[..40]).is_err());
        // A cut-off file still probes, and decodes the frames it has.
        let cut = &BARS[..BARS.len() / 2];
        assert!(probe(cut).unwrap().video.frames.unwrap() < 15);
        assert!(Decoder::new(cut).unwrap().filter_map(|f| f.ok()).count() < 15);
        let mut d = Decoder::new(&b"not an ogg file at all"[..]).unwrap();
        assert!(d.next_frame().unwrap().is_none());
    }
}
