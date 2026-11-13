import { useEffect, useRef, useState } from "react";
import { previewVideoFrames, type Preview as PreviewData, type VideoFrame } from "../lib/backend";

const clock = (t: number) => `${Math.floor(t / 60)}:${(t % 60).toFixed(1).padStart(4, "0")}`;

/** The selected file, shown by what it is: facts first, then the body. Sound
 *  and video have their own text-mode buttons; nothing here edits the file. */
export default function Preview(props: { data: PreviewData; onClose: () => void }) {
  const { data } = props;
  const name = data.path.split("/").pop();
  const b = data.body;
  return (
    <section className="preview" aria-label={`Preview of ${data.path}`} onKeyDown={(e) => e.key === "Escape" && props.onClose()}>
      <div className="preview-head">
        <h2>
          {name} <span className="preview-kind">({data.kind})</span>
        </h2>
        <button onClick={props.onClose} aria-label="Close preview" title="Close preview (Escape)">
          Close
        </button>
      </div>
      <dl className="preview-facts">
        {data.facts.map((f) => (
          <div key={f.label} className="preview-fact">
            <dt>{f.label}</dt>
            <dd>{f.value}</dd>
          </div>
        ))}
      </dl>
      {data.note && <p className="preview-note" role="status">{data.note}</p>}
      {b.type === "image" && (
        <div className="preview-stage">
          <img src={b.dataUrl} alt={`${name}, ${b.width} by ${b.height}`} className={b.pixelArt ? "pixelated" : ""} />
          {b.palette.length > 0 && (
            <p className="preview-palette">
              Palette:{" "}
              {b.palette.map((c) => (
                <span key={c}>
                  <span className="preview-swatch" style={{ background: c }} aria-hidden="true" /> {c}{" "}
                </span>
              ))}
            </p>
          )}
        </div>
      )}
      {b.type === "audio" && <AudioPlayer key={data.path} url={b.dataUrl} seconds={b.seconds} waveform={b.waveform} />}
      {b.type === "video" && b.source === "ogv" && <OgvPlayer key={data.path} path={data.path} width={b.width} height={b.height} poster={b.poster} />}
      {b.type === "video" && b.source === "web" && <WebVideo key={data.path} url={b.dataUrl} />}
      {b.type === "font" && <FontSample key={data.path} url={b.dataUrl} />}
      {b.type === "members" && (
        <ul className="preview-members" aria-label="Members">
          {b.names.map((m) => (
            <li key={m.name}>
              {m.name}
              {m.name.endsWith("/") ? "" : `  (${m.size} bytes)`}
            </li>
          ))}
          {b.more > 0 && <li>… and {b.more} more</li>}
        </ul>
      )}
      {b.type === "hex" && <pre className="preview-hex" aria-label="First bytes">{b.dump}</pre>}
    </section>
  );
}

/** Peak levels as four shades of block, one cell per bin (all in the VGA font). */
function wave(w: number[]) {
  return w.map((v) => " ░▒▓█"[Math.min(4, Math.ceil(v / 64))]).join("");
}

function AudioPlayer(props: { url: string | null; seconds: number | null; waveform: number[] | null }) {
  const el = useRef<HTMLAudioElement | null>(null);
  const [playing, setPlaying] = useState(false);
  const [at, setAt] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    if (!props.url) return;
    const a = new Audio(props.url);
    el.current = a;
    a.ontimeupdate = () => setAt(a.currentTime);
    a.onended = () => setPlaying(false);
    a.onerror = () => {
      setPlaying(false);
      setErr("This system cannot play this kind of sound here.");
    };
    return () => {
      a.pause();
      a.src = "";
      el.current = null;
    };
  }, [props.url]);
  if (!props.url) return null;
  const toggle = () => {
    const a = el.current;
    if (!a) return;
    if (a.paused) {
      setErr(null);
      a.play().then(() => setPlaying(true), () => setErr("This system cannot play this kind of sound here."));
    } else {
      a.pause();
      setPlaying(false);
    }
  };
  const stop = () => {
    const a = el.current;
    if (!a) return;
    a.pause();
    a.currentTime = 0;
    setAt(0);
    setPlaying(false);
  };
  return (
    <div className="preview-stage">
      <div className="row-buttons">
        <button onClick={toggle} aria-pressed={playing}>{playing ? "Pause" : "Play"}</button>
        <button onClick={stop}>Stop</button>
        <span role="timer" aria-label="Position">
          {clock(at)}
          {props.seconds != null ? ` / ${clock(props.seconds)}` : ""}
        </span>
      </div>
      {props.waveform && <p className="preview-wave" aria-hidden="true">{wave(props.waveform)}</p>}
      {err && <p className="preview-note" role="alert">{err}</p>}
    </div>
  );
}

function WebVideo(props: { url: string | null }) {
  const el = useRef<HTMLVideoElement | null>(null);
  const [playing, setPlaying] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  if (!props.url) return null;
  const toggle = () => {
    const v = el.current;
    if (!v) return;
    if (v.paused) v.play().then(() => setPlaying(true), () => setErr("This system cannot play this video here."));
    else {
      v.pause();
      setPlaying(false);
    }
  };
  return (
    <div className="preview-stage">
      <div className="row-buttons">
        <button onClick={toggle} aria-pressed={playing}>{playing ? "Pause" : "Play"}</button>
      </div>
      <video ref={el} src={props.url} onEnded={() => setPlaying(false)} onError={() => setErr("This system cannot play this video here.")} aria-label="Video" />
      {err && <p className="preview-note" role="alert">{err}</p>}
    </div>
  );
}

const AHEAD = 12; // frames kept decoded ahead of the picture
const BATCH = 8;

/** Plays an Ogg Theora video: frames come in batches from the backend and are
 *  drawn on time by the clock. Theora's sound track is not played here. */
function OgvPlayer(props: { path: string; width: number; height: number; poster: string | null }) {
  const canvas = useRef<HTMLCanvasElement | null>(null);
  const run = useRef({ buf: [] as { f: VideoFrame; img: HTMLImageElement }[], next: 0, end: false, fetching: false, t0: 0, last: 0, raf: 0, playing: false, alive: true, shown: -1 });
  const [playing, setPlaying] = useState(false);
  const [at, setAt] = useState(0);
  const [err, setErr] = useState<string | null>(null);

  const draw = (img: HTMLImageElement) => {
    const c = canvas.current;
    const g = c?.getContext("2d");
    if (c && g) g.drawImage(img, 0, 0, c.width, c.height);
  };

  useEffect(() => {
    const r = run.current;
    r.alive = true;
    if (props.poster) {
      const img = new Image();
      img.onload = () => r.alive && r.shown < 0 && draw(img);
      img.src = props.poster;
    }
    return () => {
      r.alive = false;
      cancelAnimationFrame(r.raf);
    };
  }, [props.poster]);

  const fetchMore = () => {
    const r = run.current;
    if (r.fetching || r.end || !r.alive) return;
    r.fetching = true;
    previewVideoFrames(props.path, r.next, BATCH)
      .then(async (res) => {
        for (const f of res.frames) {
          const img = new Image();
          img.src = f.jpeg;
          await img.decode();
          if (!r.alive) return;
          r.buf.push({ f, img });
          r.next = f.index + 1;
        }
        if (res.end) r.end = true;
      })
      .catch((e) => {
        setErr(String(e));
        r.end = true;
      })
      .finally(() => {
        r.fetching = false;
      });
  };

  const tick = (now: number) => {
    const r = run.current;
    if (!r.alive || !r.playing) return;
    if (r.buf.length < AHEAD) fetchMore();
    if (r.buf.length === 0) {
      if (r.end) {
        r.playing = false;
        setPlaying(false);
        return;
      }
      r.t0 += now - r.last; // starved: the clock waits for the next frame
    } else {
      const t = (now - r.t0) / 1000;
      let show: (typeof r.buf)[number] | null = null;
      while (r.buf.length > 0 && r.buf[0].f.time <= t) show = r.buf.shift()!;
      if (show) {
        draw(show.img);
        r.shown = show.f.index;
        setAt(show.f.time);
      }
      if (r.buf.length === 0 && r.end) {
        r.playing = false;
        setPlaying(false);
        return;
      }
    }
    r.last = now;
    r.raf = requestAnimationFrame(tick);
  };

  const play = () => {
    const r = run.current;
    if (r.playing) {
      r.playing = false;
      cancelAnimationFrame(r.raf);
      setPlaying(false);
      return;
    }
    if (r.end && r.buf.length === 0) {
      // Played to the end: start again.
      r.next = 0;
      r.end = false;
      r.shown = -1;
      r.t0 = performance.now();
      setAt(0);
    } else {
      r.t0 = performance.now() - at * 1000;
    }
    r.playing = true;
    r.last = performance.now();
    setPlaying(true);
    fetchMore();
    r.raf = requestAnimationFrame(tick);
  };

  const restart = () => {
    const r = run.current;
    cancelAnimationFrame(r.raf);
    r.playing = false;
    r.buf = [];
    r.next = 0;
    r.end = false;
    r.shown = -1;
    setPlaying(false);
    setAt(0);
  };

  return (
    <div className="preview-stage">
      <div className="row-buttons">
        <button onClick={play} aria-pressed={playing}>{playing ? "Pause" : "Play"}</button>
        <button onClick={restart}>Rewind</button>
        <span role="timer" aria-label="Position">{clock(at)}</span>
      </div>
      <canvas ref={canvas} width={props.width} height={props.height} role="img" aria-label="Video picture" />
      {err && <p className="preview-note" role="alert">{err}</p>}
    </div>
  );
}

export function FontSample(props: { url: string }) {
  const [family, setFamily] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    const name = "jr-preview-" + Math.random().toString(36).slice(2);
    const face = new FontFace(name, `url(${props.url})`);
    let gone = false;
    face.load().then(
      () => {
        if (gone) return;
        document.fonts.add(face);
        setFamily(name);
      },
      () => !gone && setErr("This font could not be loaded."),
    );
    return () => {
      gone = true;
      document.fonts.delete(face);
    };
  }, [props.url]);
  if (err) return <p className="preview-note" role="alert">{err}</p>;
  if (!family) return <p className="preview-note" role="status">Loading the font…</p>;
  return (
    <div className="preview-stage preview-font" style={{ fontFamily: `"${family}"` }} aria-label="Font sample">
      <p>The quick brown fox jumps over the lazy dog.</p>
      <p>THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG</p>
      <p>0123456789 !?&amp;%$#@ ()[]{} +-*/=</p>
    </div>
  );
}
