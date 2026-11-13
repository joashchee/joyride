/**
 * Theme swatches for the true colour palette (src/lib/theme.ts): the
 * choices Settings offers, the way an editor offers dark, light and a few
 * well-known colour schemes. A swatch is only colours. The look (the VGA
 * font, the grid, the frames) never changes; each swatch re-points the role
 * tokens in src/ansiapps-theme.css and gives Monaco its syntax tones.
 *
 * 16-colour (DOS mode) has no swatches: it is the 16 CGA/VGA colours, fixed.
 *
 * Plain data on purpose: scripts/check-swatches.ts reads this file and
 * checks every text and background pair (4.5:1, WCAG AA).
 */

/** The CSS custom properties a swatch sets (the value is a #rrggbb). */
export interface UiColors {
  bg: string; text: string; muted: string; line: string;
  win: string; ink: string; winmuted: string;
  select: string; menuhi: string; hover: string; rowhover: string;
  field: string; fieldtext: string; fieldhint: string; disabled: string;
  console: string; consoletext: string; consoleerr: string; consolelink: string;
  err: string; onerr: string; bad: string; code: string; strong: string; closebox: string;
  track: string; thumb: string; shadow: string; focus: string; fill: string; fill2: string;
}

export interface Syntax {
  keyword: string; string: string; number: string; comment: string;
  func: string; key: string; op: string; escape: string;
}

/** What Monaco needs beyond the UI colours. */
export interface EditorColors {
  /** Current line, gutter, selection (the text keeps its colour on it), inactive selection. */
  lineTint: string; gutter: string; sel: string; inactiveSel: string;
  lineNumber: string; lineNumberActive: string; guide: string; guideActive: string;
  word: string; bracket: string; find: string; findOther: string;
}

export interface Swatch {
  id: string;
  label: string;
  /** For the light/dark label and Monaco's base theme. */
  dark: boolean;
  ui: UiColors;
  syntax: Syntax;
  editor: EditorColors;
}

/** Classic: the blue desktop in 24-bit values of the 16 colours, with the editor's extra tones. */
const classic: Swatch = {
  id: "classic",
  label: "Classic blue",
  dark: true,
  ui: {
    bg: "#0000aa", text: "#ffffff", muted: "#aaaaaa", line: "#55ffff",
    win: "#aaaaaa", ink: "#000000", winmuted: "#0000aa",
    select: "#00aaaa", menuhi: "#00aa00", hover: "#ffffff", rowhover: "#555555",
    field: "#000000", fieldtext: "#ffffff", fieldhint: "#aaaaaa", disabled: "#555555",
    console: "#000000", consoletext: "#aaaaaa", consoleerr: "#ff5555", consolelink: "#55ffff",
    err: "#aa0000", onerr: "#ffffff", bad: "#ff8a8a", code: "#ffff55", strong: "#55ffff", closebox: "#55ff55",
    track: "#00aaaa", thumb: "#0000aa", shadow: "#000000", focus: "#ffffff", fill: "#ffff55", fill2: "#aa5500",
  },
  syntax: { keyword: "#ffe066", string: "#9dff9d", number: "#ff9dff", comment: "#c0c0e0", func: "#8ff0ff", key: "#8ff0ff", op: "#e0e0ff", escape: "#ffb86b" },
  editor: {
    lineTint: "#0000c4", gutter: "#00007a", sel: "#000055", inactiveSel: "#00007a",
    lineNumber: "#b0b0e8", lineNumberActive: "#ffe066", guide: "#3030d0", guideActive: "#8ff0ff",
    word: "#0000e0", bracket: "#00007a", find: "#00aaaa", findOther: "#00007a",
  },
};

const dark: Swatch = {
  id: "dark",
  label: "Dark",
  dark: true,
  ui: {
    bg: "#1e2028", text: "#e8eaf0", muted: "#a3a8b8", line: "#7fdbea",
    win: "#b9bcc8", ink: "#0b0c10", winmuted: "#20306e",
    select: "#5ec4c4", menuhi: "#7bc47f", hover: "#ffffff", rowhover: "#3b3f4c",
    field: "#0b0c10", fieldtext: "#e8eaf0", fieldhint: "#a3a8b8", disabled: "#4a4e5c",
    console: "#0b0c10", consoletext: "#b9bcc8", consoleerr: "#ff8585", consolelink: "#7fdbea",
    err: "#a8322d", onerr: "#ffffff", bad: "#ff8585", code: "#f0c674", strong: "#7fdbea", closebox: "#8fe388",
    track: "#5ec4c4", thumb: "#1e2028", shadow: "#000000", focus: "#ffffff", fill: "#f0c674", fill2: "#8a5a1c",
  },
  syntax: { keyword: "#f0c674", string: "#8fe388", number: "#d79bff", comment: "#9aa0b2", func: "#7fdbea", key: "#7fdbea", op: "#c9cdd9", escape: "#ffb86b" },
  editor: {
    lineTint: "#272a34", gutter: "#171920", sel: "#2f3546", inactiveSel: "#2c3040",
    lineNumber: "#9aa0b2", lineNumberActive: "#f0c674", guide: "#3a3f4d", guideActive: "#7fdbea",
    word: "#33384a", bracket: "#33384a", find: "#5ec4c4", findOther: "#33384a",
  },
};

const light: Swatch = {
  id: "light",
  label: "Light",
  dark: false,
  ui: {
    bg: "#fbf8f0", text: "#1d1d2b", muted: "#5a5a70", line: "#0a6577",
    win: "#d6d0c0", ink: "#14141c", winmuted: "#2a2a7a",
    select: "#6cc3c3", menuhi: "#7bc47f", hover: "#ffffff", rowhover: "#c9c2ae",
    field: "#ffffff", fieldtext: "#1d1d2b", fieldhint: "#6a6a80", disabled: "#6b6b6b",
    console: "#14141c", consoletext: "#d6d0c0", consoleerr: "#ff7b7b", consolelink: "#7fe0f0",
    err: "#b3261e", onerr: "#ffffff", bad: "#b3261e", code: "#7a4b00", strong: "#0a5c70", closebox: "#0b6b2b",
    track: "#6cc3c3", thumb: "#fbf8f0", shadow: "#14141c", focus: "#1d1d2b", fill: "#ffd24d", fill2: "#aa5500",
  },
  syntax: { keyword: "#7a2fa0", string: "#0a6428", number: "#96400a", comment: "#565b6e", func: "#0a5c8a", key: "#0a5c8a", op: "#3a3a4a", escape: "#96400a" },
  editor: {
    lineTint: "#efe9d8", gutter: "#f0eadb", sel: "#d4e4f5", inactiveSel: "#e0e6ee",
    lineNumber: "#5f5f73", lineNumberActive: "#7a2fa0", guide: "#d9d2bf", guideActive: "#0a6577",
    word: "#e6dfc9", bracket: "#d8e3ef", find: "#ffd24d", findOther: "#f2e6b3",
  },
};

const solarizedDark: Swatch = {
  id: "solarized-dark",
  label: "Solarized dark",
  dark: true,
  ui: {
    bg: "#002b36", text: "#93a1a1", muted: "#839496", line: "#2aa198",
    win: "#93a1a1", ink: "#002b36", winmuted: "#073642",
    select: "#2aa198", menuhi: "#859900", hover: "#fdf6e3", rowhover: "#073642",
    field: "#073642", fieldtext: "#eee8d5", fieldhint: "#93a1a1", disabled: "#586e75",
    console: "#00212b", consoletext: "#93a1a1", consoleerr: "#ff7a6e", consolelink: "#35c9bd",
    err: "#a5231f", onerr: "#fdf6e3", bad: "#ff7a6e", code: "#e6b422", strong: "#35c9bd", closebox: "#a3b800",
    track: "#2aa198", thumb: "#002b36", shadow: "#00141a", focus: "#fdf6e3", fill: "#b58900", fill2: "#586e75",
  },
  syntax: { keyword: "#d4aa00", string: "#35c9bd", number: "#ff7aa8", comment: "#93a1a1", func: "#7db4f5", key: "#7db4f5", op: "#a3b0b0", escape: "#f08a52" },
  editor: {
    lineTint: "#073642", gutter: "#00212b", sel: "#0a3a47", inactiveSel: "#073642",
    lineNumber: "#93a1a1", lineNumberActive: "#d4aa00", guide: "#0e4a5a", guideActive: "#35c9bd",
    word: "#0b3946", bracket: "#0b3946", find: "#2aa198", findOther: "#0e4a5a",
  },
};

const solarizedLight: Swatch = {
  id: "solarized-light",
  label: "Solarized light",
  dark: false,
  ui: {
    bg: "#fdf6e3", text: "#3f555c", muted: "#586e75", line: "#1f7f78",
    win: "#eee8d5", ink: "#073642", winmuted: "#2c5f8a",
    select: "#7cc9c0", menuhi: "#a3b800", hover: "#ffffff", rowhover: "#e4dcc2",
    field: "#ffffff", fieldtext: "#073642", fieldhint: "#586e75", disabled: "#657b83",
    console: "#073642", consoletext: "#eee8d5", consoleerr: "#ff7a6e", consolelink: "#52d6cb",
    err: "#a5231f", onerr: "#fdf6e3", bad: "#a5231f", code: "#7d5f00", strong: "#1f6f6a", closebox: "#5c6b00",
    track: "#7cc9c0", thumb: "#fdf6e3", shadow: "#073642", focus: "#073642", fill: "#b58900", fill2: "#586e75",
  },
  syntax: { keyword: "#6e5400", string: "#1a6360", number: "#b52a63", comment: "#4f656c", func: "#1d5fa6", key: "#1d5fa6", op: "#3f555c", escape: "#a64008" },
  editor: {
    lineTint: "#f1ebd9", gutter: "#f4eddb", sel: "#e4f0ed", inactiveSel: "#e6e9df",
    lineNumber: "#586e75", lineNumberActive: "#6e5400", guide: "#e2dac0", guideActive: "#1f7f78",
    word: "#e4dcc2", bracket: "#d9e6e3", find: "#f1d36b", findOther: "#f3e7b5",
  },
};

/** A one-colour screen: amber phosphor, the look of an old monochrome terminal. */
const amber: Swatch = {
  id: "amber",
  label: "Amber phosphor",
  dark: true,
  ui: {
    bg: "#0a0600", text: "#ffb000", muted: "#c88a00", line: "#ffb000",
    win: "#ffb000", ink: "#0a0600", winmuted: "#3d2600",
    select: "#ffd24d", menuhi: "#ffd24d", hover: "#fff1c2", rowhover: "#3d2600",
    field: "#1a1000", fieldtext: "#ffb000", fieldhint: "#c88a00", disabled: "#7a5200",
    console: "#000000", consoletext: "#ffb000", consoleerr: "#ffd24d", consolelink: "#fff1c2",
    err: "#ffb000", onerr: "#0a0600", bad: "#ffd24d", code: "#ffd24d", strong: "#fff1c2", closebox: "#ffd24d",
    track: "#ffb000", thumb: "#0a0600", shadow: "#000000", focus: "#fff1c2", fill: "#ffb000", fill2: "#7a5200",
  },
  syntax: { keyword: "#ffd24d", string: "#ffe9a8", number: "#fff1c2", comment: "#e6a300", func: "#ffd24d", key: "#ffe9a8", op: "#ffb000", escape: "#fff1c2" },
  editor: {
    lineTint: "#241700", gutter: "#140d00", sel: "#4a3000", inactiveSel: "#3d2600",
    lineNumber: "#c88a00", lineNumberActive: "#ffd24d", guide: "#4d3300", guideActive: "#ffb000",
    word: "#3d2600", bracket: "#3d2600", find: "#7a5200", findOther: "#3d2600",
  },
};

/** The same, in green phosphor. */
const green: Swatch = {
  id: "green",
  label: "Green phosphor",
  dark: true,
  ui: {
    bg: "#001500", text: "#33ff66", muted: "#22c04a", line: "#33ff66",
    win: "#33ff66", ink: "#001500", winmuted: "#003d12",
    select: "#99ffb3", menuhi: "#99ffb3", hover: "#d6ffe0", rowhover: "#003d12",
    field: "#002200", fieldtext: "#33ff66", fieldhint: "#22c04a", disabled: "#0f7a2b",
    console: "#000000", consoletext: "#33ff66", consoleerr: "#d6ffe0", consolelink: "#99ffb3",
    err: "#33ff66", onerr: "#001500", bad: "#d6ffe0", code: "#99ffb3", strong: "#d6ffe0", closebox: "#99ffb3",
    track: "#33ff66", thumb: "#001500", shadow: "#000000", focus: "#d6ffe0", fill: "#33ff66", fill2: "#0f7a2b",
  },
  syntax: { keyword: "#99ffb3", string: "#d6ffe0", number: "#ffffff", comment: "#33d65a", func: "#99ffb3", key: "#d6ffe0", op: "#33ff66", escape: "#ffffff" },
  editor: {
    lineTint: "#002a00", gutter: "#001000", sel: "#004a16", inactiveSel: "#003d12",
    lineNumber: "#22c04a", lineNumberActive: "#99ffb3", guide: "#004d16", guideActive: "#33ff66",
    word: "#003d12", bracket: "#003d12", find: "#0f7a2b", findOther: "#003d12",
  },
};

export const SWATCHES: Swatch[] = [classic, dark, light, solarizedDark, solarizedLight, amber, green];
export const DEFAULT_SWATCH = "classic";

export function swatchById(id: string | null | undefined): Swatch {
  return SWATCHES.find((s) => s.id === id) ?? classic;
}
