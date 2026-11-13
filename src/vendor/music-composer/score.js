// The canvas of the engine, as Painter's Pixels is: a Score holds named tracks of integer notes on an integer tick grid.
// Everything is an integer (pitch 0..127, ticks, velocity) so the same recipe gives the same MIDI bytes on any machine.
export const PPQ = 96;          // ticks per quarter note
export const STEP = PPQ / 4;    // a sixteenth note, the grid recipes think in

const whole = (v, name) => { if (!Number.isInteger(v)) throw new Error(`${name} must be an integer, got ${v}`); return v; };

export class Score {
  /** `bars` of `beats` quarter notes at `bpm`, in `key` and `scale` (see scales.js). `keyChanges` is `[{ t, key, scale }]`: where a piece modulates. */
  constructor({ bpm = 120, bars = 4, beats = 4, key = 'C', scale = 'major', title = '', keyChanges = [] } = {}) {
    if (!(bpm > 0)) throw new Error(`bpm must be positive, got ${bpm}`);
    if (whole(bars, 'bars') < 1 || whole(beats, 'beats') < 1) throw new Error('bars and beats must be at least 1');
    Object.assign(this, { bpm, bars, beats, key, scale, title, keyChanges: keyChanges.map((k) => ({ ...k })) });
    this.tracks = new Map();
  }
  get barTicks() { return this.beats * PPQ; }
  get length() { return this.bars * this.barTicks; }
  /** The key and scale in force at tick `t`: the score's own until a key change says otherwise. */
  keyAt(t) {
    let k = { key: this.key, scale: this.scale };
    for (const c of this.keyChanges) if (c.t <= t) k = c;
    return { key: k.key, scale: k.scale };
  }
  /** Modulate: from tick `t` the music is in `key` and `scale`. Changes are kept in time order. */
  modulate(t, key, scale = this.scale) {
    this.keyChanges.push({ t, key, scale });
    this.keyChanges.sort((a, b) => a.t - b.t);
    return this;
  }
  /** The track called `name`, made on first use. `opts` (program, drum, wave, env, gain) apply only when it is made. */
  track(name, opts = {}) {
    let t = this.tracks.get(name);
    if (!t) { t = { name, program: 0, drum: false, wave: 'triangle', env: null, gain: 1, ...opts, notes: [] }; this.tracks.set(name, t); }
    return t;
  }
  /** One note: pitch `p` (MIDI number), start `t` and length `d` in ticks, velocity `v`. */
  note(track, p, t, d, v = 96) {
    if (whole(p, 'pitch') < 0 || p > 127) throw new Error(`pitch ${p} is outside 0..127`);
    if (whole(t, 'start') < 0) throw new Error(`start ${t} is before the score`);
    if (whole(d, 'length') < 1) throw new Error(`length ${d} must be at least 1 tick`);
    if (whole(v, 'velocity') < 1 || v > 127) throw new Error(`velocity ${v} is outside 1..127`);
    this.track(track).notes.push({ p, t, d, v });
    return this;
  }
  /** A track's notes in time order (then pitch), as a copy. */
  notes(track) { return [...(this.tracks.get(track)?.notes ?? [])].sort((a, b) => a.t - b.t || a.p - b.p); }
  count(track) { return track ? this.tracks.get(track)?.notes.length ?? 0 : [...this.tracks.values()].reduce((n, t) => n + t.notes.length, 0); }
  /** The tick where the last note ends. */
  end() { let e = 0; for (const t of this.tracks.values()) for (const n of t.notes) e = Math.max(e, n.t + n.d); return e; }
  clone() {
    const c = new Score(this);
    for (const [name, t] of this.tracks) c.tracks.set(name, { ...t, env: t.env && [...t.env], notes: t.notes.map((n) => ({ ...n })) });
    return c;
  }
  equals(o) {
    if (this.bpm !== o.bpm || this.bars !== o.bars || this.beats !== o.beats || this.key !== o.key || this.scale !== o.scale || this.tracks.size !== o.tracks.size) return false;
    for (const [name, t] of this.tracks) {
      const u = o.tracks.get(name);
      if (!u || u.program !== t.program || u.drum !== t.drum || t.notes.length !== u.notes.length) return false;
      const a = this.notes(name), b = o.notes(name);
      if (a.some((n, i) => n.p !== b[i].p || n.t !== b[i].t || n.d !== b[i].d || n.v !== b[i].v)) return false;
    }
    return true;
  }
}
