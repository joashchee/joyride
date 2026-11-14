import { useEffect, useState } from "react";
import { integrateSync, libraryAdd, libraryCustomRemove, libraryList, libraryRemove, type LibraryChange, type LibraryView } from "../lib/backend";
import Panel from "./Panel";

/** The curated libraries (Lua, and C libraries through FFI or as native modules): search, add at the pinned commit, remove. Adding needs git and the network, only when clicked. */
export default function Libraries(props: { onClose: () => void; onChanged: (c?: LibraryChange) => void; onScan: () => void; mainDirty: boolean }) {
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
      // main.lua with unsaved edits is never written under: the line is left for the Integrate button once it is saved.
      const c = await (l.installed ? libraryRemove : libraryAdd)(l.name, q, !props.mainDirty);
      setLibs(c.libraries);
      if (c.integrateError) setErr(`The library was ${l.installed ? "removed" : "added"}, but main.lua could not be updated: ${c.integrateError}`);
      props.onChanged(c);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  const integrate = async () => {
    setBusy("integrate");
    setErr(null);
    try {
      const s = await integrateSync();
      props.onChanged({ libraries: libs ?? [], synced: s, integrateError: null });
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  const forget = async (l: LibraryView) => {
    setBusy(l.name);
    setErr(null);
    try {
      setLibs(await libraryCustomRemove(l.name, q));
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
        <button onClick={props.onScan} title="Read this project's files for libraries that are not in this list">Find in this project</button>
        <button disabled={props.mainDirty || busy !== null} onClick={() => void integrate()} title={props.mainDirty ? "Save main.lua first: it has unsaved changes" : "Add or remove the require-path lines JoyrIDE keeps in main.lua for this project's libraries"}>Integrate</button>
      </div>
      {err && <pre role="alert" className="panel-error">{err}</pre>}
      <ul className="libraries-list">
        {libs?.map((l) => (
          <li key={l.name}>
            <button disabled={busy !== null || (l.broken && !l.installed) || (!l.installable && !l.installed)} onClick={() => void act(l)} aria-label={`${l.installed ? "Remove" : "Add"} ${l.name}`}>
              {busy === l.name ? "…" : l.installed ? "Remove" : "Add"}
            </button>
            {l.custom && (
              <button disabled={busy !== null || l.installed} onClick={() => void forget(l)} aria-label={`Remove ${l.name} from your own list`} title="Forget it: takes it out of this list, and changes no project">
                Forget
              </button>
            )}
            <div className="lib-main">
              <strong>{l.name}</strong> <span className="lib-meta">{l.version} · {l.licence} · {l.kind}{l.custom ? " · yours" : ""}</span>
              <div>{l.summary}</div>
              {l.native && <div className="lib-meta">Needs: {l.native}</div>}
              {l.caution && <div className="lib-meta lib-broken">Caution: {l.caution}</div>}
              {l.benchmark && <div className="lib-meta">Benchmark: {l.benchmark}</div>}
              <div className={"lib-meta" + (l.broken ? " lib-broken" : "")}>{l.badge}</div>
            </div>
          </li>
        ))}
      </ul>
      {libs?.length === 0 && <p className="hint">No library matches.</p>}
    </Panel>
  );
}
