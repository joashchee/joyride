import { useState } from "react";
import { chooseFontFile, fontAdd, fontInspect, type FontPlan } from "../lib/backend";
import { FontSample } from "./Preview";
import Panel from "./Panel";

const LICENCES: { id: string; label: string }[] = [
  { id: "OFL-1.1", label: "SIL Open Font License 1.1 (most free fonts)" },
  { id: "Apache-2.0", label: "Apache 2.0" },
  { id: "MIT", label: "MIT" },
  { id: "CC0-1.0", label: "CC0 (public domain)" },
  { id: "CC-BY-4.0", label: "Creative Commons Attribution 4.0" },
  { id: "CC-BY-SA-4.0", label: "Creative Commons Attribution-ShareAlike 4.0" },
  { id: "LicenseRef-Custom", label: "Another licence I hold (say which)" },
  { id: "unknown", label: "I do not know the licence (say why it is used)" },
];

const NAME_KEY = "joyride.confirmedBy";
const remembered = () => {
  try {
    return localStorage.getItem(NAME_KEY) ?? "";
  } catch {
    return "";
  }
};

/** Add a font to the project: choose the file, see what it says about itself
 *  and its licence, confirm the licence by name, and it is copied into the
 *  project (never over anything) and recorded with its licence. */
export default function AddFont(props: { onClose: () => void; onAdded: (path: string) => void }) {
  const [plan, setPlan] = useState<FontPlan | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [licence, setLicence] = useState<string>("");
  const [by, setBy] = useState(remembered);
  const [note, setNote] = useState("");
  const [author, setAuthor] = useState("");
  const [url, setUrl] = useState("");
  const [attribution, setAttribution] = useState("");
  const [dest, setDest] = useState("");

  const choose = async () => {
    setError(null);
    try {
      const f = await chooseFontFile();
      if (!f) return;
      const p = await fontInspect(f);
      setPlan(p);
      setLicence(p.guess?.spdx ?? "");
      setAuthor(p.designer);
      setDest(p.suggestedDest);
      setNote("");
    } catch (e) {
      setPlan(null);
      setError(String(e));
    }
  };

  const options = plan?.guess && !LICENCES.some((l) => l.id === plan.guess!.spdx) ? [{ id: plan.guess.spdx, label: plan.guess.spdx }, ...LICENCES] : LICENCES;
  const needsNote = licence === "LicenseRef-Custom" || licence === "unknown";
  const ready = !!plan && !plan.alreadyInProject && !!licence && by.trim() !== "" && dest.trim() !== "" && (!needsNote || note.trim() !== "") && !busy;

  const add = async () => {
    if (!plan || !ready) return;
    setBusy(true);
    setError(null);
    try {
      try {
        localStorage.setItem(NAME_KEY, by.trim());
      } catch {
        /* remembering the name is a convenience */
      }
      const r = await fontAdd({ source: plan.source, dest: dest.trim(), licence, confirmedBy: by.trim(), note: note.trim(), author: author.trim(), sourceUrl: url.trim(), attribution: attribution.trim() });
      props.onAdded(r.dest);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Panel title="Add font" kind="panel-font" onClose={props.onClose}>
      <div className="panel-bar">
        <button onClick={() => void choose()}>Choose font file…</button>
        {plan && <span>{plan.fileName}</span>}
      </div>
      {error && <p className="panel-error" role="alert">{error}</p>}
      {!plan && !error && <p>Pick a TrueType, OpenType, Collection, WOFF or WOFF2 file. It is copied into the project, and its licence is recorded.</p>}
      {plan && (
        <>
          <dl className="preview-facts">
            {plan.facts.map((f) => (
              <div key={f.label} className="preview-fact">
                <dt>{f.label}</dt>
                <dd>{f.value}</dd>
              </div>
            ))}
          </dl>
          {plan.warnings.map((w) => (
            <p key={w} className="panel-error" role="alert">{w}</p>
          ))}
          {plan.dataUrl && <FontSample key={plan.source} url={plan.dataUrl} />}
          {plan.alreadyInProject && <p role="status">This font is already in the project as {plan.alreadyInProject}.</p>}
          <fieldset>
            <legend>Licence</legend>
            {plan.guess ? <p role="status">Looks like {plan.guess.spdx}, from {plan.guess.from}. Check it before confirming.</p> : <p role="status">The font does not say what its licence is. Find it where you got the font.</p>}
            {plan.licenceText && <p>The font says: {plan.licenceText}</p>}
            {plan.licenceUrl && <p>Licence address in the font: {plan.licenceUrl}</p>}
            <div role="radiogroup" aria-label="Licence" className="licence-choices">
              {options.map((l) => (
                <button key={l.id} role="radio" aria-checked={licence === l.id} className={licence === l.id ? "selected" : ""} onClick={() => setLicence(l.id)}>
                  {licence === l.id ? "(•)" : "( )"} {l.label}
                </button>
              ))}
            </div>
            {needsNote && (
              <label>
                {licence === "unknown" ? "Why is it used without a known licence?" : "Which licence, and what it allows"}
                <input value={note} onChange={(e) => setNote(e.target.value)} />
              </label>
            )}
          </fieldset>
          <label>
            Author
            <input value={author} onChange={(e) => setAuthor(e.target.value)} />
          </label>
          <label>
            Where it came from (web address)
            <input value={url} onChange={(e) => setUrl(e.target.value)} />
          </label>
          <label>
            Credit line, if the licence asks for one
            <input value={attribution} onChange={(e) => setAttribution(e.target.value)} />
          </label>
          <label>
            Put it at
            <input value={dest} onChange={(e) => setDest(e.target.value)} />
          </label>
          <label>
            I confirm this licence (your name)
            <input value={by} onChange={(e) => setBy(e.target.value)} />
          </label>
          <div className="row-buttons">
            <button onClick={() => void add()} disabled={!ready}>Add to project</button>
          </div>
        </>
      )}
    </Panel>
  );
}
