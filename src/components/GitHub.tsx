import { useEffect, useState } from "react";
import { gitCommit, gitInit, gitPull, gitPush, githubCreateRepo, githubOverview, githubReleaseWorkflow, openGithubUrl, type GithubOverview } from "../lib/backend";

/** Git and GitHub, through the gh and git command lines. Sign in once with `gh auth login` in a terminal: JoyrIDE never asks for or stores a token. */
export default function GitHub(props: { onClose: () => void; onChanged: () => void; onOpen: (path: string, line: number) => void }) {
  const [o, setO] = useState<GithubOverview | null>(null);
  const [msg, setMsg] = useState("");
  const [name, setName] = useState("");
  const [isPrivate, setIsPrivate] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const refresh = () =>
    githubOverview()
      .then((v) => (setO(v), setName((n) => n || v.suggestedName), setErr(null)))
      .catch((e) => setErr(String(e)));
  useEffect(() => void refresh(), []);

  const act = async (what: string, f: () => Promise<GithubOverview>, done?: string) => {
    setBusy(what);
    setErr(null);
    setNote(null);
    try {
      setO(await f());
      if (done) setNote(done);
      props.onChanged();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  const g = o?.git;
  const online = !!o?.auth.loggedIn;
  return (
    <section className="github" aria-label="GitHub" onKeyDown={(e) => e.key === "Escape" && props.onClose()}>
      <header>
        <strong>GitHub</strong>
        {o?.auth.user && online && <span className="lib-meta"> signed in as {o.auth.user}</span>}
        <button onClick={() => void refresh()} disabled={busy !== null}>Check again</button>
        <button onClick={props.onClose} aria-label="Close GitHub">×</button>
      </header>
      {err && <pre role="alert" className="export-error">{err}</pre>}
      {note && <p role="status">{note}</p>}
      {o && !online && <pre className="hint gh-signin">{o.auth.message}</pre>}
      {g && !g.isRepo && (
        <div>
          <p>This folder is not a git repository yet.</p>
          <button disabled={busy !== null} onClick={() => void act("init", gitInit, "Initialised a repository with a .gitignore for builds.")}>Start a repository</button>
        </div>
      )}
      {g?.isRepo && (
        <>
          <p className="hint">
            Branch <strong>{g.branch}</strong>
            {g.upstream ? ` · ${g.upstream}` : " · no upstream yet"}
            {g.ahead > 0 && ` · ${g.ahead} to push`}
            {g.behind > 0 && ` · ${g.behind} to pull`}
            {o?.repo && ` · ${o.repo.nameWithOwner} (${o.repo.visibility})`}{" "}
            {o?.repo && (
              <button onClick={() => openGithubUrl(o.repo!.url).catch((e) => setErr(String(e)))}>Open on GitHub</button>
            )}
          </p>
          <ul className="git-changes" aria-label="Changes">
            {g.changes.map((c) => (
              <li key={c.path}>
                <code>{c.status}</code> {c.path}
              </li>
            ))}
          </ul>
          {g.changes.length === 0 && <p className="hint">No changes.</p>}
          <form
            className="export-buttons"
            onSubmit={(e) => {
              e.preventDefault();
              void act("commit", () => gitCommit(msg), "Committed.").then(() => setMsg(""));
            }}
          >
            <input aria-label="Commit message" placeholder="Commit message" value={msg} onChange={(e) => setMsg(e.target.value)} style={{ flex: 1 }} />
            <button type="submit" disabled={busy !== null || g.changes.length === 0 || !msg.trim()}>Commit all</button>
          </form>
          <div className="export-buttons">
            <button disabled={busy !== null || !online || !g.upstream || g.behind === 0} onClick={() => void act("pull", gitPull, "Pulled.")}>Pull</button>
            <button disabled={busy !== null || !online || (!!g.upstream && g.ahead === 0) || (!g.upstream && !o?.repo)} onClick={() => void act("push", gitPush, "Pushed.")}>
              Push
            </button>
            <button
              disabled={busy !== null}
              onClick={async () => {
                try {
                  const p = await githubReleaseWorkflow();
                  setNote(`Wrote ${p}. Push a tag like v1.0.0 to build a release.`);
                  props.onChanged();
                  props.onOpen(p, 1);
                } catch (e) {
                  setErr(String(e));
                }
              }}
              title="Adds .github/workflows/release.yml: a pushed v* tag builds a .love and attaches it to a GitHub release"
            >
              Add release workflow
            </button>
          </div>
          {online && !o?.repo && (
            <form
              className="export-buttons"
              onSubmit={(e) => {
                e.preventDefault();
                void act("create", () => githubCreateRepo(name, isPrivate), "Created the repository and linked it as origin. Commit, then Push.");
              }}
            >
              <input aria-label="New repository name" value={name} onChange={(e) => setName(e.target.value)} style={{ flex: 1 }} spellCheck={false} />
              <label>
                <input type="checkbox" checked={isPrivate} onChange={(e) => setIsPrivate(e.target.checked)} /> Private
              </label>
              <button type="submit" disabled={busy !== null || !name.trim()}>Create on GitHub</button>
            </form>
          )}
        </>
      )}
    </section>
  );
}
