// A small cue player for a page: it plays a cue sheet's baked audio (loops, layers as stems, stingers) and switches between cues on the
// music's own bars, using the manifest's switching rules (`nextSwitch`). Web Audio only, no DOM at import time: the context is passed in.
// The audio is baked ahead of time (`compose cues --audio`, or Neumetik's renders); the player decodes nothing and composes nothing.
//
//   const player = createCuePlayer(ctx, manifest, await loadCueAudio(ctx, manifest, '/music/'), { volume: 0.8 });
//   player.play('explore');                       // starts now; layers in `start` are up, the others silent
//   player.layers(['base', 'melody']);            // fade a stem in at the next bar
//   player.play('battle');                        // switches where the sheet's transition says (default: next phrase), through its stinger
//   player.stinger('pickup');                     // once, over whatever plays
import { nextSwitch, audioFiles } from './cues.js';

const LEAD = 0.03;   // seconds of notice the audio clock needs to land something exactly
const FADE = 0.04;   // the old music's fade-out at a switch
const ref = (r) => { const [id, section] = String(r).split(':'); return { id, section }; };
const isBuffer = (a) => typeof a?.duration === 'number';
const barSec = (e) => (e.beats * 60) / e.bpm;
const ignore = (f) => { try { f(); } catch { /* not started yet, or already stopped */ } };

/**
 * Fetch and decode the baked audio of every cue under `base` (a URL ending in "/"): `{ cueId: AudioBuffer | { layer: AudioBuffer } }`.
 * `formats` are the extensions to try for each file, best first (`['opus', 'wav']`): one that is missing or that the browser cannot decode
 * falls through to the next. Compressed files may decode a few milliseconds long; the player loops on the cue's exact length.
 */
export async function loadCueAudio(context, manifest, base = '', { fetch = globalThis.fetch, formats = ['wav'] } = {}) {
  const get = async (file) => {
    let last;
    for (const ext of formats) {
      const url = base + file.replace(/\.wav$/, `.${ext}`);
      try {
        const res = await fetch(url);
        if (!res.ok) throw new Error(`${url}: ${res.status}`);
        return await context.decodeAudioData(await res.arrayBuffer());
      } catch (e) { last = e; }
    }
    throw last;
  };
  const out = {};
  await Promise.all(manifest.cues.map(async (e) => {
    const f = audioFiles(e);
    if (f.mix) { out[e.id] = await get(f.mix); return; }
    const names = Object.keys(f.layers), bufs = await Promise.all(names.map((n) => get(f.layers[n])));
    out[e.id] = Object.fromEntries(names.map((n, i) => [n, bufs[i]]));
  }));
  return out;
}

/**
 * The player. `audio` maps a cue id to its AudioBuffer (a mix) or to `{ layer: AudioBuffer }` (stems of equal length). Options: `destination`
 * (default `context.destination`), `volume`, `fade` (seconds, at a switch) and `schedule` (default `setTimeout`, used only to tidy up
 * finished sources). Everything is placed on the audio clock, so switches land on the bar to the sample, not to a timer's accuracy.
 */
export function createCuePlayer(context, manifest, audio, { destination = context.destination, volume = 1, fade = FADE, schedule = (f, ms) => globalThis.setTimeout(f, ms) } = {}) {
  const by = new Map(manifest.cues.map((c) => [c.id, c])), master = context.createGain();
  master.gain.value = volume; master.connect(destination);
  let cur = null, pending = null;   // `cur` is what plays (or is about to); `pending` a switch not yet landed, with the music it replaces

  const entryOf = (id) => { const e = by.get(id); if (!e) throw new Error(`unknown cue "${id}"`); if (!audio[id]) throw new Error(`no audio for cue "${id}"`); return e; };
  const sectionBar = (e, name) => { if (!name) return 0; const s = e.sections.find((x) => x.name === name); if (!s) throw new Error(`cue "${e.id}" has no section "${name}"`); return s.fromBar; };

  /** Start a cue's sources at context time `when`, from `fromBar`; `on` are the layers that start up. */
  function launch(e, when, fromBar, on) {
    const bufs = audio[e.id], parts = isBuffer(bufs) ? { '': bufs } : bufs, out = context.createGain(), gains = {}, sources = [], level = {};
    out.gain.value = 1; out.connect(master);
    if (!isBuffer(bufs)) for (const n of Object.keys(e.layers)) if (!parts[n]) throw new Error(`cue "${e.id}" has no audio for layer "${n}"`);
    for (const [name, buf] of Object.entries(parts)) {
      const g = context.createGain(), src = context.createBufferSource();
      level[name] = name === '' || on.includes(name) ? 1 : 0;
      g.gain.value = level[name]; g.connect(out);
      src.buffer = buf; src.loop = e.kind === 'loop'; src.connect(g);
      if (src.loop) { src.loopStart = 0; src.loopEnd = e.bars * barSec(e); }   // the cue's exact length, whatever padding a compressed file decoded with
      src.start(when, fromBar * barSec(e));
      gains[name] = g; sources.push(src);
    }
    return { entry: e, startAt: when, fromBar, t0: when - fromBar * barSec(e), out, gains, level, sources, retired: false };
  }

  /** The context time a switch away from `m` lands at, no sooner than `from`: the next legal bar of the music playing. */
  function landing(m, from, at) {
    if (at === 'now') return from;
    const e = m.entry, bs = barSec(e), pos = Math.max(0, (from - m.t0) / bs), loops = Math.floor(pos / e.bars);
    return m.t0 + (loops * e.bars + nextSwitch(e, pos - loops * e.bars, at)) * bs;
  }

  /** Where `m` is at context time `t`, in bars into its loop (a cue not yet started is at the bar it starts from). */
  const barAt = (m, t) => (t < m.startAt ? m.fromBar : (t - m.t0) / barSec(m.entry)) % m.entry.bars;
  const sectionAt = (m, t) => { const b = barAt(m, t); return m.entry.sections.find((x) => b >= x.fromBar && b < x.toBar)?.name ?? null; };

  /** Fade `m` out from `when` and, once that has sounded, stop and drop it (`cancelPending` can still take that back before). */
  function retire(m, when) {
    m.out.gain.setValueAtTime(1, when);
    m.out.gain.linearRampToValueAtTime(0, when + fade);
    m.retired = true;
    schedule(() => { if (m.retired) discard(m); }, Math.max(0, (when + fade + 0.1 - context.currentTime) * 1000));
  }
  const discard = (m) => { m.sources.forEach((s) => ignore(() => s.stop())); ignore(() => m.out.disconnect()); };

  function sync() { if (pending && context.currentTime >= pending.when) { pending = null; } }
  function cancelPending() {
    if (!pending) return;
    const now = context.currentTime;
    discard(pending.next); ignore(() => pending.stinger?.stop());
    pending.old.out.gain.cancelScheduledValues(now); pending.old.out.gain.setValueAtTime(1, now); pending.old.retired = false;
    cur = pending.old; pending = null;
  }

  /** The transition the sheet gives for leaving `from` for `to`, if any. */
  function ruleFor(from, to) {
    return (manifest.transitions ?? []).find((t) => {
      const f = ref(t.from), g = ref(t.to);
      return f.id === from.entry.id && (!f.section || f.section === sectionAt(from, context.currentTime)) && g.id === to.id;
    });
  }

  /**
   * Play a cue, or `cue:section`. With nothing playing it starts now. Otherwise it switches: `at` is `now`, `bar`, `phrase` or `section`
   * (default: the sheet's transition for these two, else `phrase`), and `via` a stinger cue to play between (default: the transition's).
   * Asking for what already plays (or is already queued) does nothing, so a host may call it on every state change. Returns the context time it lands.
   */
  function play(to, opts = {}) {
    sync();
    const r = ref(to), e = entryOf(r.id), now = context.currentTime, same = (m) => m && m.entry.id === r.id && (!r.section || sectionAt(m, now) === r.section);
    if (same(pending?.next ?? cur)) return (pending?.next ?? cur).startAt;
    cancelPending();
    if (same(cur)) return cur.startAt;   // back to what was playing: the queued switch is simply withdrawn
    const fromBar = sectionBar(e, r.section);
    if (!cur) { cur = launch(e, now + LEAD, fromBar, e.start); return now + LEAD; }
    const rule = ruleFor(cur, r) ?? {}, at = opts.at ?? rule.at ?? 'phrase', via = opts.via ?? rule.via;
    const when = landing(cur, now + LEAD, at), sameCue = cur.entry.id === e.id;
    const bridge = via ? entryOf(via) : null, starts = when + (bridge ? (bridge.lengthMs + (bridge.tailMs ?? 0)) / 1000 : 0);
    const next = launch(e, starts, fromBar, sameCue ? Object.keys(cur.level).filter((n) => cur.level[n]) : e.start);
    let stinger = null;
    if (bridge) stinger = oneShot(bridge.id, when);
    retire(cur, when);
    pending = { when, next, old: cur, stinger }; cur = next;
    return starts;
  }

  function oneShot(id, when) {
    entryOf(id);
    const src = context.createBufferSource(), g = context.createGain();
    src.buffer = audio[id]; g.connect(master); src.connect(g); src.start(when);
    src.onended = () => ignore(() => g.disconnect());
    return src;
  }

  /** Play a stinger cue once over the music: `at` `now` (default) or the next `bar`, `phrase` or `section` of what plays. Returns when it ends (context time). */
  function stinger(id, { at = 'now' } = {}) {
    sync();
    const e = entryOf(id), now = context.currentTime, m = pending?.next ?? cur;
    const when = m && at !== 'now' ? landing(m, now + LEAD, at) : now + LEAD;
    oneShot(id, when);
    return when + (e.lengthMs + (e.tailMs ?? 0)) / 1000;
  }

  /** Fade stems in or out so that exactly the named layers sound, starting at the next `at` (default `bar`) over `seconds`. */
  function layers(on, { at = 'bar', seconds = 0.5 } = {}) {
    sync();
    const m = pending?.next ?? cur;
    if (!m) throw new Error('nothing is playing');
    for (const n of on) if (!(n in m.gains)) throw new Error(`cue "${m.entry.id}" has no layer "${n}"`);
    const when = landing(m, context.currentTime + LEAD, at);
    for (const [n, g] of Object.entries(m.gains)) {
      const to = on.includes(n) ? 1 : 0;
      if (to === m.level[n]) continue;
      g.gain.cancelScheduledValues(when); g.gain.setValueAtTime(m.level[n], when); g.gain.linearRampToValueAtTime(to, when + seconds);
      m.level[n] = to;
    }
    return when;
  }

  /** Fade the music out over `seconds` and stop it. */
  function stop({ seconds = 0.5 } = {}) {
    sync(); cancelPending();
    if (!cur) return;
    const m = cur, now = context.currentTime;
    m.out.gain.setValueAtTime(1, now); m.out.gain.linearRampToValueAtTime(0, now + seconds);
    m.sources.forEach((s) => ignore(() => s.stop(now + seconds + 0.01)));
    schedule(() => ignore(() => m.out.disconnect()), (seconds + 0.1) * 1000);
    cur = null;
  }

  return {
    play, stinger, layers, stop,
    setVolume(v) { master.gain.setValueAtTime(v, context.currentTime); },
    /** Where the music is, in bars into its loop (null when nothing plays or a bridge is still sounding). */
    position() { sync(); const m = cur; if (!m) return null; const t = context.currentTime; return t < m.startAt ? null : barAt(m, t); },
    /** `{ cue, section, layers, switching }`: what plays, which stems are up, and the cue it is about to switch to, if any. */
    state() { sync(); return { cue: (pending?.old ?? cur)?.entry.id ?? null, section: (pending?.old ?? cur) ? sectionAt(pending?.old ?? cur, context.currentTime) : null, layers: cur ? Object.keys(cur.level).filter((n) => n && cur.level[n]) : [], switching: pending ? { to: pending.next.entry.id, at: pending.when } : null }; },
  };
}
