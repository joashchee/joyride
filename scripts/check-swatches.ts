// Checks every theme swatch (src/swatches.ts): each text and background pair
// the UI and the editor use must be 4.5:1 (WCAG AA, 1.4.3), by the same
// relative-luminance formula as docs/ansiapps-color-contrast.md.
// Run: node scripts/check-swatches.ts   (Node 22.6+ strips the types itself)
import { SWATCHES } from "../src/swatches.ts";

const lum = (h: string) => {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
const ratio = (a: string, b: string) => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
};

let bad = 0;
for (const s of SWATCHES) {
  const u = s.ui, y = s.syntax, e = s.editor;
  // [what, foreground, background, minimum]
  const pairs: [string, string, string, number][] = [
    ["text on desktop", u.text, u.bg, 4.5], ["muted on desktop", u.muted, u.bg, 4.5],
    ["code on desktop", u.code, u.bg, 4.5], ["strong on desktop", u.strong, u.bg, 4.5],
    ["bad on desktop", u.bad, u.bg, 4.5], ["close box on desktop", u.closebox, u.bg, 4.5],
    ["frame line on desktop", u.line, u.bg, 3],
    ["ink on window", u.ink, u.win, 4.5], ["dim on window", u.winmuted, u.win, 4.5],
    ["ink on selection", u.ink, u.select, 4.5], ["ink on menu highlight", u.ink, u.menuhi, 4.5],
    ["ink on hover", u.ink, u.hover, 4.5], ["text on row hover", u.text, u.rowhover, 4.5],
    ["field text", u.fieldtext, u.field, 4.5], ["field hint", u.fieldhint, u.field, 4.5],
    ["console text", u.consoletext, u.console, 4.5], ["console error", u.consoleerr, u.console, 4.5],
    ["console link", u.consolelink, u.console, 4.5], ["text on error bar", u.onerr, u.err, 4.5],
    ["fill on progress track", u.fill, u.console, 3],
    ...(["keyword", "string", "number", "comment", "func", "key", "op", "escape"] as const).flatMap((k): [string, string, string, number][] => [
      [`syntax ${k} on editor`, y[k], u.bg, 4.5], [`syntax ${k} on current line`, y[k], e.lineTint, 4.5], [`syntax ${k} on selection`, y[k], e.sel, 4.5],
    ]),
    ["editor text", u.text, u.bg, 4.5], ["editor text on selection", u.text, e.sel, 4.5],
    ["line number on gutter", e.lineNumber, e.gutter, 4.5], ["active line number on gutter", e.lineNumberActive, e.gutter, 4.5],
    ["text on word highlight", u.text, e.word, 4.5],
  ];
  for (const [what, fg, bg, min] of pairs) {
    const r = ratio(fg, bg);
    if (r < min) {
      bad++;
      console.log(`${s.id}: ${what} ${fg} on ${bg} = ${r.toFixed(2)}:1 (needs ${min})`);
    }
  }
}
console.log(bad === 0 ? `All ${SWATCHES.length} swatches pass.` : `${bad} pair(s) fail.`);
process.exit(bad === 0 ? 0 : 1);
