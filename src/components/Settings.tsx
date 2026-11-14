import { useState } from "react";
import Panel from "./Panel";
import { CODE_FONTS, VGA_ID, cleanSizes, fontById, loadChoice, nearestSize, saveChoice, sizeHint, type CodeChoice } from "../lib/codefonts";
import { applyAppearance, currentAppearance, loadSwatch, PALETTES, SWATCHES, type Palette } from "../lib/theme";

/** The appearance options, all of them colour: the palette, and in True
 *  colour the theme swatch. The UI is always the text-mode look; there is no
 *  graphical theme to switch to. */
export default function Settings(props: { onClose: () => void }) {
  const [palette, setPalette] = useState<Palette>(() => currentAppearance().palette);
  // The swatch is remembered while DOS mode is on, so switching back restores it.
  const [swatchId, setSwatchId] = useState(() => loadSwatch().id);
  const choosePalette = (p: Palette) => {
    applyAppearance(p, SWATCHES.find((s) => s.id === swatchId) ?? SWATCHES[0]);
    setPalette(p);
  };
  const chooseSwatch = (id: string) => {
    applyAppearance("truecolor", SWATCHES.find((s) => s.id === id) ?? SWATCHES[0]);
    setPalette("truecolor");
    setSwatchId(id);
  };
  const [code, setCode] = useState<CodeChoice>(loadChoice);
  const dpr = window.devicePixelRatio || 1;
  const font = fontById(code.id);
  const sizes = font ? cleanSizes(font, dpr) : [16];
  const idx = Math.max(0, sizes.indexOf(code.size));
  const chooseCode = (c: CodeChoice) => {
    saveChoice(c);
    setCode(c);
  };
  const chooseFont = (id: string) => {
    const f = fontById(id);
    // Keep about the same size on the new font: the nearest one it renders cleanly.
    chooseCode({ id, size: f ? nearestSize(cleanSizes(f, dpr), code.size) : 16 });
  };
  return (
    <Panel title="Settings" kind="panel-settings" onClose={props.onClose}>
      <fieldset className="palette-choice">
        <legend>Colour palette</legend>
        {PALETTES.map((p) => (
          <label key={p.id}>
            <input type="radio" name="palette" checked={palette === p.id} onChange={() => choosePalette(p.id)} /> {p.label}
            <span className="lib-meta">{p.note}</span>
          </label>
        ))}
      </fieldset>
      <fieldset className="palette-choice" disabled={palette !== "truecolor"}>
        <legend>Theme swatch{palette !== "truecolor" ? " (True colour only)" : ""}</legend>
        {SWATCHES.map((s) => (
          <label key={s.id} className="swatch-row">
            <input type="radio" name="swatch" checked={swatchId === s.id} onChange={() => chooseSwatch(s.id)} /> {s.label}
            <span className="swatch-chips" aria-hidden="true">
              {[s.ui.bg, s.ui.text, s.ui.win, s.syntax.keyword, s.syntax.string, s.syntax.number].map((c, i) => (
                <span key={i} className="chip" style={{ background: c }} />
              ))}
            </span>
            <span className="lib-meta">{s.dark ? "dark" : "light"}</span>
          </label>
        ))}
      </fieldset>
      <fieldset className="palette-choice">
        <legend>Editor font</legend>
        <label>
          <input type="radio" name="codefont" checked={code.id === VGA_ID} onChange={() => chooseFont(VGA_ID)} /> IBM VGA 8x16
          <span className="lib-meta">the text-mode look, as the rest of the screen (default)</span>
        </label>
        {CODE_FONTS.map((f) => (
          <label key={f.id}>
            <input type="radio" name="codefont" checked={code.id === f.id} onChange={() => chooseFont(f.id)} /> {f.name}
            <span className="lib-meta">{f.note}</span>
          </label>
        ))}
      </fieldset>
      <div className="font-size-row" role="group" aria-label="Editor font size">
        <button disabled={code.id === VGA_ID || idx <= 0} onClick={() => chooseCode({ id: code.id, size: sizes[idx - 1] })} aria-label="Smaller editor font">[ - ]</button>
        <span aria-live="polite">{code.size} px ({sizeHint(code.size)})</span>
        <button disabled={code.id === VGA_ID || idx >= sizes.length - 1} onClick={() => chooseCode({ id: code.id, size: sizes[idx + 1] })} aria-label="Larger editor font">[ + ]</button>
        <span className="lib-meta">
          {code.id === VGA_ID ? "fixed at 16: the grid's own size" : font?.kind === "bitmap" ? "whole multiples of the size it was drawn for: every pixel sharp" : "sizes where every character is a whole number of screen pixels wide"}
        </span>
      </div>
      <p className="lib-meta">The window snaps to sizes that show whole characters and whole lines in this font.</p>
      {font && (
        <p className="credit">
          Editor font: {font.name} by {font.author}, {font.licence}, {font.source}. Unmodified; licence in the app's fonts folder.
        </p>
      )}
      <p className="credit">Text: IBM VGA 8x16 from The Ultimate Oldschool PC Font Pack by VileR, int10h.org, CC BY-SA 4.0.</p>
    </Panel>
  );
}
