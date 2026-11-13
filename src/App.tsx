import { lazy, Suspense, useEffect, useRef, useState } from "react";
import Splash from "./components/Splash";
import FileTree from "./components/FileTree";
import Console from "./components/Console";
import PreviewPane from "./components/Preview";
import AddFont from "./components/AddFont";
import NameBox from "./components/NameBox";
import Search from "./components/Search";
import ExportPanel from "./components/ExportPanel";
import Libraries from "./components/Libraries";
import Docs from "./components/Docs";
import AutoPort from "./components/AutoPort";
import GitHub from "./components/GitHub";
import Settings from "./components/Settings";
import { projectListing, chooseFolder, coreInfo, lastProject, inTauri, createDir, createFile, deletePath, renamePath, onRun, openProject, readFile, previewFile, runProject, SCREENS, stopProject, writeFile, type CoreInfo, type Preview, type Listing, type RunLine } from "./lib/backend";

// Monaco loads after the splash has gone, so the first screen never waits on it.
const Editor = lazy(() => import("./components/Editor"));
const lsp = () => import("./lib/lsp");
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
  const [ghOpen, setGhOpen] = useState(false);
  const [portOpen, setPortOpen] = useState(false);
  const [docsOpen, setDocsOpen] = useState(false);
  const [docsKey, setDocsKey] = useState(0);
  const [libsOpen, setLibsOpen] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [fontOpen, setFontOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [findKey, setFindKey] = useState(0);
  const [selected, setSelected] = useState<string | null>(null);
  const [asking, setAsking] = useState<null | { kind: "file" | "dir" | "rename"; label: string; initial: string }>(null);
  const [lspState, setLspState] = useState<string>("off");
  const [preview, setPreview] = useState<Preview | null>(null);
  const picked = useRef(0);
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

  // Back where it was: the last project reopens once the splash is gone.
  const restored = useRef(false);
  useEffect(() => {
    if (!gone || restored.current) return;
    restored.current = true;
    void lastProject()
      .then(async (d) => {
        if (d) await openDir(d);
      })
      .catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gone]);

  // Closing the window with unsaved files asks first.
  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;
  useEffect(() => {
    if (!inTauri) return;
    let off = () => {};
    let dead = false;
    void import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
      const u = await getCurrentWindow().onCloseRequested((e) => {
        const n = dirtyRef.current.size;
        if (n > 0 && !window.confirm(`${n} file${n === 1 ? " has" : "s have"} unsaved changes. Quit without saving?`)) e.preventDefault();
      });
      dead ? u() : (off = u);
    });
    return () => {
      dead = true;
      off();
    };
  }, []);

  useEffect(() => {
    let off = () => {};
    let dead = false;
    // Only once the splash is gone: the editor chunk loads after it.
    if (gone) void lsp().then((L) => (dead ? undefined : (off = L.onLspStatus(setLspState))));
    return () => {
      dead = true;
      off();
    };
  }, [gone]);

  useEffect(() => {
    let cancelled = false;
    coreInfo()
      .then((i) => {
        if (cancelled) return;
        setInfo(i);
        setStep(1);
        // A text-mode screen just redraws: no fade, the splash is gone at once.
        setLeaving(true);
        setGone(true);
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
      if (dir) await openDir(dir);
    } catch (e) {
      fail(e);
    }
  };

  const openDir = async (dir: string) => {
    try {
      const l = await openProject(dir);
      tabs.forEach(closeDoc);
      const L = await lsp();
      L.stopLsp();
      await L.setProjectRoot();
      void L.startLsp().catch((e) => setNotice(String(e)));
      setProject(l);
      setSelected(null);
      setAsking(null);
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

  /** A click in the tree: text opens in the editor, anything else previews. */
  const pickFile = async (path: string) => {
    const n = ++picked.current;
    try {
      const p = inTauri ? await previewFile(path) : null;
      if (n !== picked.current) return; // a later click won
      if (!p || p.kind === "text") {
        setPreview(null);
        await openFile(path);
      } else {
        setPreview(p);
        setNotice(null);
      }
    } catch (e) {
      if (n === picked.current) fail(e);
    }
  };

  const openFile = async (path: string) => {
    try {
      setPreview(null);
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

  // ---- New, rename, delete (the tree's buttons) ----
  const sel = project?.entries.find((e) => e.path === selected) ?? null;
  const within = (base: string) => (p: string) => p === base || p.startsWith(base + "/");
  /** The folder new things go in: the selected folder, or the selected file's folder. */
  const folderOfSelection = !sel ? "" : sel.dir ? sel.path + "/" : sel.path.includes("/") ? sel.path.slice(0, sel.path.lastIndexOf("/") + 1) : "";

  const dropTabs = (match: (p: string) => boolean) => {
    tabs.filter(match).forEach(closeDoc);
    const rest = tabs.filter((t) => !match(t));
    setTabs(rest);
    setTexts((t) => Object.fromEntries(Object.entries(t).filter(([p]) => !match(p))));
    setDirty((d) => new Set([...d].filter((p) => !match(p))));
    if (active !== null && match(active)) setActive(rest[rest.length - 1] ?? null);
  };
  const unsaved = (match: (p: string) => boolean) => [...dirty].find(match);

  const submitName = async (name: string) => {
    const a = asking;
    setAsking(null);
    if (!a) return;
    try {
      if (a.kind === "file") {
        setProject(await createFile(name));
        setSelected(name);
        await openFile(name);
      } else if (a.kind === "dir") {
        setProject(await createDir(name));
        setSelected(name);
      } else if (sel && name !== sel.path) {
        const match = within(sel.path);
        const u = unsaved(match);
        if (u) return setNotice(`Save or close ${u} first.`);
        const l = await renamePath(sel.path, name);
        const was = active !== null && match(active) ? active : null;
        dropTabs(match);
        setProject(l);
        setSelected(name);
        if (was) await openFile(name + was.slice(sel.path.length));
      }
      setNotice(null);
    } catch (e) {
      fail(e);
    }
  };
  const removeSelected = async () => {
    if (!sel) return;
    const match = within(sel.path);
    const u = unsaved(match);
    if (u) return setNotice(`Save or close ${u} first.`);
    const what = sel.dir ? `the folder ${sel.path} and everything in it` : sel.path;
    if (!window.confirm(`Delete ${what}? This cannot be undone.`)) return;
    try {
      const l = await deletePath(sel.path);
      dropTabs(match);
      setProject(l);
      setSelected(null);
      setNotice(null);
    } catch (e) {
      fail(e);
    }
  };

  const run = async () => {
    if (!project) return setNotice("Open a project first.");
    try {
      setLines([]);
      setRunning(true);
      await runProject(screen);
    } catch (e) {
      setRunning(false);
      fail(e);
    }
  };
  const [screen, setScreen] = useState("");
  const stop = () => void stopProject();
  const openAt = async (file: string, line: number) => {
    await openFile(file);
    setGoto((g) => ({ line, n: g.n + 1 }));
  };

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      if (e.shiftKey && e.key.toLowerCase() === "d") {
        e.preventDefault();
        setDocsOpen(true);
        setDocsKey((k) => k + 1);
      } else if (e.shiftKey && e.key.toLowerCase() === "f" && project) {
        e.preventDefault();
        setFindKey((k) => k + 1);
      } else if (e.key === "r") {
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
        <div className="menubar" role="toolbar" aria-label="Main">
          <button onClick={openFolder}>Open Folder…</button>
          <button onClick={run} disabled={!project || running} title="Run (Cmd+R)">Run</button>
          <button onClick={stop} disabled={!running} title="Stop (Cmd+.)">Stop</button>
          <button
            onClick={() => setScreen((c) => SCREENS[(SCREENS.findIndex((x) => x.id === c) + 1) % SCREENS.length].id)}
            disabled={running}
            title="Screen to run on: a target's overlay in a simulated screen (click to change)"
          >
            Screen: {SCREENS.find((x) => x.id === screen)?.name}
          </button>
          <button onClick={() => setDocsOpen((x) => !x)} aria-expanded={docsOpen} title="LÖVE reference (Cmd+Shift+D)">Reference…</button>
          <button onClick={() => setLibsOpen((x) => !x)} disabled={!project} aria-expanded={libsOpen}>Libraries…</button>
          <button onClick={() => setFontOpen((x) => !x)} disabled={!project} aria-expanded={fontOpen}>Add Font…</button>
          <button onClick={() => setGhOpen((x) => !x)} disabled={!project} aria-expanded={ghOpen}>GitHub…</button>
          <button onClick={() => setPortOpen((x) => !x)} disabled={!project} aria-expanded={portOpen}>Auto-Port…</button>
          <button onClick={() => setExporting((x) => !x)} disabled={!project} aria-expanded={exporting}>Export…</button>
          <button onClick={() => setSettingsOpen((x) => !x)} aria-expanded={settingsOpen}>Settings…</button>
          <span className="project-name">{project?.name ?? "No project open"}</span>
        </div>
        {settingsOpen && <Settings onClose={() => setSettingsOpen(false)} />}
        {docsOpen && <Docs onClose={() => setDocsOpen(false)} focusKey={docsKey} />}
        {libsOpen && project && <Libraries onClose={() => setLibsOpen(false)} onChanged={() => void projectListing().then(setProject).catch(fail)} />}
        {ghOpen && project && <GitHub onClose={() => setGhOpen(false)} onChanged={() => void projectListing().then(setProject).catch(fail)} onOpen={(f, l) => void openAt(f, l)} />}
        {portOpen && project && <AutoPort onClose={() => setPortOpen(false)} onOpen={(f, l) => void openAt(f, l)} onChanged={() => void projectListing().then(setProject).catch(fail)} />}
        {exporting && project && <ExportPanel onClose={() => setExporting(false)} />}
        {fontOpen && project && (
          <AddFont
            onClose={() => setFontOpen(false)}
            onAdded={(path) => {
              setFontOpen(false);
              void projectListing().then(setProject).catch(fail);
              setSelected(path);
              void pickFile(path);
            }}
          />
        )}
        <div className="workspace">
          <div className="sidebar-wrap">
          <nav className="sidebar" aria-label="Explorer">
            {project ? (
              <>
                <div className="tree-actions" role="toolbar" aria-label="File actions">
                  <button onClick={() => setAsking({ kind: "file", label: "New file", initial: folderOfSelection })}>New File</button>
                  <button onClick={() => setAsking({ kind: "dir", label: "New folder", initial: folderOfSelection })}>New Folder</button>
                  <button disabled={!sel} onClick={() => sel && setAsking({ kind: "rename", label: `Rename ${sel.path} to`, initial: sel.path })}>Rename</button>
                  <button disabled={!sel} onClick={() => void removeSelected()}>Delete</button>
                </div>
                <Search onOpen={openAt} focusKey={findKey} />
                {asking && <NameBox key={asking.kind + asking.initial} label={asking.label} initial={asking.initial} onSubmit={(n) => void submitName(n)} onCancel={() => setAsking(null)} />}
                <FileTree entries={project.entries} active={active} onOpen={pickFile} selected={selected} onSelect={setSelected} />
              </>
            ) : (
              <p className="empty">Open a folder with a LÖVE project.</p>
            )}
          </nav>
          </div>
          <section className="main" aria-label="Editor">
            <div className="tabs" role="tablist" aria-label="Open files">
              {tabs.map((t) => (
                <span key={t} className={"tab" + (t === active ? " active" : "")}>
                  <button role="tab" aria-selected={t === active && !preview} onClick={() => { setPreview(null); setActive(t); }}>
                    {t.split("/").pop()}
                    {dirty.has(t) ? " •" : ""}
                  </button>
                  <button aria-label={`Close ${t}`} className="tab-close" onClick={() => closeTab(t)}>
                    ×
                  </button>
                </span>
              ))}
            </div>
            <Suspense fallback={<p className="editor-loading" role="status">Loading the editor…</p>}>
              {gone && !preview && active !== null && active in texts && (
                <Editor path={active} text={texts[active]} onDirty={onDirty} onSave={onSave} goto={goto} onNotice={setNotice} />
              )}
            </Suspense>
            {preview && <PreviewPane key={preview.path} data={preview} onClose={() => setPreview(null)} />}
            <Console lines={lines} running={running} onClear={() => setLines([])} onOpen={openAt} />
            {active === null && !preview && <p className="empty">Pick a file to edit. Save with Cmd+S or Ctrl+S.</p>}
          </section>
        </div>
        <footer className="statusbar" role="status">
          {notice ?? `Core ${info?.version ?? "…"}`}
          {project && ` · Language server: ${{ off: "off", starting: "starting…", ready: "ready", stopped: "stopped" }[lspState] ?? lspState}`}
        </footer>
      </main>
    </>
  );
}
