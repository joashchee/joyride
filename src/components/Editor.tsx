import { useEffect, useRef } from "react";
import * as monaco from "monaco-editor/editor/editor.api.js";
import "../monaco-setup";
import { colorLiterals, colorText, formatLua, inTauri, integrateClear, integrateMarks, lintLua } from "../lib/backend";
import { trackModel, untrackModel, uriFor } from "../lib/lsp";
import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";
import { defineThemes, TEXT_MODE_OPTIONS, themeId } from "../monaco-theme";
import { currentAppearance, onAppearance } from "../lib/theme";
import { editorFont, fontById, loadChoice, loadFont, onChoice, type CodeChoice } from "../lib/codefonts";
import type { Fit } from "../lib/snap";
import { setFitProvider } from "../lib/editorfit";

// Monaco's own web worker, bundled: nothing is fetched from a CDN (CLAUDE.md
// rule 1). Lua needs no language worker; `lua-language-server` is reached
// through src/lib/lsp.ts.
(self as unknown as { MonacoEnvironment: monaco.Environment }).MonacoEnvironment = {
  getWorker: () => new EditorWorker(),
};

interface Doc {
  model: monaco.editor.ITextModel;
  saved: number;
  /** The lines JoyrIDE inserted that are still as it wrote them: decoration id to the mark's id and the line's text. */
  marks?: Map<string, { id: string; text: string }>;
  /** A change JoyrIDE itself is making (a reload from disk): not the person touching a line. */
  reloading?: boolean;
}
const docs = new Map<string, Doc>();

/** Shows where JoyrIDE inserted code: a colour bar in the gutter on each such line, with a note on hover. */
async function applyMarks(path: string, doc: Doc) {
  if (!inTauri || !path.endsWith(".lua")) return;
  try {
    const ms = await integrateMarks(path, doc.model.getValue());
    if (doc.model.isDisposed()) return;
    const good = ms.filter((m) => m.line >= 1 && m.line <= doc.model.getLineCount());
    const ids = doc.model.deltaDecorations(
      [...(doc.marks?.keys() ?? [])],
      good.map((m) => ({
        range: new monaco.Range(m.line, 1, m.line, 1),
        options: {
          isWholeLine: true,
          linesDecorationsClassName: "joyride-inserted",
          hoverMessage: { value: m.note },
          stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges,
        },
      })),
    );
    doc.marks = new Map(ids.map((id, i) => [id, { id: good[i].id, text: doc.model.getLineContent(good[i].line) }]));
  } catch {
    /* no project, or nothing was inserted: no marks */
  }
}

/** A line the person touched is theirs: its bar goes, and JoyrIDE is told so it never marks, re-adds or removes it. */
function touchMarks(doc: Doc) {
  if (!doc.marks?.size || doc.reloading) return;
  const gone: string[] = [];
  for (const [deco, m] of doc.marks) {
    const r = doc.model.getDecorationRange(deco);
    if (!r || doc.model.getLineContent(r.startLineNumber) !== m.text) {
      gone.push(deco);
      void integrateClear(m.id).catch(() => undefined);
    }
  }
  if (gone.length) {
    doc.model.deltaDecorations(gone, []);
    for (const g of gone) doc.marks.delete(g);
  }
}

/** The file changed on disk under an open, unedited tab (JoyrIDE added or removed a library's line): show the new text, keep the history,
 *  and mark what JoyrIDE inserted. */
export async function reloadDoc(path: string, text: string) {
  const d = docs.get(path);
  if (!d || d.model.getValue() === text) {
    if (d) await applyMarks(path, d);
    return;
  }
  d.reloading = true;
  d.model.pushEditOperations([], [{ range: d.model.getFullModelRange(), text }], () => null);
  d.saved = d.model.getAlternativeVersionId();
  d.reloading = false;
  await applyMarks(path, d);
}

let lintTimer: ReturnType<typeof setTimeout> | undefined;
/** JoyrIDE's own LÖVE lints (version-calibrated) as markers, a moment after typing stops. */
function scheduleLint(path: string, model: monaco.editor.ITextModel) {
  if (!inTauri || !path.endsWith(".lua")) return;
  clearTimeout(lintTimer);
  lintTimer = setTimeout(async () => {
    try {
      const items = await lintLua(path, model.getValue());
      if (model.isDisposed()) return;
      monaco.editor.setModelMarkers(
        model,
        "joyride-lint",
        items
          .filter((i) => i.line >= 1 && i.line <= model.getLineCount())
          .map((i) => ({
            startLineNumber: i.line,
            startColumn: 1,
            endLineNumber: i.line,
            endColumn: model.getLineMaxColumn(i.line),
            severity: i.severity === "error" ? monaco.MarkerSeverity.Error : monaco.MarkerSeverity.Warning,
            message: i.message,
            source: "JoyrIDE",
            code: i.rule,
          })),
      );
    } catch {
      /* no project or no lints: leave the markers as they were */
    }
  }, 400);
}

/** The editor on screen, if any, for the Reference panel's Insert. */
let liveEditor: monaco.editor.IStandaloneCodeEditor | null = null;

/** Puts the chosen coding font on the editor: loads its file, then sets the family, size, line height and the letter-spacing nudge
 *  that make each character a whole number of device pixels, and has Monaco measure again. */
async function applyFont(editor: monaco.editor.IStandaloneCodeEditor, choice: CodeChoice) {
  const font = fontById(choice.id);
  try {
    if (font) await loadFont(font);
    else await document.fonts.load('16px "ansiapps VGA"');
  } catch {
    // the file would not load: Monaco falls back to the next family in the list
  }
  const f = editorFont(choice, window.devicePixelRatio || 1);
  editor.updateOptions({ fontFamily: f.fontFamily, fontSize: f.fontSize, lineHeight: f.lineHeight, letterSpacing: f.letterSpacing, fontLigatures: f.fontLigatures });
  monaco.editor.remeasureFonts();
}

/** What the window needs to know to fit the editor's font: the text area and the cell, in whole device pixels. Null with no editor on screen. */
export function editorFit(): Fit | null {
  const e = liveEditor;
  if (!e || !e.getModel()) return null;
  const l = e.getLayoutInfo();
  if (l.width <= 0 || l.height <= 0) return null;
  const dpr = window.devicePixelRatio || 1;
  const O = monaco.editor.EditorOption;
  return {
    contentW: Math.round(l.contentWidth * dpr),
    contentH: Math.round((l.height - l.horizontalScrollbarHeight) * dpr),
    cellW: Math.round(e.getOption(O.fontInfo).typicalHalfwidthCharacterWidth * dpr),
    cellH: Math.round(e.getOption(O.lineHeight) * dpr),
    stepW: Math.round(8 * dpr),
    stepH: Math.round(16 * dpr),
  };
}

setFitProvider(() => editorFit());

/** Inserts a snippet (Monaco syntax: `${1:name}`, `$0`) at the cursor and focuses the editor.
 *  False when no file is open in the editor. */
export function insertSnippet(snippet: string): boolean {
  const e = liveEditor;
  if (!e || !e.getModel()) return false;
  e.focus();
  (e.getContribution("snippetController2") as { insert?: (s: string) => void } | null)?.insert?.(snippet);
  return true;
}

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
    defineThemes();
    const editor = monaco.editor.create(host.current!, {
      ...TEXT_MODE_OPTIONS,
      model: null,
      theme: themeId(currentAppearance()),
      automaticLayout: true,
      tabSize: 2,
      ariaLabel: "Code editor",
    });
    // Monaco measures the font when it is created: if the VGA font was not in
    // yet, measure again once it is, so the cells are exactly 8 by 16.
    void applyFont(editor, loadChoice());
    const offFont = onChoice((c) => void applyFont(editor, c));
    ed.current = editor;
    liveEditor = editor;
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
      if (path) {
        touchMarks(docs.get(path)!);
        cb.current.onDirty(path, m.getAlternativeVersionId() !== docs.get(path)!.saved);
        scheduleLint(path, m);
      }
    });
    // Format Document (Shift+Alt+F) with the bundled StyLua; a syntax error is reported, the text left alone.
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
    // Colour swatches on love.graphics.setColor(0.2, 0.4, 1): click to pick, the numbers are rewritten.
    const colors = inTauri
      ? monaco.languages.registerColorProvider("lua", {
          async provideDocumentColors(model) {
            const items = await colorLiterals(model.getValue()).catch(() => []);
            return items.map((c) => {
              const a = model.getPositionAt(c.start);
              const b = model.getPositionAt(c.end);
              return {
                color: { red: c.r, green: c.g, blue: c.b, alpha: c.a },
                range: { startLineNumber: a.lineNumber, startColumn: a.column, endLineNumber: b.lineNumber, endColumn: b.column },
              };
            });
          },
          async provideColorPresentations(_model, info) {
            const { red, green, blue, alpha } = info.color;
            const label = await colorText(red, green, blue, alpha);
            return [{ label, textEdit: { range: info.range, text: label } }];
          },
        })
      : null;
    const offPalette = onAppearance((a) => monaco.editor.setTheme(themeId(a)));
    return () => {
      offPalette();
      offFont();
      formatter?.dispose();
      colors?.dispose();
      sub.dispose();
      editor.dispose();
      ed.current = null;
      if (liveEditor === editor) liveEditor = null;
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
      void applyMarks(props.path, d);
    }
    ed.current?.setModel(d.model);
    scheduleLint(props.path, d.model);
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
