import { useEffect, useState } from "react";
import { sampleCopy, sampleList, type SampleInfo } from "../lib/backend";
import Panel from "./Panel";

/** The sample projects that ship with the app. Opening one copies it to a folder the person owns (the app-data folder) and opens that
 *  copy, so nothing shipped is ever changed; a copy already there is opened as it is, with its edits. */
export default function Samples(props: { onClose: () => void; onOpen: (dir: string) => void }) {
  const [list, setList] = useState<SampleInfo[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    sampleList()
      .then((l) => live && setList(l))
      .catch((e) => live && setErr(String(e)));
    return () => {
      live = false;
    };
  }, []);

  const open = async (s: SampleInfo, fresh: boolean) => {
    setBusy(s.id);
    setErr(null);
    try {
      props.onOpen(await sampleCopy(s.id, fresh));
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Panel title="Samples" kind="panel-libraries" onClose={props.onClose}>
      {err && <pre role="alert" className="panel-error">{err}</pre>}
      <ul className="libraries-list">
        {list?.map((s) => (
          <li key={s.id}>
            <button disabled={busy !== null} onClick={() => void open(s, false)} aria-label={`Open ${s.name}`}>
              {busy === s.id ? "…" : "Open"}
            </button>
            <button disabled={busy !== null} onClick={() => void open(s, true)} aria-label={`Open a fresh copy of ${s.name}, keeping the old copy`} title="Copy it again; the old copy is kept beside it">
              Fresh
            </button>
            <div className="lib-main">
              <strong>{s.name}</strong>
              <div>{s.summary}</div>
            </div>
          </li>
        ))}
      </ul>
      {list?.length === 0 && <p className="hint">No samples in this build.</p>}
      <p className="hint">Opening copies a sample to a folder of yours first. Fresh copies it again and keeps the old copy beside it.</p>
    </Panel>
  );
}
