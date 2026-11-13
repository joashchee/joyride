/**
 * JoyrIDE's UI is the ANSIapps text-mode look and nothing else: there is no
 * graphical ("modern") theme and no switch for one. The appearance options
 * are only colours:
 *
 *  - Palette "truecolor" (the default): the text-mode look in 24-bit colour,
 *    with a choice of theme swatches (src/swatches.ts: classic blue, dark,
 *    light, Solarized, amber and green phosphor, ...).
 *  - Palette "dos16" (DOS mode): the 16 CGA/VGA colours, fixed. No swatches.
 *
 * The choices are `data-palette` and `data-swatch` on <html>; a swatch sets
 * the role tokens (--bg, --r-win, ...) that src/ansiapps-theme.css reads, and
 * the stored copy lets index.html set them before first paint (key
 * "joyride.ui", the same names), so there is no flash of another colour.
 */
import { DEFAULT_SWATCH, SWATCHES, swatchById, type Swatch } from "../swatches";

export type Palette = "truecolor" | "dos16";

export const PALETTES: { id: Palette; label: string; note: string }[] = [
  { id: "truecolor", label: "True colour", note: "24-bit colour with a choice of theme swatches" },
  { id: "dos16", label: "16-colour (DOS mode)", note: "only the 16 CGA/VGA text-mode colours" },
];

export { SWATCHES };

/** Also read by the inline script in index.html. */
const PALETTE_KEY = "joyride.palette";
const SWATCH_KEY = "joyride.swatch";
const UI_KEY = "joyride.ui";

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}
function write(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    // localStorage unavailable: the choice just won't persist across launches.
  }
}

export const loadPalette = (): Palette => (read(PALETTE_KEY) === "dos16" ? "dos16" : "truecolor");
export const loadSwatch = (): Swatch => swatchById(read(SWATCH_KEY) ?? DEFAULT_SWATCH);

export interface Appearance {
  palette: Palette;
  swatch: Swatch;
}

const listeners = new Set<(a: Appearance) => void>();

/** Calls back whenever the palette or swatch changes (the editor re-themes itself). Returns the unsubscribe. */
export function onAppearance(f: (a: Appearance) => void): () => void {
  listeners.add(f);
  return () => void listeners.delete(f);
}

export function currentAppearance(): Appearance {
  const root = document.documentElement.dataset;
  return { palette: root.palette === "dos16" ? "dos16" : "truecolor", swatch: swatchById(root.swatch) };
}

/** The CSS property of a swatch colour: the four desktop ones are plain, the rest are roles. */
const prop = (name: string) => (["bg", "text", "muted", "line"].includes(name) ? `--${name}` : `--r-${name}`);

export function applyAppearance(palette: Palette, swatch: Swatch) {
  const root = document.documentElement;
  root.dataset.palette = palette;
  root.dataset.swatch = palette === "dos16" ? "dos16" : swatch.id;
  for (const name of Object.keys(swatch.ui)) {
    if (palette === "truecolor") root.style.setProperty(prop(name), swatch.ui[name as keyof Swatch["ui"]]);
    else root.style.removeProperty(prop(name)); // DOS mode: the stylesheet's 16 colours
  }
  write(PALETTE_KEY, palette);
  write(SWATCH_KEY, swatch.id);
  write(UI_KEY, palette === "truecolor" ? JSON.stringify(swatch.ui) : null);
  const a = { palette, swatch };
  listeners.forEach((f) => f(a));
}
