import { invoke } from "@tauri-apps/api/core";

/** True inside the Tauri window; false in a plain browser (`npm run dev`). */
export const inTauri = "__TAURI_INTERNALS__" in window;

export interface CoreInfo {
  version: string;
}

export async function coreInfo(): Promise<CoreInfo> {
  if (!inTauri) return { version: "browser preview" };
  return invoke<CoreInfo>("core_info");
}

export interface Entry {
  path: string;
  name: string;
  dir: boolean;
}
export interface Listing {
  name: string;
  entries: Entry[];
  truncated: boolean;
  hasMain: boolean;
}

/** Asks for a folder; null when cancelled (or in a plain browser). */
export async function chooseFolder(): Promise<string | null> {
  if (!inTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: true, multiple: false, title: "Open a LÖVE project" });
  return typeof r === "string" ? r : null;
}
export const openProject = (path: string) => invoke<Listing>("open_project", { path });
export const readFile = (path: string) => invoke<string>("read_project_file", { path });
export const writeFile = (path: string, text: string) =>
  invoke<void>("write_project_file", { path, text });

export interface RunLine {
  stream: "out" | "err";
  text: string;
  file: string | null;
  line: number | null;
}
export const runProject = () => invoke<void>("run_project");
export const stopProject = () => invoke<void>("stop_project");

/** Subscribes to the game's output and end; returns the unsubscribe. */
export async function onRun(
  line: (l: RunLine) => void,
  exit: (code: number | null) => void,
): Promise<() => void> {
  if (!inTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  const a = await listen<RunLine>("run-line", (e) => line(e.payload));
  const b = await listen<{ code: number | null }>("run-exit", (e) => exit(e.payload.code));
  return () => {
    a();
    b();
  };
}
