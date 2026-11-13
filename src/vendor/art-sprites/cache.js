// A small cache for drawn looks. A game that redraws a character whenever its gear changes would otherwise draw the same
// pixels every tick; this keys a look by its spec, draws it once, and keeps the most recent few. No DOM: `toUrl` is injected.

/**
 * `draw(spec)` returns Art (see recipes/index.js). `toUrl(pixels)` turns one frame into whatever the host displays (an image URL,
 * an ImageBitmap, a texture), so the cache serves a browser, a canvas library or a test alike. Looks are keyed by `JSON.stringify(spec)`.
 */
export function createLookCache(draw, toUrl, { keep = 12 } = {}) {
  const cache = new Map();
  return {
    /** `{ key, spec, art, still, clip(name), fps(name) }`; `clip` is null for a name the art has no clip for. Throws if `draw` does. */
    get(spec) {
      const key = JSON.stringify(spec);
      if (cache.has(key)) { const hit = cache.get(key); cache.delete(key); cache.set(key, hit); return hit; }
      const art = draw(spec), urls = {};
      const look = { key, spec, art, still: toUrl(art.frames[0]), clip: (name) => (art.clips?.[name] ? (urls[name] ??= art.clips[name].frames.map(toUrl)) : null), fps: (name) => art.clips?.[name]?.fps ?? 8 };
      cache.set(key, look);
      if (cache.size > keep) cache.delete(cache.keys().next().value);
      return look;
    },
    clear: () => cache.clear(),
    get size() { return cache.size; },
  };
}
