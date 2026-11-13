import { useEffect, useState } from "react";
import { gamesOpenUrl, gamesSearch, type GamesResult } from "../lib/backend";
import Panel from "./Panel";

const PLATFORMS = ["love", "windows", "mac", "linux", "android", "ios", "web", "portmaster", "source"];

const size = (n: number) => (n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} kB`);

/** The LÖVE games and apps catalogue: search, filter, read the licence and the latest version, open a page or download in the browser. Nothing is downloaded by the app. */
export default function Games(props: { onClose: () => void }) {
  const [q, setQ] = useState("");
  const [kind, setKind] = useState("");
  const [platform, setPlatform] = useState("");
  const [openSource, setOpenSource] = useState(false);
  const [archived, setArchived] = useState(false);
  const [res, setRes] = useState<GamesResult | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    gamesSearch(q, kind, platform, openSource, archived)
      .then((r) => live && (setRes(r), setErr(null)))
      .catch((e) => live && setErr(String(e)));
    return () => {
      live = false;
    };
  }, [q, kind, platform, openSource, archived]);

  const go = (url: string) => gamesOpenUrl(url).catch((e) => setErr(String(e)));

  return (
    <Panel title="Games and apps" kind="panel-games" onClose={props.onClose}>
      <div className="panel-bar">
        <input aria-label="Search games and apps" placeholder="Search" value={q} onChange={(e) => setQ(e.target.value)} spellCheck={false} autoFocus />
      </div>
      <fieldset className="panel-bar">
        <legend>Kind</legend>
        {[["", "All"], ["game", "Games"], ["app", "Apps"]].map(([v, l]) => (
          <label key={v}>
            <input type="radio" name="games-kind" checked={kind === v} onChange={() => setKind(v)} /> {l}{" "}
          </label>
        ))}
      </fieldset>
      <fieldset className="panel-bar">
        <legend>Download for</legend>
        {["", ...PLATFORMS].map((p) => (
          <label key={p}>
            <input type="radio" name="games-platform" checked={platform === p} onChange={() => setPlatform(p)} /> {p || "any"}{" "}
          </label>
        ))}
      </fieldset>
      <div className="panel-bar">
        <label><input type="checkbox" checked={openSource} onChange={(e) => setOpenSource(e.target.checked)} /> Open source only</label>{" "}
        <label><input type="checkbox" checked={archived} onChange={(e) => setArchived(e.target.checked)} /> Include abandoned</label>
      </div>
      {err && <pre role="alert" className="panel-error">{err}</pre>}
      <ul className="libraries-list" aria-label="Games and apps">
        {res?.games.map((x) => (
          <li key={x.id}>
            <button aria-expanded={x.id === open} onClick={() => setOpen(x.id === open ? null : x.id)} aria-label={`${x.id === open ? "Hide" : "Show"} ${x.name}`}>
              {x.id === open ? "Hide" : "Show"}
            </button>
            <div className="lib-main">
              <strong>{x.name}</strong> <span className="lib-meta">{x.kind} · {x.latest ?? "no release"} · {x.licence}{x.licenceConfirmed ? "" : x.licence === "unknown" ? "" : " (unconfirmed)"}</span>
              <div>{x.summary}</div>
              {x.id === open && (
                <div className="games-detail">
                  {x.authors.length > 0 && <div className="lib-meta">By {x.authors.join(", ")}</div>}
                  {x.loveVersion && <div className="lib-meta">Made for LÖVE {x.loveVersion}</div>}
                  <div className={"lib-meta" + (x.openSource ? "" : " lib-broken")}>
                    {x.openSource ? `Open source: ${x.licence}` : x.licence === "unknown" ? "Licence unknown: check before using or sharing" : `Not open source: ${x.licence}`}
                    {x.licenceFrom ? ` (from ${x.licenceFrom})` : ""}
                  </div>
                  {x.archived && <div className="lib-meta lib-broken">Abandoned</div>}
                  {x.downloads.length > 0 && <strong>Downloads</strong>}
                  <ul>
                    {x.downloads.map((d) => (
                      <li key={d.url}>
                        <button onClick={() => void go(d.url)} aria-label={`Open the ${d.platform} download of ${x.name} in the browser`}>Open</button>{" "}
                        {d.platform} <span className="lib-meta">{[d.version, d.date, d.size ? size(d.size) : null].filter(Boolean).join(" · ")}</span>
                      </li>
                    ))}
                  </ul>
                  <strong>Pages</strong>
                  <ul>
                    {x.sources.map((s) => (
                      <li key={s.url}>
                        <button onClick={() => void go(s.url)} aria-label={`Open the ${s.kind} page of ${x.name} in the browser`}>Open</button> {s.kind} <span className="lib-meta">{s.url.replace("https://", "")}</span>
                      </li>
                    ))}
                  </ul>
                  {x.checked && <div className="lib-meta">Checked {x.checked}</div>}
                </div>
              )}
            </div>
          </li>
        ))}
      </ul>
      {res && res.total === 0 && <p className="hint">No games or apps are listed yet. The list fills when the catalogue is refreshed.</p>}
      {res && res.total > 0 && res.games.length === 0 && <p className="hint">Nothing matches ({res.total} listed).</p>}
      {res && res.total > 0 && <p className="hint">{res.games.length} of {res.total} shown. Links open in your browser; nothing is downloaded here.</p>}
    </Panel>
  );
}
