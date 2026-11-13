import { useEffect, useRef } from "react";
import type { RunLine } from "../lib/backend";

/** The game's output. A line that names a project file and line is a button
 *  that opens it there. */
export default function Console(props: {
  lines: RunLine[];
  running: boolean;
  onClear: () => void;
  onOpen: (file: string, line: number) => void;
}) {
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => end.current?.scrollIntoView({ block: "end" }), [props.lines.length]);
  return (
    <section className="console" aria-label="Console">
      <div className="console-head">
        <span>Console{props.running ? " (running)" : ""}</span>
        <button onClick={props.onClear}>Clear</button>
      </div>
      <div className="console-body" role="log" aria-live="off" tabIndex={0}>
        {props.lines.map((l, i) =>
          l.file && l.line ? (
            <button key={i} className={"cline link " + l.stream} onClick={() => props.onOpen(l.file!, l.line!)}>
              {l.text}
            </button>
          ) : (
            <div key={i} className={"cline " + l.stream}>{l.text || " "}</div>
          ),
        )}
        <div ref={end} />
      </div>
    </section>
  );
}
