import { useCallback, useEffect, useState } from "react";
import {
  assetsConfirm,
  assetsImportApply,
  assetsImportPlan,
  assetsSearch,
  assetsUse,
  type AssetList,
  type AssetView,
  type ImportPlanView,
  type ImportReportView,
} from "../lib/backend";
import Panel from "./Panel";

export const LICENCES: { id: string; label: string }[] = [
  { id: "CC0-1.0", label: "CC0 (public domain)" },
  { id: "MIT", label: "MIT" },
  { id: "Apache-2.0", label: "Apache 2.0" },
  { id: "Zlib", label: "zlib" },
  { id: "OFL-1.1", label: "SIL Open Font License 1.1" },
  { id: "CC-BY-4.0", label: "CC BY 4.0" },
  { id: "CC-BY-SA-4.0", label: "CC BY-SA 4.0" },
  { id: "CC-BY-NC-4.0", label: "CC BY-NC 4.0 (not for sale)" },
  { id: "LicenseRef-AllRightsReserved", label: "All rights reserved" },
  { id: "LicenseRef-Custom", label: "Another licence (say which)" },
  { id: "unknown", label: "Unknown (say why it is used)" },
];

const NAME_KEY = "joyride.confirmedBy";
const remembered = () => {
  try {
    return localStorage.getItem(NAME_KEY) ?? "";
  } catch {
    return "";
  }
};
const remember = (n: string) => {
  try {
    localStorage.setItem(NAME_KEY, n);
  } catch {
    /* a convenience only */
  }
};

const KINDS = ["", "image", "audio", "font", "shader", "data", "video", "archive"];
const PAGE = 6;
const size = (n: number) => (n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} kB`);
const needsNote = (l: string) => l === "unknown" || l === "LicenseRef-Custom";
const options = (extra?: string | null) => (extra && !LICENCES.some((l) => l.id === extra) ? [{ id: extra, label: extra }, ...LICENCES] : LICENCES);

function licenceLine(a: AssetView): string {
  if (a.status === "confirmed") {
    const sale = a.commercial === "yes" ? "can be sold" : a.commercial === "no" ? "not for sale" : "sale not stated";
    return `${a.spdx}, confirmed by ${a.confirmedBy}; ${sale}${a.attributionRequired ? "; credit required" : ""}${a.shareAlike ? "; share-alike" : ""}`;
  }
  return a.status === "guessed" ? `${a.spdx || "licence"} guessed, not confirmed: confirm it to use this` : "licence unknown: confirm it to use this";
}

function Pager(props: { page: number; pages: number; set: (n: number) => void }) {
  if (props.pages <= 1) return null;
  return (
    <div className="panel-bar" role="navigation" aria-label="Pages">
      <button onClick={() => props.set(Math.max(0, props.page - 1))} disabled={props.page === 0}>Prev</button>
      <span>Page {props.page + 1}/{props.pages}</span>
      <button onClick={() => props.set(Math.min(props.pages - 1, props.page + 1))} disabled={props.page >= props.pages - 1}>Next</button>
    </div>
  );
}

/** The library: every asset with its licence, preview and uses. An asset is
 *  used in a project only once its licence is confirmed by a named person. */
function Browse(props: { hasProject: boolean; onAdded: (path: string) => void }) {
  const [q, setQ] = useState("");
  const [kind, setKind] = useState("");
  const [confirmedOnly, setConfirmedOnly] = useState(false);
  const [commercialOnly, setCommercialOnly] = useState(false);
  const [list, setList] = useState<AssetList | null>(null);
  const [page, setPage] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [acting, setActing] = useState<null | { id: number; mode: "use" | "confirm" }>(null);
  const [dest, setDest] = useState("");
  const [licence, setLicence] = useState("");
  const [by, setBy] = useState(remembered);
  const [note, setNote] = useState("");
  const [author, setAuthor] = useState("");
  const [url, setUrl] = useState("");
  const [credit, setCredit] = useState("");
  const [rev, setRev] = useState(0);

  useEffect(() => {
    let live = true;
    assetsSearch(q, kind, confirmedOnly, commercialOnly)
      .then((r) => live && (setList(r), setErr(null), setPage(0)))
      .catch((e) => live && setErr(String(e)));
    return () => {
      live = false;
    };
  }, [q, kind, confirmedOnly, commercialOnly, rev]);

  const assets = list?.assets ?? [];
  const pages = Math.max(1, Math.ceil(assets.length / PAGE));
  const shown = assets.slice(page * PAGE, page * PAGE + PAGE);

  const begin = (a: AssetView, mode: "use" | "confirm") => {
    setActing({ id: a.id, mode });
    setMsg(null);
    setErr(null);
    setDest(`${a.kind === "other" ? "assets" : a.kind + "s"}/${a.name}`);
    setLicence(a.spdx || "");
    setNote("");
    setAuthor(a.author);
    setUrl(a.sourceUrl);
    setCredit("");
  };

  const submit = async () => {
    if (!acting) return;
    try {
      if (acting.mode === "use") {
        const p = await assetsUse(acting.id, dest.trim());
        setMsg(`Added ${p}.`);
        setActing(null);
        setRev((r) => r + 1);
        props.onAdded(p);
      } else {
        remember(by.trim());
        await assetsConfirm({ id: acting.id, licence, confirmedBy: by.trim(), note: note.trim(), author: author.trim(), sourceUrl: url.trim(), attribution: credit.trim() });
        setMsg("Licence confirmed.");
        setActing(null);
        setRev((r) => r + 1);
      }
      setErr(null);
    } catch (e) {
      setErr(String(e));
    }
  };

  const ready = acting?.mode === "use" ? dest.trim() !== "" : !!licence && by.trim() !== "" && (!needsNote(licence) || note.trim() !== "");

  return (
    <>
      <div className="panel-bar">
        <input aria-label="Search assets" placeholder="Search name, tag, licence, author" value={q} onChange={(e) => setQ(e.target.value)} spellCheck={false} autoFocus />
      </div>
      <fieldset className="panel-bar">
        <legend>Kind</legend>
        {KINDS.map((k) => (
          <label key={k}>
            <input type="radio" name="asset-kind" checked={kind === k} onChange={() => setKind(k)} /> {k || "all"}{" "}
          </label>
        ))}
      </fieldset>
      <div className="panel-bar">
        <label><input type="checkbox" checked={confirmedOnly} onChange={(e) => setConfirmedOnly(e.target.checked)} /> Confirmed licences only</label>{" "}
        <label><input type="checkbox" checked={commercialOnly} onChange={(e) => setCommercialOnly(e.target.checked)} /> Can be sold</label>
      </div>
      {err && <pre role="alert" className="panel-error">{err}</pre>}
      {msg && <p role="status">{msg}</p>}
      {list && list.total === 0 && <p>The library is empty. Use "Import from this project" to add the assets a project already has, or Add Font.</p>}
      {list && list.total > 0 && <p role="status">{list.matched} of {list.total} shown{list.matched > assets.length ? ` (first ${assets.length})` : ""}.</p>}
      <ul className="libraries-list" aria-label="Assets">
        {shown.map((a) => (
          <li key={a.id}>
            {a.thumb ? <img className="asset-thumb" src={a.thumb} alt="" /> : <span className="asset-thumb" aria-hidden="true">[{a.kind}]</span>}
            <div>
              <strong>{a.name}</strong> <span>{a.kind}, {size(a.size)}</span>
              <div>{licenceLine(a)}</div>
              {a.author && <div>By {a.author}</div>}
              {a.tags.length > 0 && <div>Tags: {a.tags.join(", ")}</div>}
              {a.uses.length > 0 && <div>Used in: {a.uses.slice(0, 3).join("; ")}{a.uses.length > 3 ? ` and ${a.uses.length - 3} more` : ""}</div>}
              {acting?.id === a.id && (
                <div className="asset-form">
                  {acting.mode === "use" ? (
                    <label>
                      Put it at
                      <input value={dest} onChange={(e) => setDest(e.target.value)} />
                    </label>
                  ) : (
                    <>
                      <label>
                        Licence
                        <select value={licence} onChange={(e) => setLicence(e.target.value)}>
                          <option value="">Choose…</option>
                          {options(a.spdx).map((l) => (
                            <option key={l.id} value={l.id}>{l.label}</option>
                          ))}
                        </select>
                      </label>
                      {needsNote(licence) && (
                        <label>
                          Which licence, or why it is used anyway
                          <input value={note} onChange={(e) => setNote(e.target.value)} />
                        </label>
                      )}
                      <label>
                        Author
                        <input value={author} onChange={(e) => setAuthor(e.target.value)} />
                      </label>
                      <label>
                        Where it came from (web address)
                        <input value={url} onChange={(e) => setUrl(e.target.value)} />
                      </label>
                      <label>
                        Credit line, if the licence asks for one
                        <input value={credit} onChange={(e) => setCredit(e.target.value)} />
                      </label>
                      <label>
                        I confirm this licence (your name)
                        <input value={by} onChange={(e) => setBy(e.target.value)} />
                      </label>
                    </>
                  )}
                  <div className="row-buttons">
                    <button onClick={() => void submit()} disabled={!ready}>{acting.mode === "use" ? "Add to project" : "Confirm licence"}</button>
                    <button onClick={() => setActing(null)}>Cancel</button>
                  </div>
                </div>
              )}
            </div>
            {acting?.id !== a.id && (
              <div className="row-buttons">
                {a.status === "confirmed" ? (
                  <button onClick={() => begin(a, "use")} disabled={!props.hasProject} title={props.hasProject ? "Copy into the open project" : "Open a project first"}>Use</button>
                ) : (
                  <button onClick={() => begin(a, "confirm")}>Confirm…</button>
                )}
              </div>
            )}
          </li>
        ))}
      </ul>
      <Pager page={page} pages={pages} set={setPage} />
    </>
  );
}

interface Row {
  include: boolean;
  licence: string;
  note: string;
  author: string;
}

/** Import the open project's assets into the library: each licence is
 *  checked and confirmed by name before anything is copied; originals stay. */
function Import(props: { onImported: () => void }) {
  const [plan, setPlan] = useState<ImportPlanView | null>(null);
  const [rows, setRows] = useState<Record<string, Row>>({});
  const [includeCode, setIncludeCode] = useState(false);
  const [by, setBy] = useState(remembered);
  const [page, setPage] = useState(0);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [report, setReport] = useState<ImportReportView | null>(null);

  const scan = useCallback(async () => {
    setBusy(true);
    setErr(null);
    setReport(null);
    try {
      const p = await assetsImportPlan(includeCode);
      const r: Record<string, Row> = {};
      for (const a of p.assets) r[a.hash] = { include: !a.existingLicence, licence: a.existingLicence ?? a.guess ?? "", note: "", author: "" };
      setPlan(p);
      setRows(r);
      setPage(0);
    } catch (e) {
      setPlan(null);
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }, [includeCode]);

  const set = (hash: string, patch: Partial<Row>) => setRows((r) => ({ ...r, [hash]: { ...r[hash], ...patch } }));
  const chosen = plan?.assets.filter((a) => rows[a.hash]?.include) ?? [];
  const problems = chosen.filter((a) => !rows[a.hash].licence || (needsNote(rows[a.hash].licence) && !rows[a.hash].note.trim())).length;
  const ready = !!plan && chosen.length > 0 && problems === 0 && by.trim() !== "" && !busy;

  const apply = async () => {
    if (!plan || !ready) return;
    setBusy(true);
    setErr(null);
    remember(by.trim());
    try {
      const r = await assetsImportApply(
        includeCode,
        chosen.map((a) => ({ hash: a.hash, licence: rows[a.hash].licence, confirmedBy: by.trim(), note: rows[a.hash].note.trim(), author: rows[a.hash].author.trim(), sourceUrl: "", attribution: "" })),
      );
      setReport(r);
      setPlan(null);
      props.onImported();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const pages = plan ? Math.max(1, Math.ceil(plan.assets.length / PAGE)) : 1;
  const shown = plan?.assets.slice(page * PAGE, page * PAGE + PAGE) ?? [];

  return (
    <>
      <div className="panel-bar">
        <button onClick={() => void scan()} disabled={busy}>{plan ? "Scan again" : "Scan this project"}</button>
        <label><input type="checkbox" checked={includeCode} onChange={(e) => setIncludeCode(e.target.checked)} /> Include Lua libraries</label>
      </div>
      {err && <pre role="alert" className="panel-error">{err}</pre>}
      {report && (
        <div role="status">
          <p>Imported {report.imported} new, linked {report.linked} already in the library, left {report.skipped} alone{report.bytesSaved > 0 ? `; ${size(report.bytesSaved)} not stored twice` : ""}.</p>
          {report.conflicts.map((c) => (
            <p key={c.name} className="panel-error">{c.name}: the library holds it under {c.existing}, not {c.proposed}. Nothing changed.</p>
          ))}
          {report.failed.map((f) => (
            <p key={f} className="panel-error">{f}</p>
          ))}
        </div>
      )}
      {!plan && !report && !err && <p>Scan lists every image, sound, font and other asset in the open project, once per distinct file. Nothing is copied until you confirm each licence and press Import. Your files are never changed or deleted.</p>}
      {plan && (
        <>
          <p role="status">
            {plan.assets.length} distinct assets{plan.duplicateBytes > 0 ? `; identical copies waste ${size(plan.duplicateBytes)}` : ""}; {chosen.length} chosen.
            {plan.unreadable.length > 0 ? ` ${plan.unreadable.length} could not be read.` : ""}
          </p>
          <div className="panel-bar">
            <button onClick={() => setRows((r) => Object.fromEntries(Object.entries(r).map(([h, x]) => [h, { ...x, licence: x.licence || plan.assets.find((a) => a.hash === h)?.guess || "" }])))}>Fill guesses</button>
            <button onClick={() => setRows((r) => Object.fromEntries(Object.entries(r).map(([h, x]) => [h, { ...x, include: !!x.licence }])))}>Choose those with a licence</button>
            <button onClick={() => setRows((r) => Object.fromEntries(Object.entries(r).map(([h, x]) => [h, { ...x, include: false }])))}>Choose none</button>
          </div>
          <ul className="libraries-list" aria-label="Assets to import">
            {shown.map((a) => {
              const r = rows[a.hash];
              return (
                <li key={a.hash}>
                  <div>
                    <label>
                      <input type="checkbox" checked={r.include} onChange={(e) => set(a.hash, { include: e.target.checked })} /> <strong>{a.name}</strong>{" "}
                      <span>{a.kind}, {size(a.size)}</span>
                    </label>
                    <div>{a.paths.slice(0, 2).join(", ")}{a.paths.length > 2 ? ` and ${a.paths.length - 2} more` : ""}{a.paths.length > 1 ? " (identical files, stored once)" : ""}</div>
                    {a.existingLicence && <div>Already in the library under {a.existingLicence}.</div>}
                    {a.guess && !a.existingLicence && <div>Looks like {a.guess}, from {a.guessFrom}. Check it.</div>}
                    {r.include && (
                      <div className="asset-form">
                        <label>
                          Licence
                          <select value={r.licence} onChange={(e) => set(a.hash, { licence: e.target.value })}>
                            <option value="">Choose…</option>
                            {options(a.guess).map((l) => (
                              <option key={l.id} value={l.id}>{l.label}</option>
                            ))}
                          </select>
                        </label>
                        {needsNote(r.licence) && (
                          <label>
                            Which licence, or why it is used anyway
                            <input value={r.note} onChange={(e) => set(a.hash, { note: e.target.value })} />
                          </label>
                        )}
                        <label>
                          Author
                          <input value={r.author} onChange={(e) => set(a.hash, { author: e.target.value })} />
                        </label>
                      </div>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
          <Pager page={page} pages={pages} set={setPage} />
          <label>
            I confirm the licences I chose (your name)
            <input value={by} onChange={(e) => setBy(e.target.value)} />
          </label>
          {problems > 0 && <p role="alert" className="panel-error">{problems} chosen without a licence or a reason.</p>}
          <div className="row-buttons">
            <button onClick={() => void apply()} disabled={!ready}>Import {chosen.length}</button>
          </div>
        </>
      )}
    </>
  );
}

export default function AssetLibrary(props: { hasProject: boolean; onClose: () => void; onAdded: (path: string) => void }) {
  const [tab, setTab] = useState<"browse" | "import">("browse");
  const [rev, setRev] = useState(0);
  return (
    <Panel title="Asset library" kind="panel-assets" onClose={props.onClose}>
      <div className="panel-bar" role="tablist" aria-label="Asset library">
        <button role="tab" aria-selected={tab === "browse"} onClick={() => setTab("browse")}>{tab === "browse" ? "[Library]" : " Library "}</button>
        <button role="tab" aria-selected={tab === "import"} onClick={() => setTab("import")} disabled={!props.hasProject} title={props.hasProject ? "" : "Open a project first"}>{tab === "import" ? "[Import from this project]" : " Import from this project "}</button>
      </div>
      {tab === "browse" ? <Browse key={rev} hasProject={props.hasProject} onAdded={props.onAdded} /> : <Import onImported={() => setRev((r) => r + 1)} />}
    </Panel>
  );
}
