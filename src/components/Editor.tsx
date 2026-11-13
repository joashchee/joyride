import { useEffect, useRef } from "react";
import * as monaco from "monaco-editor/editor/editor.api.js";
import "../monaco-setup";
import { formatLua, inTauri } from "../lib/backend";
import { trackModel, untrackModel, uriFor } from "../lib/lsp";
import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";

// Monaco's own web worker, bundled: nothing is fetched from a CDN (CLAUDE.md
// rule 1). Lua needs no language worker; `lua-language-server` is reached
// through src/lib/lsp.ts.
(self as unknown as { MonacoEnvironment: monaco.Environment }).MonacoEnvironment = {
  getWorker: () => new EditorWorker(),
};

interface Doc {
  model: monaco.editor.ITextModel;
  saved: number;
}
const docs = new Map<string, Doc>();

/** Forgets a closed file's text and undo history. */
export function closeDoc(path: string) {
  const m = docs.get(path)?.model;
  if (m) untrackModel(m);
  m?.dispose();
  docs.delete(path);
}

function languageOf(path: string) {
  const ext = path.slice(path.lastIndexOf(".") + 1).toLowerCase();
  return { lua: "lua", json: "json", md: "markdown", toml: "ini", txt: "plaintext" }[ext] ?? "plaintext";
}

export default function Editor(props: {
  /** The file shown; `text` is only used the first time it is opened. */
  path: string;
  text: string;
  onDirty: (path: string, dirty: boolean) => void;
  onSave: (path: string, text: string) => Promise<void>;
  /** Moves the cursor to a line; `n` changes on each request, so the same line can be asked for twice. */
  goto?: { line: number; n: number };
  /** Something to tell the person (a format error). */
  onNotice?: (text: string | null) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const ed = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const cb = useRef(props);
  cb.current = props;

  useEffect(() => {
    const dark = window.matchMedia("(prefers-color-scheme: dark)");
    const editor = monaco.editor.create(host.current!, {
      model: null,
      theme: dark.matches ? "vs-dark" : "vs",
      automaticLayout: true,
      minimap: { enabled: false },
      fontSize: 14,
      tabSize: 2,
      ariaLabel: "Code editor",
    });
    ed.current = editor;
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, async () => {
      const m = editor.getModel();
      const path = m && [...docs].find(([, d]) => d.model === m)?.[0];
      if (!m || !path) return;
      const version = m.getAlternativeVersionId();
      await cb.current.onSave(path, m.getValue());
      docs.get(path)!.saved = version;
      cb.current.onDirty(path, m.getAlternativeVersionId() !== version);
    });
    const sub = editor.onDidChangeModelContent(() => {
      const m = editor.getModel();
      const path = m && [...docs].find(([, d]) => d.model === m)?.[0];
      if (path) cb.current.onDirty(path, m.getAlternativeVersionId() !== docs.get(path)!.saved);
    });
    // Format Document (⇧⌥F) with the bundled StyLua; a syntax error is reported, the text left alone.
    const formatter = inTauri
      ? monaco.languages.registerDocumentFormattingEditProvider("lua", {
          displayName: "StyLua",
          async provideDocumentFormattingEdits(model) {
            try {
              const text = await formatLua(model.getValue());
              cb.current.onNotice?.(null);
              return text === model.getValue() ? [] : [{ range: model.getFullModelRange(), text }];
            } catch (e) {
              cb.current.onNotice?.(`Format: ${String(e)}`);
              return [];
            }
          },
        })
      : null;
    const theme = () => monaco.editor.setTheme(dark.matches ? "vs-dark" : "vs");
    dark.addEventListener("change", theme);
    return () => {
      dark.removeEventListener("change", theme);
      formatter?.dispose();
      sub.dispose();
      editor.dispose();
      ed.current = null;
    };
  }, []);

  useEffect(() => {
    let d = docs.get(props.path);
    if (!d) {
      const lang = languageOf(props.path);
      const uri = uriFor(props.path) ?? undefined;
      const model = monaco.editor.createModel(props.text, lang, uri);
      if (lang === "lua" && uri) trackModel(model);
      d = { model, saved: model.getAlternativeVersionId() };
      docs.set(props.path, d);
    }
    ed.current?.setModel(d.model);
    ed.current?.focus();
  }, [props.path, props.text]);

  useEffect(() => {
    const g = props.goto;
    if (!g || !ed.current) return;
    ed.current.revealLineInCenter(g.line);
    ed.current.setPosition({ lineNumber: g.line, column: 1 });
    ed.current.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.goto?.n]);

  return <div ref={host} className="editor-host" />;
}
