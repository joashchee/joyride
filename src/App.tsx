import { lazy, Suspense, useEffect, useState } from "react";
import Splash from "./components/Splash";
import FileTree from "./components/FileTree";
import Console from "./components/Console";
import { chooseFolder, coreInfo, onRun, openProject, readFile, runProject, stopProject, writeFile, type CoreInfo, type Listing, type RunLine } from "./lib/backend";

// Monaco loads after the splash has gone, so the first screen never waits on it.
const Editor = lazy(() => import("./components/Editor"));
const closeDoc = (p: string) => import("./components/Editor").then((m) => m.closeDoc(p));

// What the first screen needs, in order. Heavy things (Monaco, the language
// server, toolchain checks) load after the editor is usable, not here.
const STEPS = ["Opening the engine", "Ready"] as const;

export default function App() {
  const [step, setStep] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<CoreInfo | null>(null);
  const [leaving, setLeaving] = useState(false);
  const [gone, setGone] = useState(false);
  const [project, setProject] = useState<Listing | null>(null);
  const [tabs, setTabs] = useState<string[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const [texts, setTexts] = useState<Record<string, string>>({});
  const [dirty, setDirty] = useState<Set<string>>(new Set());
  const [notice, setNotice] = useState<string | null>(null);
  const [lines, setLines] = useState<RunLine[]>([]);
  const [running, setRunning] = useState(false);
  const [goto, setGoto] = useState<{ line: number; n: number }>({ line: 1, n: 0 });

  useEffect(() => {
    let off = () => {};
    let dead = false;
    onRun(
      (l) => setLines((a) => (a.length >= 2000 ? [...a.slice(-1500), l] : [...a, l])),
      (code) => {
        setRunning(false);
        setLines((a) => [...a, { stream: code === 0 || code === null ? "out" : "err", text: code === null ? "— stopped —" : `— exited with code ${code} —`, file: null, line: null }]);
      },
    ).then((u) => (dead ? u() : (off = u)));
    return () => {
      dead = true;
      off();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    coreInfo()
      .then((i) => {
        if (cancelled) return;
        setInfo(i);
        setStep(1);
        setLeaving(true);
        setTimeout(() => !cancelled && setGone(true), 220);
      })
      .catch((e) => !cancelled && setError(`JoyrIDE could not start.\n${String(e)}`));
    return () => {
      cancelled = true;
    };
  }, []);

  const fail = (e: unknown) => setNotice(String(e));

  const openFolder = async () => {
    try {
      const dir = await chooseFolder();
      if (!dir) return;
      const l = await openProject(dir);
      tabs.forEach(closeDoc);
      setProject(l);
      setTabs([]);
      setActive(null);
      setTexts({});
      setDirty(new Set());
      setNotice(l.truncated ? "This project has more files than the tree shows." : null);
      if (l.hasMain) void openFile("main.lua");
    } catch (e) {
      fail(e);
    }
  };

  const openFile = async (path: string) => {
    try {
      if (!(path in texts)) {
        const text = await readFile(path);
        setTexts((t) => ({ ...t, [path]: text }));
      }
      setTabs((t) => (t.includes(path) ? t : [...t, path]));
      setActive(path);
      setNotice(null);
    } catch (e) {
      fail(e);
    }
  };

  const closeTab = (path: string) => {
    if (dirty.has(path) && !window.confirm(`Close ${path} without saving?`)) return;
    void closeDoc(path);
    setDirty((d) => {
      const n = new Set(d);
      n.delete(path);
      return n;
    });
    setTexts((t) => {
      const { [path]: _drop, ...rest } = t;
      return rest;
    });
    const rest = tabs.filter((t) => t !== path);
    setTabs(rest);
    if (active === path) setActive(rest[rest.length - 1] ?? null);
  };

  const run = async () => {
    if (!project) return setNotice("Open a project first.");
    try {
      setLines([]);
      setRunning(true);
      await runProject();
    } catch (e) {
      setRunning(false);
      fail(e);
    }
  };
  const stop = () => void stopProject();
  const openAt = async (file: string, line: number) => {
    await openFile(file);
    setGoto((g) => ({ line, n: g.n + 1 }));
  };

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      if (e.key === "r") {
        e.preventDefault();
        void run();
      } else if (e.key === ".") {
        e.preventDefault();
        stop();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  const onDirty = (path: string, d: boolean) =>
    setDirty((s) => {
      if (s.has(path) === d) return s;
      const n = new Set(s);
      d ? n.add(path) : n.delete(path);
      return n;
    });

  const onSave = async (path: string, text: string) => {
    try {
      await writeFile(path, text);
      setNotice(null);
    } catch (e) {
      fail(e);
      throw e;
    }
  };

  return (
    <>
      {!gone && (
        <Splash label={STEPS[step]} progress={step / (STEPS.length - 1)} error={error} leaving={leaving} />
      )}
      <main className="shell" aria-label="JoyrIDE">
        <div className="toolbar">
          <button onClick={openFolder}>Open Folder…</button>
          <button onClick={run} disabled={!project || running} title="Run (⌘R)">Run</button>
          <button onClick={stop} disabled={!running} title="Stop (⌘.)">Stop</button>
          <span className="project-name">{project?.name ?? "No project open"}</span>
        </div>
        <div className="workspace">
          <nav className="sidebar" aria-label="Explorer">
            {project ? (
              <FileTree entries={project.entries} active={active} onOpen={openFile} />
            ) : (
              <p className="empty">Open a folder with a LÖVE project.</p>
            )}
          </nav>
          <section className="main" aria-label="Editor">
            <div className="tabs" role="tablist" aria-label="Open files">
              {tabs.map((t) => (
                <span key={t} className={"tab" + (t === active ? " active" : "")}>
                  <button role="tab" aria-selected={t === active} onClick={() => setActive(t)}>
                    {t.split("/").pop()}
                    {dirty.has(t) ? " ●" : ""}
                  </button>
                  <button aria-label={`Close ${t}`} className="tab-close" onClick={() => closeTab(t)}>
                    ×
                  </button>
                </span>
              ))}
            </div>
            <Suspense fallback={<p className="editor-loading" role="status">Loading the editor…</p>}>
              {gone && active !== null && active in texts && (
                <Editor path={active} text={texts[active]} onDirty={onDirty} onSave={onSave} goto={goto} />
              )}
            </Suspense>
            <Console lines={lines} running={running} onClear={() => setLines([])} onOpen={openAt} />
            {active === null && <p className="empty">Pick a file to edit. Save with ⌘S or Ctrl+S.</p>}
          </section>
        </div>
        <footer className="statusbar" role="status">
          {notice ?? `Core ${info?.version ?? "…"}`}
        </footer>
      </main>
    </>
  );
}
