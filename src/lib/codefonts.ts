/**
 * The coding fonts that ship with JoyrIDE, and how a size is chosen for one so its characters land on whole pixels.
 *
 * The default is the text-mode look: IBM VGA 8x16 in the editor too. A person may choose another font for code in Settings; the rest of
 * the UI stays VGA on its 8 by 16 grid (decided 2026-11-14, an exception to "16px VGA font only" that applies inside Monaco only).
 *
 * Every font is Regular, unmodified, as its project published it, with its licence beside it in `public/fonts/code/licences/`.
 * `advance` and `upem` are read from each file's own `hmtx` and `head` tables by `scripts/check-code-fonts.ts`, which fails if they,
 * a file or a licence is wrong, so the sizes offered here are computed from the real fonts.
 */

export interface CodeFont {
  id: string;
  name: string;
  file: string;
  /** "vector": any size, hinted by the system; "bitmap": drawn on a pixel grid, so only whole multiples of `native` are clean. */
  kind: "vector" | "bitmap";
  /** The cell height in pixels the pixel font was drawn for (bitmap only). */
  native?: number;
  /** The one character advance and the units per em, from the font file. */
  advance: number;
  upem: number;
  licence: string;
  author: string;
  source: string;
  note: string;
}

const CODE = "/fonts/code/";
const f = (c: Omit<CodeFont, "kind"> & { kind?: CodeFont["kind"] }): CodeFont => ({ kind: "vector", ...c });

export const CODE_FONTS: CodeFont[] = [
  f({ id: "jetbrains-mono", name: "JetBrains Mono", file: "jetbrains-mono.ttf", advance: 600, upem: 1000, licence: "OFL-1.1", author: "The JetBrains Mono Project Authors", source: "https://github.com/JetBrains/JetBrainsMono", note: "Made for code: tall lowercase, clear 0 O l 1, hinted for small sizes" }),
  f({ id: "source-code-pro", name: "Source Code Pro", file: "source-code-pro.otf", advance: 600, upem: 1000, licence: "OFL-1.1", author: "Adobe", source: "https://github.com/adobe-fonts/source-code-pro", note: "Adobe's code font; CFF outlines; no ligatures" }),
  f({ id: "hack", name: "Hack", file: "hack.ttf", advance: 1233, upem: 2048, licence: "MIT AND Bitstream-Vera", author: "Source Foundry Authors; Bitstream Inc.", source: "https://github.com/source-foundry/Hack", note: "Hand-hinted for small sizes on small screens; from DejaVu and Bitstream Vera" }),
  f({ id: "ibm-plex-mono", name: "IBM Plex Mono", file: "ibm-plex-mono.ttf", advance: 600, upem: 1000, licence: "OFL-1.1", author: "IBM Corp.", source: "https://github.com/IBM/plex", note: "A slab-serif mono with a technical feel" }),
  f({ id: "inconsolata", name: "Inconsolata", file: "inconsolata.ttf", advance: 500, upem: 1000, licence: "OFL-1.1", author: "The Inconsolata Project Authors", source: "https://github.com/googlefonts/Inconsolata", note: "Narrow: more columns on a small screen" }),
  f({ id: "fira-code", name: "Fira Code", file: "fira-code.ttf", advance: 1200, upem: 1950, licence: "OFL-1.1", author: "The Fira Code Project Authors", source: "https://github.com/tonsky/FiraCode", note: "Ligatures are turned off here" }),
  f({ id: "intel-one-mono", name: "Intel One Mono", file: "intel-one-mono.ttf", advance: 614, upem: 1000, licence: "OFL-1.1", author: "The Intel One Mono Project Authors", source: "https://github.com/intel/intel-one-mono", note: "Designed for legibility, including for low vision" }),
  f({ id: "dejavu-sans-mono", name: "DejaVu Sans Mono", file: "dejavu-sans-mono.ttf", advance: 1233, upem: 2048, licence: "Bitstream-Vera", author: "Bitstream Inc.; DejaVu contributors (public domain)", source: "https://github.com/dejavu-fonts/dejavu-fonts", note: "The widest coverage of languages and symbols" }),
  f({ id: "spleen-6x12", name: "Spleen 6x12", file: "spleen-6x12.otf", kind: "bitmap", native: 12, advance: 512, upem: 1024, licence: "BSD-2-Clause", author: "Frederic Cambus", source: "https://github.com/fcambus/spleen", note: "A pixel font drawn on a 6 by 12 cell: sharp on a small laptop screen" }),
  f({ id: "spleen-8x16", name: "Spleen 8x16", file: "spleen-8x16.otf", kind: "bitmap", native: 16, advance: 512, upem: 1024, licence: "BSD-2-Clause", author: "Frederic Cambus", source: "https://github.com/fcambus/spleen", note: "A pixel font on an 8 by 16 cell, like the text-mode UI" }),
  f({ id: "spleen-12x24", name: "Spleen 12x24", file: "spleen-12x24.otf", kind: "bitmap", native: 24, advance: 512, upem: 1023, licence: "BSD-2-Clause", author: "Frederic Cambus", source: "https://github.com/fcambus/spleen", note: "A pixel font on a 12 by 24 cell: a large or high-density screen" }),
  f({ id: "spleen-16x32", name: "Spleen 16x32", file: "spleen-16x32.otf", kind: "bitmap", native: 32, advance: 512, upem: 1024, licence: "BSD-2-Clause", author: "Frederic Cambus", source: "https://github.com/fcambus/spleen", note: "A pixel font on a 16 by 32 cell: a big monitor" }),
  f({ id: "spleen-32x64", name: "Spleen 32x64", file: "spleen-32x64.otf", kind: "bitmap", native: 64, advance: 512, upem: 1024, licence: "BSD-2-Clause", author: "Frederic Cambus", source: "https://github.com/fcambus/spleen", note: "A pixel font on a 32 by 64 cell: a wall of screens, read from a distance" }),
];

/** The default: the text-mode font, as the rest of the UI. Not a file in `CODE_FONTS`: it is the theme's own. */
export const VGA_ID = "vga";
export const VGA_FAMILY = '"ansiapps VGA"';

export const MIN_SIZE = 10;
export const MAX_SIZE = 72;
/** A character's advance this far from a whole device pixel is still "clean": a letter-spacing nudge of at most this much fixes it. */
export const TOLERANCE = 0.25;

export const fontById = (id: string) => CODE_FONTS.find((c) => c.id === id);

/** One size's cell: the character advance and the line height in CSS pixels, both whole *device* pixels. */
export interface Cell {
  size: number;
  /** Advance after the nudge, in CSS px: a whole number of device pixels. */
  width: number;
  /** The nudge to give Monaco as `letterSpacing`, in CSS px (zero when the font already lands on whole pixels). */
  letterSpacing: number;
  lineHeight: number;
  /** How far the font's own advance was from whole pixels, in device px. */
  drift: number;
}

/** The cell a font makes at `size` CSS px on a screen of `dpr` device pixels per CSS pixel. */
export function cellFor(font: Pick<CodeFont, "advance" | "upem" | "kind" | "native">, size: number, dpr: number): Cell {
  const raw = (font.advance / font.upem) * size * dpr; // the advance in device pixels
  const whole = Math.max(1, Math.round(raw));
  const lh = Math.max(1, Math.round(size * dpr * (font.kind === "bitmap" ? 1 : 1.35)));
  return { size, width: whole / dpr, letterSpacing: (whole - raw) / dpr, lineHeight: lh / dpr, drift: Math.abs(raw - whole) };
}

/**
 * The sizes worth offering. A bitmap font is clean only at whole multiples of the size it was drawn for (its pixels then fall on
 * device pixels); a vector font at any size whose advance is within `TOLERANCE` of whole device pixels.
 */
export function cleanSizes(font: CodeFont, dpr: number): number[] {
  const out: number[] = [];
  if (font.kind === "bitmap") {
    const n = font.native!;
    for (let k = 1; n * k <= MAX_SIZE * 2; k++) if (n * k >= MIN_SIZE || k > 1) out.push(n * k);
    return out.filter((s) => s >= Math.min(MIN_SIZE, n));
  }
  for (let s = MIN_SIZE; s <= MAX_SIZE; s++) if (cellFor(font, s, dpr).drift <= TOLERANCE) out.push(s);
  return out;
}

/** The nearest offered size to `want`. */
export function nearestSize(sizes: number[], want: number): number {
  return sizes.reduce((best, s) => (Math.abs(s - want) < Math.abs(best - want) ? s : best), sizes[0] ?? want);
}

/** Roughly which screens a size suits, for the Settings hint: a laptop panel, a desktop monitor, a big or multi-monitor setup. */
export function sizeHint(size: number): string {
  return size <= 16 ? "laptop" : size <= 24 ? "desktop" : "large display";
}

// ---- the choice, remembered -------------------------------------------------------------------------------------------------

export interface CodeChoice {
  id: string;
  size: number;
}
const KEY = "joyride.codefont";
export const DEFAULT_CHOICE: CodeChoice = { id: VGA_ID, size: 16 };

export function loadChoice(): CodeChoice {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (v && typeof v.size === "number" && (v.id === VGA_ID || fontById(v.id))) return { id: v.id, size: v.size };
  } catch {
    // unreadable: the default
  }
  return DEFAULT_CHOICE;
}

const listeners = new Set<(c: CodeChoice) => void>();
/** Remembers the choice and tells the editor. */
export function saveChoice(c: CodeChoice) {
  try {
    localStorage.setItem(KEY, JSON.stringify(c));
  } catch {
    // not remembered across launches
  }
  listeners.forEach((l) => l(c));
}
export function onChoice(l: (c: CodeChoice) => void): () => void {
  listeners.add(l);
  return () => void listeners.delete(l);
}

const loaded = new Map<string, Promise<void>>();
/** Loads a font's file once and registers it as a CSS family `JoyrIDE Code <id>`; the VGA font is already loaded by the theme. */
export function loadFont(font: CodeFont): Promise<void> {
  let p = loaded.get(font.id);
  if (!p) {
    const face = new FontFace(familyOf(font), `url("${CODE}${font.file}")`);
    p = face.load().then((ok) => void document.fonts.add(ok));
    loaded.set(font.id, p);
  }
  return p;
}
export const familyOf = (font: CodeFont) => `JoyrIDE Code ${font.id}`;

/** What Monaco is given for a choice: the family, a size, the line height and the letter-spacing that make the cell whole pixels. */
export interface EditorFont {
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  letterSpacing: number;
  fontLigatures: boolean;
  /** The cell in CSS px, for snapping the window. */
  cell: { width: number; height: number };
}

export function editorFont(c: CodeChoice, dpr: number): EditorFont {
  const font = fontById(c.id);
  if (!font) return { fontFamily: `${VGA_FAMILY}, ui-monospace, monospace`, fontSize: 16, lineHeight: 16, letterSpacing: 0, fontLigatures: false, cell: { width: 8, height: 16 } };
  const cell = cellFor(font, c.size, dpr);
  return {
    fontFamily: `"${familyOf(font)}", ui-monospace, monospace`,
    fontSize: c.size,
    lineHeight: cell.lineHeight,
    letterSpacing: cell.letterSpacing,
    fontLigatures: false,
    cell: { width: cell.width, height: cell.lineHeight },
  };
}
