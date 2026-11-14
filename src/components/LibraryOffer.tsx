import { useState } from "react";
import { librariesOfferAdd, librariesOfferDismiss, type LibraryOffer as Offer } from "../lib/backend";
import Panel from "./Panel";

/** Shown when a project is opened and it uses libraries JoyrIDE's list does not have: what was found and where, and the choice to add
 *  them to Libraries (your own list: they are not audited, and nothing is downloaded by adding them). Nothing of the project is run. */
export default function LibraryOffer(props: { offer: Offer; onClose: () => void; onAdded: (count: number) => void; onOpenLibraries: () => void }) {
  const { offer } = props;
  const [picked, setPicked] = useState<Set<string>>(() => new Set(offer.new.map((f) => f.name)));
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const toggle = (n: string) =>
    setPicked((p) => {
      const q = new Set(p);
      if (q.has(n)) q.delete(n);
      else q.add(n);
      return q;
    });

  const add = async () => {
    setBusy(true);
    setErr(null);
    try {
      const n = await librariesOfferAdd([...picked]);
      props.onAdded(n);
    } catch (e) {
      setErr(String(e));
      setBusy(false);
    }
  };

  const never = async () => {
    setBusy(true);
    try {
      await librariesOfferDismiss(offer.new.map((f) => f.name));
      props.onClose();
    } catch (e) {
      setErr(String(e));
      setBusy(false);
    }
  };

  return (
    <Panel title="Libraries found in this project" kind="panel-libraries" onClose={props.onClose}>
      <p className="hint">
        {offer.project} uses {offer.new.length} {offer.new.length === 1 ? "library" : "libraries"} that {offer.new.length === 1 ? "is" : "are"} not in Libraries.
        {offer.used.length > 0 && ` It also uses ${offer.used.map((u) => u.name).join(", ")}, which are.`} Adding puts them in your own list; nothing is downloaded and
        nothing is run.
      </p>
      {err && <pre role="alert" className="panel-error">{err}</pre>}
      <ul className="libraries-list">
        {offer.new.map((f) => (
          <li key={f.name}>
            <label className="lib-main">
              <input type="checkbox" checked={picked.has(f.name)} onChange={() => toggle(f.name)} disabled={busy} aria-label={`Add ${f.name} to Libraries`} /> <strong>{f.name}</strong>{" "}
              <span className="lib-meta">
                {[f.version, f.licence ?? "licence not found", f.git ?? "no source address"].filter(Boolean).join(" · ")}
              </span>
              {f.summary && <div>{f.summary}</div>}
              <div className="lib-meta">
                {f.how.join("; ")}
                {f.path ? ` (${f.path})` : ""}
                {f.required ? "; required by the project" : "; not required by any file"}
              </div>
              {!f.git && <div className="lib-meta lib-broken">No address was found for it, so it can be listed but not added to other projects from here.</div>}
            </label>
          </li>
        ))}
      </ul>
      {offer.unresolved.length > 0 && <p className="hint">Required but not provided by the project (not offered): {offer.unresolved.join(", ")}.</p>}
      <div className="panel-bar">
        <button disabled={busy || picked.size === 0} onClick={() => void add()}>Add {picked.size} to Libraries</button>
        <button disabled={busy} onClick={props.onClose}>Not now</button>
        <button disabled={busy} onClick={() => void never()} title="Do not offer these again for this project">Never for this project</button>
      </div>
    </Panel>
  );
}
