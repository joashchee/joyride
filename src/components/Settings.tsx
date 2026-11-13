import { useState } from "react";
import Panel from "./Panel";
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
      <p className="credit">Text: IBM VGA 8x16 from The Ultimate Oldschool PC Font Pack by VileR, int10h.org, CC BY-SA 4.0.</p>
    </Panel>
  );
}
