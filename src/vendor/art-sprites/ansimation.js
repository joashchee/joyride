// ANSImation: frames written as cursor-move deltas, played back by the terminal at the speed of the line (a stated baud), and a reader
// that replays one. The first frame clears the screen and prints in full; each later frame starts with a bare home (`ESC[H`, the
// frame mark the reader splits on) and then redraws only the changed stretch of each changed row, addressed with `ESC[row;colH`.
// The finished picture is always available as a still (`result.still`, or the last frame read), for reduced motion and for hosts
// that cannot play it. Browser-safe. Static art never uses cursor moves; these are for animation only.
import { Cells, legalize } from './cells.js';
import { readAnsi, writeAnsi, makeEncoder, withSauce } from './ansi.js';

const HOME = [0x1b, 0x5b, 0x48];   // ESC [ H
const TARGETS = ['classic', 'ice', 'modern'];

/**
 * Frames of cells (all one size) to the bytes of an ANSImation. Options are `writeAnsi`'s (`target`, `charset`, `colors`, `sauce`),
 * plus `baud` (default 2400: the line speed the timing is reckoned at) and `fps` (the rate the clip wants, default 8). Returns
 * `{ bytes, still, frames: [{ bytes, seconds }], seconds, baud, charset, target, colors, width, height, warnings }`: `seconds` is
 * how long the line takes to send a frame, and one warning counts the later frames that take longer than 1/fps. `sauce` writes SAUCE with
 * the ANSiMation file type; `still` is the last frame as an ordinary `.ans` with the same options.
 */
export function writeAnsimation(frames, { target = 'classic', charset, colors = 16, sauce, baud = 2400, fps = 8 } = {}) {
  if (!frames.length) throw new RangeError('an ANSImation needs at least one frame');
  if (!TARGETS.includes(target)) throw new Error(`unknown target "${target}"; known: ${TARGETS.join(', ')}`);
  if (!(baud > 0)) throw new RangeError(`baud must be positive, not ${baud}`);
  const { w, h } = frames[0];
  if (frames.some((f) => f.w !== w || f.h !== h)) throw new RangeError('every frame of an ANSImation must be the same size');
  charset ??= target === 'modern' ? 'utf8' : 'cp437';
  const bgLimit = target === 'classic' ? 8 : 16, work = frames.map((f) => legalize(f, { bgLimit }));
  const enc = makeEncoder({ target, charset, colors }), sizes = [], warnings = [];
  for (let k = 0; k < work.length; k++) {
    const start = enc.bytes.length;
    if (k === 0) {
      enc.raw('\x1b[2J\x1b[H');
      for (let y = 0; y < h; y++) { enc.row(work[0], y); if (y < h - 1) enc.raw('\r\n'); }
    } else {
      enc.raw('\x1b[H');
      for (let y = 0; y < h; y++) {
        const [a, b] = [work[k - 1], work[k]]; let x0 = 0, x1 = w;
        const same = (x) => ['ch', 'fg', 'bg', 'flags'].every((p) => a[p][y * w + x] === b[p][y * w + x]);
        while (x0 < w && same(x0)) x0++;
        if (x0 === w) continue;
        while (same(x1 - 1)) x1--;
        enc.raw(`\x1b[${y + 1};${x0 + 1}H`); enc.row(b, y, x0, x1, true);
      }
    }
    sizes.push(enc.bytes.length - start);
  }
  warnings.push(...enc.notes());
  const out = frames.length ? sizes.map((n) => ({ bytes: n, seconds: n / (baud / 10) })) : [];
  const slow = out.map((f, k) => [f, k]).filter(([f]) => f.seconds > 1 / fps).slice(1);       // frame 0 is the screen being drawn, not a step
  if (slow.length) warnings.push(`${slow.length} of ${out.length - 1} later frame(s) take longer than 1/${fps} s to send at ${baud} baud (slowest ${Math.max(...slow.map(([f]) => f.seconds)).toFixed(2)} s): it will play slower than ${fps} fps`);
  let bytes = Uint8Array.from(enc.bytes);
  const still = writeAnsi(work[work.length - 1], { target, charset, colors, sauce: sauce ? sauce : false }), ice = target === 'ice' && work.some((f) => f.bg.some((v) => v >= 8 && v < 16));
  if (sauce) { const sealed = withSauce(bytes, typeof sauce === 'object' ? sauce : {}, w, h, ice, 2); bytes = sealed.bytes; warnings.push(...sealed.notes); }
  return { bytes, still: still.bytes, frames: out, seconds: out.reduce((s, f) => s + f.seconds, 0), baud, charset, target, colors, width: w, height: h, warnings };
}

/**
 * Replay an ANSImation: the bytes (or string) to the frames it shows, one `Cells` per frame mark (`ESC[H`), each the whole screen as
 * it stands at that point. Also reads a plain `.ans` as one frame. Options are `readAnsi`'s. Returns `{ frames, still, width, height,
 * sauce, charset, ice, warnings }`; `still` is the last frame, the finished picture.
 */
export function readAnsimation(input, opts = {}) {
  const whole = readAnsi(input, opts), bytes = typeof input === 'string' ? new TextEncoder().encode(input) : input;
  const text = typeof input === 'string', end = whole.sauce ? sauceStart(bytes) : bytes.length, stop = bytes.indexOf(0x1a);
  const data = bytes.subarray(0, stop >= 0 && stop < end ? stop : end), marks = [];
  for (let i = 0; i + 2 < data.length; i++) if (data[i] === HOME[0] && data[i + 1] === HOME[1] && data[i + 2] === HOME[2]) marks.push(i + 3);
  const cuts = marks.length > 1 ? marks.slice(1).map((m) => m - 3).concat([data.length]) : [data.length];
  // A frame ends where the next mark begins, and each is the whole stream up to there: the grid carries over from the frames before.
  const o = { ...opts, charset: text ? 'utf8' : whole.charset, width: whole.width, ice: whole.ice };
  const reads = cuts.map((c) => readAnsi(data.subarray(0, c), o).cells), height = Math.max(whole.height, ...reads.map((c) => c.h));
  const frames = reads.map((c) => { const f = new Cells(whole.width, height); for (const k of ['ch', 'fg', 'bg', 'flags']) f[k].set(c[k]); return f; });
  return { frames, still: frames[frames.length - 1], width: whole.width, height, sauce: whole.sauce, charset: whole.charset, ice: whole.ice, warnings: whole.warnings };
}

// Where the SAUCE block (and its comments) begin in a file that has one.
function sauceStart(b) {
  const n = b[b.length - 128 + 104], at = b.length - 128 - (n ? 5 + 64 * n : 0);
  return n && at >= 0 && String.fromCharCode(...b.subarray(at, at + 5)) === 'COMNT' ? at : b.length - 128;
}
