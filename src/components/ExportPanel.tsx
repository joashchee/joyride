import { useEffect, useState } from "react";
import { exportInfo, exportProject, type ExportInfo } from "../lib/backend";
import Panel from "./Panel";

/** Export Center: builds into the project's build/ folder. Web, Android and iOS need downloaded templates and come later. */
export default function ExportPanel(props: { onClose: () => void }) {
  const [info, setInfo] = useState<ExportInfo | null>(null);
  const [variant, setVariant] = useState(false);
  const [commercial, setCommercial] = useState(false);
  const [accept, setAccept] = useState<string[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    exportInfo()
      .then((i) => (setInfo(i), setAccept(i.accepted)))
      .catch((e) => setErr(String(e)));
  }, []);

  const build = async (kind: "love" | "portmaster") => {
    setBusy(kind);
    setDone(null);
    setErr(null);
    try {
      setDone(await exportProject(kind, variant, accept, commercial));
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Panel title={info ? `Export, LÖVE ${info.loveVersion}` : "Export"} kind="panel-export" onClose={props.onClose}>
      {info?.variantOffered && (
        <label>
          <input type="checkbox" checked={variant} onChange={(e) => setVariant(e.target.checked)} /> Build the LÖVE 11.5 variant
        </label>
      )}
      {info?.variantOffered && variant && info.uses.length > 0 && (
        <fieldset className="variant-uses">
          <legend>12.0 functions this game uses</legend>
          {info.uses.map((u) => (
            <label key={u.api} className={"use-" + u.support}>
              {u.support === "unsupported" ? (
                <input type="checkbox" checked={accept.includes(u.api)} onChange={(e) => setAccept((a) => (e.target.checked ? [...a, u.api] : a.filter((x) => x !== u.api)))} aria-label={`Accept ${u.api} without an 11.5 equivalent`} />
              ) : (
                <span aria-hidden="true">✓</span>
              )}{" "}
              <code>{u.api}</code> <span className="lib-meta">{u.support}: {u.note} · {u.file}:{u.line}{u.count > 1 ? ` (+${u.count - 1})` : ""}</span>
            </label>
          ))}
          <p className="hint">Tick an unsupported function to build anyway, when the game guards it.</p>
        </fieldset>
      )}
      {info && info.unusable.length > 0 && (
        <fieldset className="variant-uses">
          <legend>Files LÖVE {info.loveVersion} cannot use</legend>
          {info.unusable.slice(0, 8).map((u) => (
            <p key={u} className="lib-meta">{u}</p>
          ))}
          {info.unusable.length > 8 && <p className="hint">… and {info.unusable.length - 8} more (they are in the Console when you Run).</p>}
          <p className="hint">The export still builds; remove or convert these if the game loads them.</p>
        </fieldset>
      )}
      <label>
        <input type="checkbox" checked={commercial} onChange={(e) => setCommercial(e.target.checked)} /> This game is sold (stops the build on an asset whose licence forbids sale)
      </label>
      <div className="row-buttons">
        <button disabled={busy !== null || (info?.variantOffered && variant)} onClick={() => void build("love")} title="A .love file for desktop LÖVE">
          {busy === "love" ? "Building…" : ".love file"}
        </button>
        <button disabled={busy !== null} onClick={() => void build("portmaster")} title="A PortMaster zip for handhelds (uses PortMaster's own LÖVE)">
          {busy === "portmaster" ? "Building…" : "PortMaster zip"}
        </button>
      </div>
      {info?.variantOffered && !variant && <p className="hint">PortMaster only has LÖVE 11.5: tick the variant to export this 12.0 project for it.</p>}
      {done && <p role="status">Built {done}</p>}
      {err && <pre role="alert" className="panel-error">{err}</pre>}
    </Panel>
  );
}
