import { useEffect, useState } from "react";
import { libraryAdd, libraryList, libraryRemove, type LibraryView } from "../lib/backend";
import Panel from "./Panel";

/** The curated libraries: search, add at the pinned commit, remove. Adding needs git and the network, only when clicked. */
export default function Libraries(props: { onClose: () => void; onChanged: () => void }) {
  const [q, setQ] = useState("");
  const [libs, setLibs] = useState<LibraryView[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    libraryList(q)
      .then((l) => live && (setLibs(l), setErr(null)))
      .catch((e) => live && setErr(String(e)));
    return () => {
      live = false;
    };
  }, [q]);

  const act = async (l: LibraryView) => {
    setBusy(l.name);
    setErr(null);
    try {
      setLibs(await (l.installed ? libraryRemove : libraryAdd)(l.name, q));
      props.onChanged();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Panel title="Libraries" kind="panel-libraries" onClose={props.onClose}>
      <div className="panel-bar">
        <input aria-label="Search libraries" placeholder="Search" value={q} onChange={(e) => setQ(e.target.value)} spellCheck={false} autoFocus />
      </div>
      {err && <pre role="alert" className="panel-error">{err}</pre>}
      <ul className="libraries-list">
        {libs?.map((l) => (
          <li key={l.name}>
            <button disabled={busy !== null || (l.broken && !l.installed)} onClick={() => void act(l)} aria-label={`${l.installed ? "Remove" : "Add"} ${l.name}`}>
              {busy === l.name ? "…" : l.installed ? "Remove" : "Add"}
            </button>
            <div className="lib-main">
              <strong>{l.name}</strong> <span className="lib-meta">{l.version} · {l.licence}</span>
              <div>{l.summary}</div>
              <div className={"lib-meta" + (l.broken ? " lib-broken" : "")}>{l.badge}</div>
            </div>
          </li>
        ))}
      </ul>
      {libs?.length === 0 && <p className="hint">No library matches.</p>}
    </Panel>
  );
}
