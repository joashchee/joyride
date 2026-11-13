/**
 * The editor's two palettes (src/lib/theme.ts). Monaco is Borland-style:
 * the blue desktop with light text, popups as light-gray Turbo Vision
 * windows. Every text/background pair is on the ranked list in
 * docs/ansiapps-color-contrast.md (DOS mode) or 4.5:1 or better, checked
 * with the same formula (true colour).
 *
 * "dos16" uses only the 16 CGA/VGA colours, so there is no tint for the
 * current line, no translucent selection and no shade between two colours.
 * "truecolor" has one theme per swatch (src/swatches.ts, chosen in Settings):
 * the swatch's own surfaces, syntax tones and highlights.
 */
import * as monaco from "monaco-editor/editor/editor.api.js";
import type { Appearance } from "./lib/theme";
import { SWATCHES, type Swatch } from "./swatches";

/** The Monaco theme for the current palette and swatch (DOS mode has no swatches). */
export const themeId = (a: Appearance) => (a.palette === "dos16" ? "joyride-dos16" : "joyride-" + a.swatch.id);

// The 16 text-mode colours (the same values as --dos-* in ansiapps-theme.css).
const D = {
  black: "#000000", blue: "#0000aa", green: "#00aa00", cyan: "#00aaaa",
  red: "#aa0000", magenta: "#aa00aa", brown: "#aa5500", lightgray: "#aaaaaa",
  darkgray: "#555555", lightblue: "#5555ff", lightgreen: "#55ff55", lightcyan: "#55ffff",
  lightred: "#ff5555", lightmagenta: "#ff55ff", yellow: "#ffff55", white: "#ffffff",
};

/** What a syntax class looks like in each palette. */
interface Tones {
  text: string; keyword: string; string: string; number: string; comment: string;
  func: string; key: string; op: string; escape: string; invalid: string;
}

const dos: Tones = {
  text: D.white, keyword: D.yellow, string: D.lightgreen, number: D.lightmagenta, comment: D.lightgray,
  func: D.lightcyan, key: D.lightcyan, op: D.white, escape: D.lightmagenta, invalid: D.white,
};
const hex = (c: string) => c.slice(1);

function rules(t: Tones): monaco.editor.ITokenThemeRule[] {
  return [
    { token: "", foreground: hex(t.text) },
    { token: "keyword", foreground: hex(t.keyword) },
    { token: "string", foreground: hex(t.string) },
    { token: "string.escape", foreground: hex(t.escape) },
    { token: "string.invalid", foreground: hex(t.invalid), background: hex(D.red) },
    { token: "number", foreground: hex(t.number) },
    { token: "comment", foreground: hex(t.comment) },
    { token: "identifier", foreground: hex(t.text) },
    { token: "key", foreground: hex(t.key) },
    { token: "delimiter", foreground: hex(t.op) },
    { token: "predefined", foreground: hex(t.func) },
    { token: "type", foreground: hex(t.func) },
  ];
}

/** Popups (suggest, hover, find, menus): light-gray windows with black text, a green highlight. */
const popups = {
  "editorWidget.background": D.lightgray,
  "editorWidget.foreground": D.black,
  "editorWidget.border": D.black,
  "editorSuggestWidget.background": D.lightgray,
  "editorSuggestWidget.foreground": D.black,
  "editorSuggestWidget.border": D.black,
  "editorSuggestWidget.selectedBackground": D.green,
  "editorSuggestWidget.selectedForeground": D.black,
  "editorSuggestWidget.highlightForeground": D.blue,
  "editorSuggestWidget.focusHighlightForeground": D.black,
  "editorHoverWidget.background": D.lightgray,
  "editorHoverWidget.foreground": D.black,
  "editorHoverWidget.border": D.black,
  "editorHoverWidget.statusBarBackground": D.lightgray,
  "list.hoverBackground": D.white,
  "list.hoverForeground": D.black,
  "list.activeSelectionBackground": D.green,
  "list.activeSelectionForeground": D.black,
  "list.inactiveSelectionBackground": D.green,
  "list.inactiveSelectionForeground": D.black,
  "list.focusBackground": D.green,
  "list.focusForeground": D.black,
  "list.highlightForeground": D.blue,
  "menu.background": D.lightgray,
  "menu.foreground": D.black,
  "menu.selectionBackground": D.green,
  "menu.selectionForeground": D.black,
  "menu.separatorBackground": D.black,
  "menu.border": D.black,
  "input.background": D.black,
  "input.foreground": D.white,
  "input.border": D.black,
  "inputOption.activeBackground": D.cyan,
  "inputOption.activeForeground": D.black,
  "inputOption.activeBorder": D.black,
  "input.placeholderForeground": D.lightgray,
  "button.background": D.lightgray,
  "button.foreground": D.black,
  "focusBorder": D.white,
  "widget.shadow": D.black,
  "scrollbar.shadow": D.blue,
  "scrollbarSlider.background": D.lightgray,
  "scrollbarSlider.hoverBackground": D.white,
  "scrollbarSlider.activeBackground": D.white,
  "editorOverviewRuler.border": D.blue,
  "editorError.foreground": D.lightred,
  "editorWarning.foreground": D.yellow,
  "editorInfo.foreground": D.lightcyan,
  "editorLink.activeForeground": D.white,
  "editorCodeLens.foreground": D.lightgray,
  "editorLightBulb.foreground": D.yellow,
};

/** Colours in "dos16" that Monaco takes as colours: nothing translucent. */
const dosColors = {
  ...popups,
  "editor.background": D.blue,
  "editor.foreground": D.white,
  "editorGutter.background": D.blue,
  "editorLineNumber.foreground": D.lightgray,
  "editorLineNumber.activeForeground": D.yellow,
  "editorCursor.foreground": D.white,
  // Selected text keeps its own colour, so the selection is black: every bright
  // colour reads on it (light gray 9.0, light magenta 8.0, light green 15.8).
  "editor.selectionBackground": D.black,
  "editor.inactiveSelectionBackground": D.darkgray,
  "editor.lineHighlightBackground": D.blue,
  "editor.lineHighlightBorder": D.blue,
  "editor.wordHighlightBackground": D.cyan,
  "editor.wordHighlightStrongBackground": D.cyan,
  "editor.findMatchBackground": D.cyan,
  "editor.findMatchHighlightBackground": D.darkgray,
  "editorBracketMatch.background": D.darkgray,
  "editorBracketMatch.border": D.lightcyan,
  "editorIndentGuide.background1": D.lightblue,
  "editorIndentGuide.activeBackground1": D.lightcyan,
  "editorWhitespace.foreground": D.lightblue,
  "editorRuler.foreground": D.lightblue,
  "editorStickyScroll.background": D.blue,
};

/** A swatch as a Monaco theme: popups stay light-gray-window style, in the swatch's own window colours. */
function swatchTheme(sw: Swatch): monaco.editor.IStandaloneThemeData {
  const { ui: u, editor: e, syntax: y } = sw;
  const tones: Tones = { text: u.text, keyword: y.keyword, string: y.string, number: y.number, comment: y.comment, func: y.func, key: y.key, op: y.op, escape: y.escape, invalid: u.onerr };
  const colors = {
    "editor.background": u.bg,
    "editor.foreground": u.text,
    "editorGutter.background": e.gutter,
    "editorLineNumber.foreground": e.lineNumber,
    "editorLineNumber.activeForeground": e.lineNumberActive,
    "editorCursor.foreground": u.text,
    "editor.selectionBackground": e.sel,
    "editor.inactiveSelectionBackground": e.inactiveSel,
    "editor.lineHighlightBackground": e.lineTint,
    "editor.lineHighlightBorder": e.lineTint,
    "editor.wordHighlightBackground": e.word,
    "editor.wordHighlightStrongBackground": e.word,
    "editor.findMatchBackground": e.find,
    "editor.findMatchHighlightBackground": e.findOther,
    "editorBracketMatch.background": e.bracket,
    "editorBracketMatch.border": u.line,
    "editorIndentGuide.background1": e.guide,
    "editorIndentGuide.activeBackground1": e.guideActive,
    "editorWhitespace.foreground": e.guide,
    "editorRuler.foreground": e.guide,
    "editorStickyScroll.background": e.gutter,
    // Popups: the swatch's window colours, ink text, its menu highlight.
    "editorWidget.background": u.win, "editorWidget.foreground": u.ink, "editorWidget.border": u.ink,
    "editorSuggestWidget.background": u.win, "editorSuggestWidget.foreground": u.ink, "editorSuggestWidget.border": u.ink,
    "editorSuggestWidget.selectedBackground": u.menuhi, "editorSuggestWidget.selectedForeground": u.ink,
    "editorSuggestWidget.highlightForeground": u.winmuted, "editorSuggestWidget.focusHighlightForeground": u.ink,
    "editorHoverWidget.background": u.win, "editorHoverWidget.foreground": u.ink, "editorHoverWidget.border": u.ink,
    "editorHoverWidget.statusBarBackground": u.win,
    "list.hoverBackground": u.hover, "list.hoverForeground": u.ink,
    "list.activeSelectionBackground": u.menuhi, "list.activeSelectionForeground": u.ink,
    "list.inactiveSelectionBackground": u.menuhi, "list.inactiveSelectionForeground": u.ink,
    "list.focusBackground": u.menuhi, "list.focusForeground": u.ink, "list.highlightForeground": u.winmuted,
    "menu.background": u.win, "menu.foreground": u.ink, "menu.selectionBackground": u.menuhi,
    "menu.selectionForeground": u.ink, "menu.separatorBackground": u.ink, "menu.border": u.ink,
    "input.background": u.field, "input.foreground": u.fieldtext, "input.border": u.field,
    "inputOption.activeBackground": u.select, "inputOption.activeForeground": u.ink, "inputOption.activeBorder": u.ink,
    "input.placeholderForeground": u.fieldhint,
    "button.background": u.win, "button.foreground": u.ink,
    "focusBorder": u.focus, "widget.shadow": u.shadow, "scrollbar.shadow": u.bg,
    "scrollbarSlider.background": u.win, "scrollbarSlider.hoverBackground": u.hover, "scrollbarSlider.activeBackground": u.hover,
    "editorOverviewRuler.border": u.bg,
    "editorError.foreground": u.bad, "editorWarning.foreground": u.code, "editorInfo.foreground": u.strong,
    "editorLink.activeForeground": u.text, "editorCodeLens.foreground": u.muted, "editorLightBulb.foreground": u.code,
  };
  return { base: sw.dark ? "vs-dark" : "vs", inherit: false, rules: rules(tones), colors };
}

let defined = false;
/** Registers both themes once; safe to call again. */
export function defineThemes() {
  if (defined) return;
  defined = true;
  monaco.editor.defineTheme("joyride-dos16", { base: "vs-dark", inherit: false, rules: rules(dos), colors: dosColors });
  for (const sw of SWATCHES) monaco.editor.defineTheme("joyride-" + sw.id, swatchTheme(sw));
}

/** Options that make Monaco a text-mode screen: the VGA font on its 8 by 16 cells, block cursor, nothing smooth. */
export const TEXT_MODE_OPTIONS: monaco.editor.IStandaloneEditorConstructionOptions = {
  fontFamily: '"ansiapps VGA", ui-monospace, monospace',
  fontSize: 16,
  lineHeight: 16,
  letterSpacing: 0,
  fontWeight: "400",
  fontLigatures: false,
  cursorStyle: "block",
  cursorBlinking: "blink",
  cursorSmoothCaretAnimation: "off",
  smoothScrolling: false,
  roundedSelection: false,
  renderLineHighlight: "line",
  minimap: { enabled: false },
  overviewRulerLanes: 0,
  overviewRulerBorder: false,
  hideCursorInOverviewRuler: true,
  scrollBeyondLastLine: false,
  glyphMargin: false,
  lineDecorationsWidth: 16,
  lineNumbersMinChars: 4,
  folding: false,
  padding: { top: 0, bottom: 0 },
  scrollbar: { verticalScrollbarSize: 16, horizontalScrollbarSize: 16, useShadows: false, verticalHasArrows: false, horizontalHasArrows: false },
  stickyScroll: { enabled: false },
  occurrencesHighlight: "off",
  matchBrackets: "always",
  accessibilitySupport: "auto",
};
