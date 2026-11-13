import { useEffect, useState } from "react";
import { docsGet, docsSearch, type DocFound } from "../lib/backend";

/** The offline LÖVE reference: search by name (fuzzy), read the signatures and description. */
export default function Docs(props: { onClose: () => void; focusKey: number }) {
  const [q, setQ] = useState("");
  const [version, setVersion] = useState("");
  const [results, setResults] = useState<DocFound[]>([]);
  const [open, setOpen] = useState<string | null>(null);
  const [detail, setDetail] = useState<{ text: string; snippet: string | null } | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    if (!q.trim()) {
      setResults([]);
      return;
    }
    docsSearch(q)
      .then((r) => live && (setResults(r.results), setVersion(r.version), setErr(null)))
      .catch((e) => live && setErr(String(e)));
    return () => {
      live = false;
    };
  }, [q]);

  useEffect(() => {
    if (!open) return setDetail(null);
    let live = true;
    docsGet(open).then((d) => live && setDetail(d)).catch((e) => live && setErr(String(e)));
    return () => {
      live = false;
    };
  }, [open]);

  return (
    <section className="docs" aria-label="LÖVE reference" onKeyDown={(e) => e.key === "Escape" && props.onClose()}>
      <header>
        <strong>LÖVE reference</strong>
        {version && <span className="lib-meta"> {version}</span>}
        <input key={props.focusKey} aria-label="Search the LÖVE reference" placeholder="Search, e.g. draw or newImage" value={q} onChange={(e) => setQ(e.target.value)} spellCheck={false} autoFocus />
        <button onClick={props.onClose} aria-label="Close reference">×</button>
      </header>
      {err && <pre role="alert" className="export-error">{err}</pre>}
      <div className="docs-body">
        <ul aria-label="Results">
          {results.map((r) => (
            <li key={r.name}>
              <button className={r.name === open ? "active" : ""} onClick={() => setOpen(r.name)} title={r.summary}>
                {r.name} <span className="lib-meta">{r.kind}</span>
              </button>
            </li>
          ))}
        </ul>
        {detail && (
          <article aria-label={open ?? ""}>
            <pre className="docs-text">{detail.text.replace(/```lua\n|```/g, "")}</pre>
            {detail.snippet && (
              <>
                <strong>Snippet</strong>
                <pre className="docs-text">{detail.snippet.replace(/\$\{\d+:([^}]*)\}/g, "$1").replace(/\$0/g, "")}</pre>
              </>
            )}
          </article>
        )}
      </div>
      {q.trim() && results.length === 0 && !err && <p className="empty">Nothing matches.</p>}
    </section>
  );
}
