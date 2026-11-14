import { useEffect, useState } from "react";
import { soundfontList, soundfontPlan, soundfontUse, type SoundChoice, type SoundfontOption, type SoundfontPlan } from "../lib/backend";

const LABEL = { blocker: "Stops it", warning: "Warning", info: "Note" } as const;

/** The General MIDI SoundFonts that ship with JoyrIDE, as a source for the open project: one click shows what adding one would do and
 *  what to know first (the edge cases), a second adds it with its licence, recorded as confirmed, and shows the Lua that plays it. */
export default function ShippedSoundfonts(props: { hasProject: boolean; onAdded: (path: string) => void }) {
  const [fonts, setFonts] = useState<SoundChoice | null>(null);
  const [plan, setPlan] = useState<SoundfontPlan | null>(null);
  const [done, setDone] = useState<SoundfontPlan | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let live = true;
    soundfontList()
      .then((s) => live && setFonts(s))
      .catch((e) => live && setErr(String(e)));
    return () => {
      live = false;
    };
  }, []);

  const look = async (f: SoundfontOption) => {
    setBusy(f.id);
    setErr(null);
    setDone(null);
    try {
      setPlan(await soundfontPlan(f.id));
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  const add = async () => {
    if (!plan) return;
    setBusy(plan.id);
    setErr(null);
    try {
      const v = await soundfontUse(plan.id);
      setDone(v);
      setPlan(null);
      props.onAdded(v.dest);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      setCopied(false);
      setErr("The clipboard is not available here: select the Lua above and copy it by hand.");
    }
  };

  return (
    <div>
      <p className="hint">General MIDI SoundFonts that ship with JoyrIDE. Adding one copies it into this project with its licence and records it; nothing is downloaded.</p>
      {err && <pre role="alert" className="panel-error">{err}</pre>}
      {!props.hasProject && <p className="hint">Open a project first: a SoundFont is added to the project that is open.</p>}
      <ul className="libraries-list">
        {fonts?.fonts
          .filter((f) => f.id !== "builtin")
          .map((f) => (
            <li key={f.id}>
              <button disabled={!props.hasProject || busy !== null || !f.available} onClick={() => void look(f)} aria-label={`Use ${f.name} in this project`}>
                {busy === f.id ? "…" : "Use in project…"}
              </button>
              <div className="lib-main">
                <strong>{f.name}</strong> <span className="lib-meta">{Math.round(f.size / 1024)} KB · {f.licence} · {f.author}</span>
                <div>{f.summary}</div>
                {!f.available && <div className="lib-meta lib-broken">Not in this build of JoyrIDE.</div>}
              </div>
            </li>
          ))}
      </ul>
      {plan && (
        <section aria-label={`Adding ${plan.name}`}>
          <p>
            <strong>{plan.name}</strong> would go to <code>{plan.dest}</code>, with its licence beside it as <code>{plan.licenceDest}</code>.
          </p>
          <ul role="list">
            {plan.warnings.map((w, i) => (
              <li key={i} className={w.level === "blocker" ? "lib-meta lib-broken" : "lib-meta"}>
                <strong>{LABEL[w.level]}:</strong> {w.text}
              </li>
            ))}
          </ul>
          <div className="panel-bar">
            <button disabled={plan.blocked || busy !== null} onClick={() => void add()}>
              {plan.alreadyThere ? "Already there: make sure its licence is too" : plan.warnings.some((w) => w.level === "warning") ? "Add anyway" : "Add to project"}
            </button>
            <button disabled={busy !== null} onClick={() => setPlan(null)}>Cancel</button>
          </div>
        </section>
      )}
      {done && (
        <section aria-label={`${done.name} added`}>
          <p>
            <strong>{done.name}</strong> is in the project at <code>{done.dest}</code>, its licence at <code>{done.licenceDest}</code>. Ship both.
          </p>
          <ul role="list">
            {done.warnings.filter((w) => w.level === "warning").map((w, i) => (
              <li key={i} className="lib-meta">
                <strong>Remember:</strong> {w.text}
              </li>
            ))}
          </ul>
          <p className="hint">To play a song through it (needs neumetik-midi):</p>
          <pre aria-label="Lua that plays a song through the SoundFont">{done.snippet}</pre>
          <div className="panel-bar">
            <button onClick={() => void copy(done.snippet)}>{copied ? "Copied" : "Copy the Lua"}</button>
          </div>
        </section>
      )}
    </div>
  );
}
