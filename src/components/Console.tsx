import { useEffect, useRef } from "react";
import type { RunLine } from "../lib/backend";
import Panel from "./Panel";

/** The game's output. A line that names a project file and line is a button
 *  that opens it there. */
export default function Console(props: {
  lines: RunLine[];
  running: boolean;
  onClear: () => void;
  onOpen: (file: string, line: number) => void;
}) {
  const body = useRef<HTMLDivElement>(null);
  // Keep the newest line showing; scrolled by the box itself, never the page.
  useEffect(() => {
    if (body.current) body.current.scrollTop = body.current.scrollHeight;
  }, [props.lines.length]);
  return (
    <Panel title={props.running ? "Console (running)" : "Console"} kind="panel-console" actions={<button onClick={props.onClear}>Clear</button>}>
      <div ref={body} className="console-body" role="log" aria-live="off" tabIndex={0}>
        {props.lines.map((l, i) =>
          l.file && l.line ? (
            <button key={i} className={"cline link " + l.stream} onClick={() => props.onOpen(l.file!, l.line!)}>
              {l.text}
            </button>
          ) : (
            <div key={i} className={"cline " + l.stream}>{l.text || " "}</div>
          ),
        )}
      </div>
    </Panel>
  );
}
