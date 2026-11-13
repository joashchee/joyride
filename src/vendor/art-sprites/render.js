// Putting a Pixels canvas on screen. Browser only (it reaches for ImageData and canvas elements when called, not when imported,
// so importing it under Node is harmless). Every function takes an integer `scale` and scales by nearest neighbour: pixels stay square.

/** ImageData for a frame at an integer scale, over `bg` (a hex colour) or transparent. */
export const toImageData = (px, scale = 1, bg = null) => new ImageData(new Uint8ClampedArray(px.toRgba(scale, bg)), px.w * scale, px.h * scale);

/** A frame on a canvas. Pass `canvas` to reuse one (it is resized); otherwise a new element is made. */
export function toCanvas(px, { scale = 1, bg = null, canvas = document.createElement('canvas') } = {}) {
  canvas.width = px.w * scale; canvas.height = px.h * scale;
  canvas.getContext('2d').putImageData(toImageData(px, scale, bg), 0, 0);
  return canvas;
}

/** A frame as a PNG data URL: what an `<img src>` or a CSS `url()` takes. */
export const toDataUrl = (px, opts) => toCanvas(px, opts).toDataURL('image/png');

/** Draw a frame onto an existing 2D context at (x, y), compositing over what is there (`putImageData` would overwrite it). */
export function drawFrame(ctx, px, x = 0, y = 0, scale = 1) {
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(toCanvas(px, { scale }), x, y);
}

/**
 * Play a clip (`{ frames, fps, loop }`, as in `art.clips[name]` or the art itself) on a canvas until it ends or `stop()` is called.
 * Returns `stop`. `onFrame(i)` fires as each frame is shown (the impact frame in `clip.events`, say). `raf` and `now` are injectable for tests.
 */
export function play(canvas, clip, { scale = 1, loop = clip.loop, onFrame, onEnd, raf = (f) => requestAnimationFrame(f), now = () => performance.now() } = {}) {
  const frames = clip.frames.map((f) => toCanvas(f, { scale })), ms = 1000 / (clip.fps || 8), ctx = canvas.getContext('2d');
  canvas.width = frames[0].width; canvas.height = frames[0].height;
  ctx.imageSmoothingEnabled = false;
  let stopped = false, shown = -1; const t0 = now();
  const tick = () => {
    if (stopped) return;
    const n = Math.floor((now() - t0) / ms);
    if (!loop && n >= frames.length) { stopped = true; onEnd?.(); return; }
    const i = n % frames.length;
    if (i !== shown) { shown = i; ctx.clearRect(0, 0, canvas.width, canvas.height); ctx.drawImage(frames[i], 0, 0); onFrame?.(i); }
    raf(tick);
  };
  tick();
  return () => { stopped = true; };
}
