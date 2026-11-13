import { useEffect, useState } from "react";
import { autoportGenerate, autoportScan, type PortReport } from "../lib/backend";

const LEVEL = { blocker: "Will not work", warning: "Check", info: "Handled" } as const;

/** Auto-Port: what the scanner finds for each target, and the overlay under .joyride/patches/ (the project's own files are never changed). */
export default function AutoPort(props: { onClose: () => void; onOpen: (path: string, line: number) => void; onChanged: () => void }) {
  const [rep, setRep] = useState<PortReport | null>(null);
  const [target, setTarget] = useState("portmaster");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    autoportScan().then(setRep).catch((e) => setErr(String(e)));
  }, []);

  const t = rep?.targets.find((x) => x.name === target);
  const generate = async () => {
    setBusy(true);
    setErr(null);
    try {
      setRep(await autoportGenerate(target));
      props.onChanged();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="autoport" aria-label="Auto-Port" onKeyDown={(e) => e.key === "Escape" && props.onClose()}>
      <header>
        <strong>Auto-Port</strong>
        <div role="tablist" aria-label="Target">
          {rep?.targets.map((x) => (
            <button key={x.name} role="tab" aria-selected={x.name === target} className={x.name === target ? "active" : ""} onClick={() => setTarget(x.name)}>
              {x.title}
              {x.blockers > 0 ? ` (${x.blockers})` : ""}
            </button>
          ))}
        </div>
        <button onClick={props.onClose} aria-label="Close Auto-Port">×</button>
      </header>
      {err && <pre role="alert" className="export-error">{err}</pre>}
      {rep && (
        <p className="hint">
          {rep.files} Lua file{rep.files === 1 ? "" : "s"} · input:{" "}
          {[rep.keyboard && "keyboard", rep.mouse && "mouse", rep.touch && "touch", rep.gamepad && "gamepad"].filter(Boolean).join(", ") || "none found"}
          {rep.keys.length > 0 && ` · keys: ${rep.keys.slice(0, 12).join(" ")}${rep.keys.length > 12 ? " …" : ""}`}
        </p>
      )}
      {t && (
        <>
          {t.findings.length === 0 && <p>Nothing to adapt for this target.</p>}
          <ul className="port-findings" aria-label="Findings">
            {t.findings.map((f, i) => (
              <li key={i} className={"port-" + f.level}>
                <button onClick={() => props.onOpen(f.file, f.line)}>
                  <strong>{LEVEL[f.level]}</strong> {f.file}:{f.line} <span className="lib-meta">{f.rule}</span>
                  <div>{f.message}</div>
                </button>
              </li>
            ))}
          </ul>
          <div className="export-buttons">
            <button disabled={busy} onClick={() => void generate()}>{t.overlayFiles.length ? "Refresh overlay" : "Generate overlay"}</button>
            {t.overlayFiles.map((f) => (
              <button key={f} onClick={() => props.onOpen(f, 1)} title={f}>{f.split("/").pop()}</button>
            ))}
          </div>
          <p className="hint">The overlay lives in .joyride/patches/{t.name}/ and is merged into a copy at build time. Your source is not changed.</p>
        </>
      )}
    </section>
  );
}
