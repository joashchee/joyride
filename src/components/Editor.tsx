import { useEffect, useRef } from "react";
import * as monaco from "monaco-editor/editor/editor.api.js";
import "../monaco-setup";
import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";

// Monaco's own web worker, bundled: nothing is fetched from a CDN (CLAUDE.md
// rule 1). Lua needs no language worker; `lua-language-server` arrives
// through lsp_bridge later.
(self as unknown as { MonacoEnvironment: monaco.Environment }).MonacoEnvironment = {
  getWorker: () => new EditorWorker(),
};

export default function Editor(props: {
  value: string;
  language?: string;
  onChange?: (value: string) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const onChange = useRef(props.onChange);
  onChange.current = props.onChange;

  useEffect(() => {
    const dark = window.matchMedia("(prefers-color-scheme: dark)");
    const editor = monaco.editor.create(host.current!, {
      value: props.value,
      language: props.language ?? "lua",
      theme: dark.matches ? "vs-dark" : "vs",
      automaticLayout: true,
      minimap: { enabled: false },
      fontSize: 14,
      tabSize: 2,
      ariaLabel: "Code editor",
    });
    const sub = editor.onDidChangeModelContent(() => onChange.current?.(editor.getValue()));
    const theme = () => monaco.editor.setTheme(dark.matches ? "vs-dark" : "vs");
    dark.addEventListener("change", theme);
    return () => {
      dark.removeEventListener("change", theme);
      sub.dispose();
      editor.getModel()?.dispose();
      editor.dispose();
    };
    // The editor owns its text after creation; `value` is only the first.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return <div ref={host} className="editor-host" />;
}
