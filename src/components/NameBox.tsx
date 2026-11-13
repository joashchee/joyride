import { useEffect, useRef } from "react";

/** A one-line name prompt (WKWebView has no `window.prompt`). Enter accepts, Escape cancels. */
export default function NameBox(props: {
  label: string;
  initial: string;
  onSubmit: (name: string) => void;
  onCancel: () => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const i = input.current!;
    i.focus();
    // Select the file name part, not the folder or the extension.
    const start = i.value.lastIndexOf("/") + 1;
    const dot = i.value.lastIndexOf(".");
    i.setSelectionRange(start, dot > start ? dot : i.value.length);
  }, []);
  return (
    <form
      className="namebox"
      onSubmit={(e) => {
        e.preventDefault();
        const v = input.current!.value.trim();
        v ? props.onSubmit(v) : props.onCancel();
      }}
    >
      <label>
        {props.label}
        <input
          ref={input}
          defaultValue={props.initial}
          spellCheck={false}
          autoCapitalize="off"
          onKeyDown={(e) => e.key === "Escape" && props.onCancel()}
        />
      </label>
      <button type="submit">OK</button>
      <button type="button" onClick={props.onCancel}>Cancel</button>
    </form>
  );
}
