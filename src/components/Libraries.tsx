import { useEffect, useState } from "react";
import { libraryAdd, libraryList, libraryRemove, type LibraryView } from "../lib/backend";

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
    <section className="libraries" aria-label="Libraries" onKeyDown={(e) => e.key === "Escape" && props.onClose()}>
      <header>
        <strong>Libraries</strong>
        <input aria-label="Search libraries" placeholder="Search" value={q} onChange={(e) => setQ(e.target.value)} spellCheck={false} autoFocus />
        <button onClick={props.onClose} aria-label="Close libraries">×</button>
      </header>
      {err && <pre role="alert" className="export-error">{err}</pre>}
      <ul>
        {libs?.map((l) => (
          <li key={l.name}>
            <div className="lib-main">
              <strong>{l.name}</strong> <span className="lib-meta">{l.version} · {l.licence}</span>
              <div>{l.summary}</div>
              <div className={"lib-meta" + (l.broken ? " lib-broken" : "")}>{l.badge}</div>
            </div>
            <button disabled={busy !== null || (l.broken && !l.installed)} onClick={() => void act(l)} aria-label={`${l.installed ? "Remove" : "Add"} ${l.name}`}>
              {busy === l.name ? "…" : l.installed ? "Remove" : "Add"}
            </button>
          </li>
        ))}
      </ul>
      {libs?.length === 0 && <p className="empty">No library matches.</p>}
    </section>
  );
}
