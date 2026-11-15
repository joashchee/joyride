// A small Language Server client for lua-language-server over Tauri events
// (src-tauri/src/lsp.rs). Monaco 0.57's own `monaco-lsp-client` is not
// published to npm, so this covers what the editor needs: document sync
// (full text), diagnostics, completion, hover, signature help,
// go-to-definition, references, rename, document highlights, document
// symbols (outline, Cmd+Shift+O) and code actions (quick fixes). Positions are Monaco 1-based, the protocol 0-based.
import { invoke } from "@tauri-apps/api/core";
import * as monaco from "monaco-editor/editor/editor.api.js";

type Json = any; // eslint-disable-line @typescript-eslint/no-explicit-any

interface Pending {
  resolve: (v: Json) => void;
  reject: (e: Error) => void;
}

let rootUri: string | null = null;
let ready: Promise<void> | null = null; // eslint-disable-line @typescript-eslint/no-unused-vars
let nextId = 1;
const pending = new Map<number, Pending>();
const tracked = new Map<string, { model: monaco.editor.ITextModel; version: number; sub: monaco.IDisposable }>();
const onStatus: ((s: LspStatus) => void)[] = [];
let providers: monaco.IDisposable[] = [];
let offs: (() => void)[] = [];
let session = 0;

export type LspStatus = "off" | "starting" | "ready" | "stopped";
let status: LspStatus = "off";
function setStatus(s: LspStatus) {
  status = s;
  onStatus.forEach((f) => f(s));
}
export function onLspStatus(f: (s: LspStatus) => void) {
  onStatus.push(f);
  f(status);
  return () => void onStatus.splice(onStatus.indexOf(f), 1);
}

const send = (m: Json) => invoke<void>("lsp_send", { message: JSON.stringify({ jsonrpc: "2.0", ...m }) });
const notify = (method: string, params: Json) => send({ method, params }).catch(() => {});
function request<T = Json>(method: string, params: Json): Promise<T> {
  const id = nextId++;
  return new Promise<T>((resolve, reject) => {
    pending.set(id, { resolve, reject });
    send({ id, method, params }).catch((e) => {
      pending.delete(id);
      reject(new Error(String(e)));
    });
  });
}

const key = (u: monaco.Uri | string) => monaco.Uri.parse(String(u)).toString();
const pos = (p: monaco.IPosition) => ({ line: p.lineNumber - 1, character: p.column - 1 });
const range = (r: Json): monaco.IRange => ({
  startLineNumber: r.start.line + 1,
  startColumn: r.start.character + 1,
  endLineNumber: r.end.line + 1,
  endColumn: r.end.character + 1,
});
const doc = (m: monaco.editor.ITextModel) => ({ uri: m.uri.toString() });

/** Names the open project, so models get their URIs before the server is up. */
export async function setProjectRoot() {
  rootUri = await invoke<string>("lsp_root");
}

/** The model URI for a project-relative path. */
export function uriFor(path: string): monaco.Uri | null {
  if (!rootUri) return null;
  return monaco.Uri.parse(rootUri + "/" + path.split("/").map(encodeURIComponent).join("/"));
}

const SEVERITY = [0, monaco.MarkerSeverity.Error, monaco.MarkerSeverity.Warning, monaco.MarkerSeverity.Info, monaco.MarkerSeverity.Hint];

function handle(msg: Json) {
  if (msg.method === undefined && msg.id !== undefined) {
    const p = pending.get(msg.id);
    pending.delete(msg.id);
    if (!p) return;
    msg.error ? p.reject(new Error(msg.error.message ?? "language server error")) : p.resolve(msg.result);
    return;
  }
  if (msg.method === "textDocument/publishDiagnostics") {
    const t = tracked.get(key(msg.params.uri));
    if (!t) return;
    monaco.editor.setModelMarkers(
      t.model,
      "lua-language-server",
      (msg.params.diagnostics as Json[]).map((d) => ({
        ...range(d.range),
        severity: SEVERITY[d.severity ?? 1],
        message: d.message,
        source: d.source,
        code: d.code === undefined ? undefined : String(d.code),
      })),
    );
    return;
  }
  if (msg.id !== undefined && msg.method) {
    // Requests from the server: answer what we can, refuse the rest.
    if (msg.method === "workspace/configuration") {
      invoke<Json>("lsp_settings").then((s) => send({ id: msg.id, result: (msg.params.items as Json[]).map(() => s) }));
    } else if (msg.method === "workspace/workspaceFolders") {
      void send({ id: msg.id, result: [{ uri: rootUri, name: "project" }] });
    } else {
      void send({ id: msg.id, result: null });
    }
  }
}

function markdown(c: Json): monaco.IMarkdownString[] {
  const one = (x: Json): string => (typeof x === "string" ? x : x?.value ?? "");
  const text = Array.isArray(c) ? c.map(one).join("\n\n") : one(c);
  return text ? [{ value: text }] : [];
}

const KINDS = monaco.languages.CompletionItemKind;
// LSP CompletionItemKind (1-based) → Monaco's.
const KIND_MAP = [KINDS.Text, KINDS.Text, KINDS.Method, KINDS.Function, KINDS.Constructor, KINDS.Field, KINDS.Variable, KINDS.Class, KINDS.Interface, KINDS.Module, KINDS.Property, KINDS.Unit, KINDS.Value, KINDS.Enum, KINDS.Keyword, KINDS.Snippet, KINDS.Color, KINDS.File, KINDS.Reference, KINDS.Folder, KINDS.EnumMember, KINDS.Constant, KINDS.Struct, KINDS.Event, KINDS.Operator, KINDS.TypeParameter];

function register() {
  const lang = "lua";
  providers.push(
    monaco.languages.registerCompletionItemProvider(lang, {
      triggerCharacters: [".", ":", "(", "[", '"', "'", "/", "@"],
      async provideCompletionItems(model, p) {
        if (!tracked.has(key(model.uri))) return { suggestions: [] };
        const r = await request<Json>("textDocument/completion", { textDocument: doc(model), position: pos(p) }).catch(() => null);
        const items: Json[] = Array.isArray(r) ? r : r?.items ?? [];
        const w = model.getWordUntilPosition(p);
        const dflt = { startLineNumber: p.lineNumber, endLineNumber: p.lineNumber, startColumn: w.startColumn, endColumn: w.endColumn };
        return {
          incomplete: !!r?.isIncomplete,
          suggestions: items.map((i) => {
            const e = i.textEdit;
            const snippet = i.insertTextFormat === 2;
            return {
              label: i.label,
              kind: KIND_MAP[(i.kind ?? 1) - 1] ?? KINDS.Text,
              detail: i.detail,
              documentation: i.documentation ? markdown(i.documentation)[0] : undefined,
              insertText: e?.newText ?? i.insertText ?? i.label,
              insertTextRules: snippet ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet : undefined,
              range: e?.range ? range(e.range) : e?.insert ? range(e.insert) : dflt,
              sortText: i.sortText,
              filterText: i.filterText,
            };
          }),
        };
      },
    }),
    monaco.languages.registerHoverProvider(lang, {
      async provideHover(model, p) {
        if (!tracked.has(key(model.uri))) return null;
        const r = await request<Json>("textDocument/hover", { textDocument: doc(model), position: pos(p) }).catch(() => null);
        if (!r?.contents) return null;
        return { contents: markdown(r.contents), range: r.range ? range(r.range) : undefined };
      },
    }),
    monaco.languages.registerSignatureHelpProvider(lang, {
      signatureHelpTriggerCharacters: ["(", ","],
      async provideSignatureHelp(model, p) {
        if (!tracked.has(key(model.uri))) return null;
        const r = await request<Json>("textDocument/signatureHelp", { textDocument: doc(model), position: pos(p) }).catch(() => null);
        if (!r?.signatures?.length) return null;
        return {
          value: {
            activeSignature: r.activeSignature ?? 0,
            activeParameter: r.activeParameter ?? 0,
            signatures: (r.signatures as Json[]).map((s) => ({
              label: s.label,
              documentation: s.documentation ? markdown(s.documentation)[0] : undefined,
              parameters: (s.parameters ?? []).map((q: Json) => ({ label: q.label, documentation: q.documentation ? markdown(q.documentation)[0] : undefined })),
            })),
          },
          dispose() {},
        };
      },
    }),
    monaco.languages.registerDefinitionProvider(lang, {
      async provideDefinition(model, p) {
        if (!tracked.has(key(model.uri))) return null;
        const r = await request<Json>("textDocument/definition", { textDocument: doc(model), position: pos(p) }).catch(() => null);
        const list: Json[] = !r ? [] : Array.isArray(r) ? r : [r];
        return list.map((l) => ({
          uri: monaco.Uri.parse(l.uri ?? l.targetUri),
          range: range(l.range ?? l.targetSelectionRange),
        }));
      },
    }),
  );
}

const SYMBOL_KINDS = monaco.languages.SymbolKind;
// LSP SymbolKind (1-based) → Monaco's.
const SYMBOL_MAP = [SYMBOL_KINDS.File, SYMBOL_KINDS.Module, SYMBOL_KINDS.Namespace, SYMBOL_KINDS.Package, SYMBOL_KINDS.Class, SYMBOL_KINDS.Method, SYMBOL_KINDS.Property, SYMBOL_KINDS.Field, SYMBOL_KINDS.Constructor, SYMBOL_KINDS.Enum, SYMBOL_KINDS.Interface, SYMBOL_KINDS.Function, SYMBOL_KINDS.Variable, SYMBOL_KINDS.Constant, SYMBOL_KINDS.String, SYMBOL_KINDS.Number, SYMBOL_KINDS.Boolean, SYMBOL_KINDS.Array, SYMBOL_KINDS.Object, SYMBOL_KINDS.Key, SYMBOL_KINDS.Null, SYMBOL_KINDS.EnumMember, SYMBOL_KINDS.Struct, SYMBOL_KINDS.Event, SYMBOL_KINDS.Operator, SYMBOL_KINDS.TypeParameter];

/** LSP DocumentSymbol (nested) or SymbolInformation (flat) → Monaco's outline. */
function toSymbol(s: Json): monaco.languages.DocumentSymbol {
  const r = s.range ?? s.location.range;
  return {
    name: s.name,
    detail: s.detail ?? s.containerName ?? "",
    kind: SYMBOL_MAP[(s.kind ?? 13) - 1] ?? SYMBOL_KINDS.Variable,
    tags: [],
    range: range(r),
    selectionRange: range(s.selectionRange ?? r),
    children: (s.children ?? []).map(toSymbol),
  };
}

/** Turns an LSP WorkspaceEdit into Monaco's, or says which closed files it needs. */
function toWorkspaceEdit(e: Json): monaco.languages.WorkspaceEdit {
  const byUri: Record<string, Json[]> = { ...(e?.changes ?? {}) };
  for (const c of e?.documentChanges ?? []) if (c.textDocument) (byUri[c.textDocument.uri] ??= []).push(...c.edits);
  const closed = Object.keys(byUri).filter((u) => !tracked.has(key(u)));
  if (closed.length) {
    const names = closed.map((u) => decodeURIComponent(u.slice(u.lastIndexOf("/") + 1)));
    throw new Error(`Open ${names.slice(0, 5).join(", ")}${names.length > 5 ? ` and ${names.length - 5} more` : ""} first: a rename only changes files that are open.`);
  }
  return {
    edits: Object.entries(byUri).flatMap(([u, edits]) =>
      edits.map((t) => ({ resource: monaco.Uri.parse(u), versionId: undefined, textEdit: { range: range(t.range), text: t.newText } })),
    ),
  };
}

function registerNavigation() {
  const lang = "lua";
  providers.push(
    monaco.languages.registerReferenceProvider(lang, {
      async provideReferences(model, p, ctx) {
        if (!tracked.has(key(model.uri))) return null;
        const r = await request<Json[]>("textDocument/references", { textDocument: doc(model), position: pos(p), context: { includeDeclaration: ctx.includeDeclaration } }).catch(() => null);
        return (r ?? []).map((l) => ({ uri: monaco.Uri.parse(l.uri), range: range(l.range) }));
      },
    }),
    monaco.languages.registerDocumentHighlightProvider(lang, {
      async provideDocumentHighlights(model, p) {
        if (!tracked.has(key(model.uri))) return null;
        const r = await request<Json[]>("textDocument/documentHighlight", { textDocument: doc(model), position: pos(p) }).catch(() => null);
        return (r ?? []).map((h) => ({ range: range(h.range), kind: [0, monaco.languages.DocumentHighlightKind.Text, monaco.languages.DocumentHighlightKind.Read, monaco.languages.DocumentHighlightKind.Write][h.kind ?? 1] }));
      },
    }),
    monaco.languages.registerDocumentSymbolProvider(lang, {
      displayName: "Lua",
      async provideDocumentSymbols(model) {
        if (!tracked.has(key(model.uri))) return null;
        const r = await request<Json[]>("textDocument/documentSymbol", { textDocument: doc(model) }).catch(() => null);
        return (r ?? []).map(toSymbol);
      },
    }),
    monaco.languages.registerRenameProvider(lang, {
      async resolveRenameLocation(model, p) {
        if (!tracked.has(key(model.uri))) return { text: "", range: new monaco.Range(1, 1, 1, 1), rejectReason: "Open the file in the editor first." };
        const r = await request<Json>("textDocument/prepareRename", { textDocument: doc(model), position: pos(p) }).catch(() => null);
        const rg = r?.range ?? r;
        if (!rg?.start) return { text: "", range: new monaco.Range(p.lineNumber, p.column, p.lineNumber, p.column), rejectReason: "There is no name to rename here." };
        const mr = range(rg);
        return { range: mr, text: model.getValueInRange(mr) };
      },
      async provideRenameEdits(model, p, newName) {
        const r = await request<Json>("textDocument/rename", { textDocument: doc(model), position: pos(p), newName }).catch((e: Error) => {
          throw e;
        });
        if (!r) return { edits: [], rejectReason: "The language server cannot rename this." };
        try {
          return toWorkspaceEdit(r);
        } catch (e) {
          return { edits: [], rejectReason: String((e as Error).message) };
        }
      },
    }),
    monaco.languages.registerCodeActionProvider(lang, {
      async provideCodeActions(model, rg, ctx) {
        if (!tracked.has(key(model.uri))) return null;
        const diagnostics = ctx.markers
          .filter((m) => m.source !== undefined)
          .map((m) => ({ range: { start: { line: m.startLineNumber - 1, character: m.startColumn - 1 }, end: { line: m.endLineNumber - 1, character: m.endColumn - 1 } }, message: m.message, severity: m.severity === monaco.MarkerSeverity.Error ? 1 : m.severity === monaco.MarkerSeverity.Warning ? 2 : 3, source: m.source, code: m.code }));
        const r = await request<Json[]>("textDocument/codeAction", {
          textDocument: doc(model),
          range: { start: { line: rg.startLineNumber - 1, character: rg.startColumn - 1 }, end: { line: rg.endLineNumber - 1, character: rg.endColumn - 1 } },
          context: { diagnostics, only: ctx.only ? [ctx.only] : undefined },
        }).catch(() => null);
        const actions: monaco.languages.CodeAction[] = [];
        for (const a of r ?? []) {
          if (!a.edit) continue; // commands the client would have to run are not supported
          try {
            actions.push({ title: a.title, kind: a.kind, isPreferred: a.isPreferred, edit: toWorkspaceEdit(a.edit) });
          } catch {
            /* an action that needs a closed file is left out */
          }
        }
        return { actions, dispose() {} };
      },
    }),
  );
}

/** Starts the server for the open project. Safe to call again for another project. */
export function startLsp(): Promise<void> {
  stopLsp();
  setStatus("starting");
  ready = (async () => {
    const { listen } = await import("@tauri-apps/api/event");
    // Events carry the session that sent them; a stopped server's late
    // events (its end, above all) must not touch the next session.
    offs = [
      await listen<{ session: number; text: string }>("lsp-message", (e) => e.payload.session === session && handle(JSON.parse(e.payload.text))),
      await listen<{ session: number }>("lsp-end", (e) => {
        if (e.payload.session !== session) return;
        pending.forEach((p) => p.reject(new Error("the language server stopped")));
        pending.clear();
        setStatus("stopped");
      }),
    ];
    const started = await invoke<{ uri: string; session: number }>("lsp_start");
    session = started.session;
    rootUri = started.uri;
    // A server that cannot start is reported by the rejection above; the
    // editor keeps working without completions.
    await request("initialize", {
      processId: null,
      clientInfo: { name: "JoyrIDE" },
      rootUri,
      workspaceFolders: [{ uri: rootUri, name: "project" }],
      capabilities: {
        workspace: { configuration: true, workspaceFolders: true },
        textDocument: {
          synchronization: {},
          publishDiagnostics: {},
          hover: { contentFormat: ["markdown", "plaintext"] },
          completion: { completionItem: { snippetSupport: true, documentationFormat: ["markdown", "plaintext"] } },
          signatureHelp: { signatureInformation: { documentationFormat: ["markdown", "plaintext"] } },
          definition: {},
          references: {},
          documentHighlight: {},
          documentSymbol: { hierarchicalDocumentSymbolSupport: true },
          rename: { prepareSupport: true },
          codeAction: { codeActionLiteralSupport: { codeActionKind: { valueSet: ["quickfix", "refactor", "source"] } } },
        },
      },
    });
    await notify("initialized", {});
    register();
    registerNavigation();
    setStatus("ready");
    // Documents opened before the server was ready.
    tracked.forEach((t) => open(t.model, t));
  })().catch((e) => {
    setStatus("stopped");
    throw e;
  });
  return ready;
}

export function stopLsp() {
  offs.forEach((o) => o());
  offs = [];
  session = 0;
  pending.forEach((p) => p.reject(new Error("the language server stopped")));
  pending.clear();
  providers.forEach((p) => p.dispose());
  providers = [];
  tracked.forEach((t) => {
    monaco.editor.setModelMarkers(t.model, "lua-language-server", []);
  });
  void invoke("lsp_stop").catch(() => {});
  ready = null;
  if (status !== "off") setStatus("off");
}

function open(model: monaco.editor.ITextModel, t: { version: number }) {
  void notify("textDocument/didOpen", { textDocument: { uri: model.uri.toString(), languageId: "lua", version: t.version, text: model.getValue() } });
}

/** Starts syncing a Lua model with the server (and again after a restart). */
export function trackModel(model: monaco.editor.ITextModel) {
  const k = key(model.uri);
  if (tracked.has(k)) return;
  const t = { model, version: 1, sub: model.onDidChangeContent(() => {
    t.version++;
    if (status === "ready") void notify("textDocument/didChange", { textDocument: { uri: model.uri.toString(), version: t.version }, contentChanges: [{ text: model.getValue() }] });
  }) };
  tracked.set(k, t);
  if (status === "ready") open(model, t);
}

export function untrackModel(model: monaco.editor.ITextModel) {
  const k = key(model.uri);
  const t = tracked.get(k);
  if (!t) return;
  t.sub.dispose();
  tracked.delete(k);
  monaco.editor.setModelMarkers(model, "lua-language-server", []);
  if (status === "ready") void notify("textDocument/didClose", { textDocument: doc(model) });
}

