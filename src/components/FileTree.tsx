import { useState } from "react";
import type { Entry } from "../lib/backend";

/** The project's files. Folders open and close; Enter or a click opens a file. */
export default function FileTree(props: {
  entries: Entry[];
  active: string | null;
  onOpen: (path: string) => void;
  /** The entry the New/Rename/Delete buttons act on. */
  selected: string | null;
  onSelect: (path: string) => void;
  /** Files LÖVE cannot use, by path, with the reason. */
  warnings?: Map<string, string>;
}) {
  const [closed, setClosed] = useState<Set<string>>(new Set());
  const hidden = (e: Entry) => [...closed].some((c) => e.path.startsWith(c + "/"));
  const toggle = (p: string) =>
    setClosed((s) => {
      const n = new Set(s);
      n.has(p) ? n.delete(p) : n.add(p);
      return n;
    });
  return (
    <ul className="tree" role="tree" aria-label="Project files">
      {props.entries.filter((e) => !hidden(e)).map((e) => {
        const depth = e.path.split("/").length - 1;
        return (
          <li key={e.path} role="none">
            <button
              role="treeitem"
              aria-level={depth + 1}
              aria-expanded={e.dir ? !closed.has(e.path) : undefined}
              aria-selected={e.path === props.selected}
              className={"tree-item" + (e.path === props.active ? " active" : "") + (e.path === props.selected ? " selected" : "")}
              style={{ paddingLeft: 8 + depth * 8 }}
              title={props.warnings?.get(e.path)}
              aria-label={props.warnings?.has(e.path) ? `${e.name}, warning: ${props.warnings.get(e.path)}` : undefined}
              onClick={() => {
                props.onSelect(e.path);
                e.dir ? toggle(e.path) : props.onOpen(e.path);
              }}
            >
              <span aria-hidden="true">{e.dir ? (closed.has(e.path) ? "►" : "▼") : "·"}</span> {e.name}{props.warnings?.has(e.path) && <span aria-hidden="true"> !</span>}
            </button>
          </li>
        );
      })}
    </ul>
  );
}
