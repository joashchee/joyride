import { useEffect, useRef, useState } from "react";
import { searchProject, type Hit } from "../lib/backend";

/** Find in files: plain text over the project, results open the file at the line. */
export default function Search(props: { onOpen: (path: string, line: number) => void; focusKey: number }) {
  const [q, setQ] = useState("");
  const [matchCase, setMatchCase] = useState(false);
  const [hits, setHits] = useState<Hit[] | null>(null);
  const [more, setMore] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const seq = useRef(0);

  useEffect(() => {
    if (props.focusKey > 0) {
      input.current?.focus();
      input.current?.select();
    }
  }, [props.focusKey]);

  const go = async () => {
    const n = ++seq.current;
    if (!q) return setHits(null);
    try {
      const r = await searchProject(q, matchCase);
      if (n !== seq.current) return; // a newer search has started
      setHits(r.hits);
      setMore(r.truncated);
      setErr(null);
    } catch (e) {
      setErr(String(e));
    }
  };

  return (
    <section className="search" aria-label="Find in files">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void go();
        }}
      >
        <input ref={input} aria-label="Find in files" placeholder="Find in files" value={q} onChange={(e) => setQ(e.target.value)} spellCheck={false} />
        <label>
          <input type="checkbox" checked={matchCase} onChange={(e) => setMatchCase(e.target.checked)} /> Match case
        </label>
      </form>
      {err && <p className="empty" role="alert">{err}</p>}
      {hits && (
        <>
          <p className="search-count" role="status">
            {hits.length === 0 ? "No matches." : `${hits.length}${more ? "+" : ""} match${hits.length === 1 ? "" : "es"}`}
          </p>
          <ul className="search-hits" aria-label="Search results">
            {hits.map((h, i) => (
              <li key={i}>
                <button onClick={() => props.onOpen(h.path, h.line)} title={`${h.path}:${h.line}`}>
                  <span className="hit-where">{h.path}:{h.line}</span> <span className="hit-text">{h.text}</span>
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
