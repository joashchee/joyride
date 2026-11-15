import { useState } from "react";
import type { Draft, ReportInfo } from "../lib/backend";
import Panel from "./Panel";

const when = (secs: number) => new Date(secs * 1000).toLocaleString();

/** After a crash or a quit that left edits behind: the unsaved edits JoyrIDE kept as drafts, to put back in their tabs or throw away.
 *  Restoring only fills the tab (nothing is written to the file until you save); closing the panel keeps every draft for later. */
export default function Recovery(props: {
  drafts: Draft[];
  /** The last session did not exit cleanly: the report that was written for it. */
  crash: ReportInfo | null;
  onRestore: (d: Draft) => Promise<void>;
  onDiscard: (d: Draft) => Promise<void>;
  onClose: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const each = async (list: Draft[], f: (d: Draft) => Promise<void>) => {
    setBusy(true);
    setErr(null);
    try {
      for (const d of list) await f(d);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };
  const { drafts, crash } = props;
  return (
    <Panel title="Recovery" kind="panel-libraries" onClose={props.onClose}>
      {crash && (
        <p role="status">
          JoyrIDE did not close properly last time. A report was saved in Settings, under Diagnostics; nothing was sent anywhere.
        </p>
      )}
      {err && <pre role="alert" className="panel-error">{err}</pre>}
      {drafts.length > 0 && (
        <>
          <p>
            {drafts.length} {drafts.length === 1 ? "file has" : "files have"} edits that were never saved. Restore puts the edit back in the file's tab; the file itself changes only when you save.
          </p>
          <div className="panel-bar">
            <button disabled={busy} onClick={() => void each(drafts, props.onRestore)}>Restore all</button>
            <button disabled={busy} onClick={() => void each(drafts, props.onDiscard)} title="Throw the unsaved edits away">Discard all</button>
          </div>
          <ul className="libraries-list">
            {drafts.map((d) => (
              <li key={d.path}>
                <button disabled={busy} onClick={() => void each([d], props.onRestore)} aria-label={`Restore ${d.path}`}>Restore</button>
                <button disabled={busy} onClick={() => void each([d], props.onDiscard)} aria-label={`Discard the edits to ${d.path}`}>Discard</button>
                <div className="lib-main">
                  <strong>{d.path}</strong>
                  <div>
                    kept {when(d.savedAt)}
                    {d.missing ? " · the file is gone from disk; Restore makes it again" : ""}
                  </div>
                </div>
              </li>
            ))}
          </ul>
        </>
      )}
      {drafts.length === 0 && <p className="hint">Nothing was left unsaved.</p>}
      <p className="hint">Closing this keeps the drafts: they are offered again the next time this project opens.</p>
    </Panel>
  );
}
