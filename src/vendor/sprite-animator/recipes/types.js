// The motion-type recipes: one recipe for each preset of each motion type in types.js, id `<type>.<preset>` (`figure.hop`, `cloth.gust`,
// `tiling.scroll`). They make the clip Painter's own preset of that name makes (Painter 1.20's `motion: 'hop'`), integer for integer, plus the
// lint (the type's channels and limits), `writeClip`, `tempo` and the CLI. Params are the preset's own, plus `w` and `h`, the art's size.
import { MOTION_TYPES, motionClip } from '../types.js';

// Three variants per preset, so the golden test and `animate variants` see more than the default: a bigger art, 64 wide (a size-led preset scales with
// it, so `scroll` steps by 2 to stay short), and the first number param doubled, or the other direction for a scroll.
const variants = (type, name) => () => {
  const params = MOTION_TYPES[type].presets[name].params, [key, v] = Object.entries(params).find(([k, x]) => typeof x === 'number' && k !== 'fps' && k !== 'steps') ?? [];
  const big = type === 'tiling' && name !== 'ripple' ? { w: 64, h: 64, step: 2 } : { w: 64, h: 48 };
  return [{}, big, key ? { [key]: v * 2 } : { fps: params.fps * 2 }, ...(name === 'scroll' ? [{ direction: 'up' }] : [])];
};

/** `{ 'figure.hop': recipe, ... }`: every preset of every type, in the shape of a RECIPES entry. */
export const TYPE_RECIPES = Object.fromEntries(Object.entries(MOTION_TYPES).flatMap(([type, t]) => Object.entries(t.presets).map(([name, p]) =>
  [`${type}.${name}`, { describe: `${type} ${name}: ${p.describe}; ${Object.keys(p.params).join(', ')}, w, h`, type, variants: variants(type, name), draw: (params) => motionClip(type, name, params) }])));
