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

export interface FileWarning {
  path: string;
  message: string;
}
export const projectWarnings = () => invoke<FileWarning[]>("project_warnings");

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
export const lastProject = () => (inTauri ? invoke<string | null>("last_project") : Promise.resolve(null));
export const openProject = (path: string) => invoke<Listing>("open_project", { path });
export const projectListing = () => invoke<Listing>("project_listing");
export const readFile = (path: string) => invoke<string>("read_project_file", { path });

export interface PreviewFact {
  label: string;
  value: string;
}
export type PreviewBody =
  | { type: "text" }
  | { type: "image"; dataUrl: string; width: number; height: number; pixelArt: boolean; palette: string[] }
  | { type: "audio"; dataUrl: string | null; waveform: number[] | null; seconds: number | null }
  | { type: "video"; source: "ogv" | "web"; dataUrl: string | null; width: number; height: number; fps: number; frames: number | null; seconds: number | null; poster: string | null }
  | { type: "font"; dataUrl: string }
  | { type: "members"; names: { name: string; size: number }[]; more: number }
  | { type: "hex"; dump: string };
export interface Preview {
  path: string;
  kind: "text" | "image" | "audio" | "video" | "font" | "archive" | "music" | "binary";
  size: number;
  facts: PreviewFact[];
  note: string | null;
  body: PreviewBody;
  /** Music written as text: the panel offers Edit. */
  editable: boolean;
}
/** What the selected file is and how to show it ("text" goes to the editor). */
export const previewFile = (path: string) => invoke<Preview>("preview_file", { path });
export interface VideoFrame {
  index: number;
  time: number;
  jpeg: string;
}
/** Up to `count` frames of an Ogg Theora video from frame `from`. */
export const previewVideoFrames = (path: string, from: number, count: number) =>
  invoke<{ frames: VideoFrame[]; end: boolean }>("preview_video_frames", { path, from, count });
export const writeFile = (path: string, text: string) =>
  invoke<void>("write_project_file", { path, text });

export const createFile = (path: string) => invoke<Listing>("create_project_file", { path });
export const createDir = (path: string) => invoke<Listing>("create_project_dir", { path });
export const renamePath = (from: string, to: string) => invoke<Listing>("rename_project_path", { from, to });
export const deletePath = (path: string) => invoke<Listing>("delete_project_path", { path });

export const formatLua = (text: string) => invoke<string>("format_lua", { text });

export interface LintItem {
  line: number;
  severity: "error" | "warning";
  rule: string;
  message: string;
}
export const lintLua = (path: string, text: string) => invoke<LintItem[]>("lint_lua", { path, text });

export interface ColorItem {
  start: number;
  end: number;
  r: number;
  g: number;
  b: number;
  a: number;
}
export const colorLiterals = (text: string) => invoke<ColorItem[]>("color_literals", { text });
export const colorText = (r: number, g: number, b: number, a: number) => invoke<string>("color_text", { r, g, b, a });

export interface Hit {
  path: string;
  line: number;
  text: string;
}
export const searchProject = (query: string, caseSensitive: boolean) =>
  invoke<{ hits: Hit[]; truncated: boolean }>("search_project", { query, caseSensitive });

export interface UseRow {
  api: string;
  support: "exact" | "reduced" | "unsupported";
  note: string;
  file: string;
  line: number;
  count: number;
}
export interface ExportInfo {
  loveVersion: string;
  variantOffered: boolean;
  uses: UseRow[];
  accepted: string[];
  unusable: string[];
}
export const exportInfo = () => invoke<ExportInfo>("export_info");
export const exportProject = (kind: "love" | "portmaster", variant: boolean, accept: string[], commercial: boolean) =>
  invoke<string>("export_project", { kind, variant, accept, commercial });

export interface LibraryView {
  name: string;
  summary: string;
  licence: string;
  git: string;
  version: string;
  badge: string;
  broken: boolean;
  installed: boolean;
  /** "Lua", "C library (FFI)" or "C module". */
  kind: string;
  /** For a C library: the binary that has to sit next to the game. */
  native: string | null;
  /** A warning to read before adding it. */
  caution: string | null;
}
export const libraryList = (query: string) => invoke<LibraryView[]>("library_list", { query });
export const libraryAdd = (name: string, query: string) => invoke<LibraryView[]>("library_add", { name, query });
export const libraryRemove = (name: string, query: string) => invoke<LibraryView[]>("library_remove", { name, query });

export interface SampleInfo {
  id: string;
  name: string;
  summary: string;
}
/** The sample projects that ship with the app (a copy is made in the app-data folder before one is opened). */
export const sampleList = () => (inTauri ? invoke<SampleInfo[]>("sample_list") : Promise.resolve([] as SampleInfo[]));
export const sampleCopy = (id: string, fresh: boolean) => invoke<string>("sample_copy", { id, fresh });

export interface DocFound {
  name: string;
  kind: string;
  summary: string;
}
export const docsSearch = (query: string) => invoke<{ version: string; results: DocFound[] }>("docs_search", { query });
export const docsGet = (name: string) => invoke<{ text: string; snippet: string | null } | null>("docs_get", { name });

export interface PortFinding {
  level: "info" | "warning" | "blocker";
  file: string;
  line: number;
  rule: string;
  message: string;
}
export interface PortTarget {
  name: string;
  title: string;
  findings: PortFinding[];
  blockers: number;
  overlayFiles: string[];
}
export interface PortReport {
  files: number;
  keyboard: boolean;
  mouse: boolean;
  touch: boolean;
  gamepad: boolean;
  keys: string[];
  targets: PortTarget[];
}
export const autoportScan = () => invoke<PortReport>("autoport_scan");
export const autoportGenerate = (target: string) => invoke<PortReport>("autoport_generate", { target });

export interface GhAuth {
  installed: boolean;
  loggedIn: boolean;
  user: string | null;
  message: string;
}
export interface GitChange {
  status: string;
  path: string;
}
export interface GitStatus {
  isRepo: boolean;
  branch: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  changes: GitChange[];
}
export interface GithubOverview {
  auth: GhAuth;
  repo: { nameWithOwner: string; url: string; visibility: string } | null;
  git: GitStatus;
  suggestedName: string;
}
export const githubOverview = () => invoke<GithubOverview>("github_overview");
export const gitInit = () => invoke<GithubOverview>("git_init");
export const gitCommit = (message: string) => invoke<GithubOverview>("git_commit", { message });
export const gitPush = () => invoke<GithubOverview>("git_push");
export const gitPull = () => invoke<GithubOverview>("git_pull");
export const githubCreateRepo = (name: string, isPrivate: boolean) =>
  invoke<GithubOverview>("github_create_repo", { name, private: isPrivate });
export const openGithubUrl = (url: string) => invoke<void>("open_github_url", { url });
export const githubReleaseWorkflow = () => invoke<string>("github_release_workflow");

export interface RunLine {
  stream: "out" | "err";
  text: string;
  file: string | null;
  line: number | null;
}
/** `screen` is a simulator preset id (see `SCREENS`); empty runs as it is. */
export const runProject = (screen = "") => invoke<void>("run_project", { screen: screen || null });

/** The simulated screens (mirrors `joyride_core::simulator::PRESETS`). */
export const SCREENS: { id: string; name: string }[] = [
  { id: "", name: "Desktop (as is)" },
  { id: "pm-640x480", name: "PortMaster 640x480 (4:3)" },
  { id: "pm-720x720", name: "PortMaster 720x720 (1:1)" },
  { id: "pm-1280x720", name: "PortMaster 1280x720 (16:9)" },
  { id: "iphone", name: "iPhone landscape 844x390" },
  { id: "iphone-portrait", name: "iPhone portrait 390x844" },
  { id: "ipad", name: "iPad landscape 1180x820" },
  { id: "android", name: "Android landscape 915x412" },
  { id: "android-portrait", name: "Android portrait 412x915" },
];
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

/** Asks for a font file; null when cancelled (or in a plain browser). */
export async function chooseFontFile(): Promise<string | null> {
  if (!inTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ multiple: false, title: "Add a font", filters: [{ name: "Fonts", extensions: ["ttf", "otf", "ttc", "woff", "woff2"] }] });
  return typeof r === "string" ? r : null;
}
export interface FontPlan {
  source: string;
  fileName: string;
  facts: PreviewFact[];
  dataUrl: string | null;
  guess: { spdx: string; from: string } | null;
  licenceText: string;
  licenceUrl: string;
  designer: string;
  suggestedDest: string;
  warnings: string[];
  alreadyInProject: string | null;
}
export const fontInspect = (source: string) => invoke<FontPlan>("font_inspect", { source });
export interface FontAddRequest {
  source: string;
  dest: string;
  licence: string;
  confirmedBy: string;
  note: string;
  author: string;
  sourceUrl: string;
  attribution: string;
}
export const fontAdd = (request: FontAddRequest) => invoke<{ dest: string; alreadyInLibrary: boolean }>("font_add", { request });

export interface GameLink {
  kind: string;
  url: string;
}
export interface GameDownload {
  url: string;
  platform: string;
  version: string | null;
  date: string | null;
  size: number | null;
}
export interface GameView {
  id: string;
  name: string;
  kind: "game" | "app";
  summary: string;
  authors: string[];
  tags: string[];
  licence: string;
  licenceFrom: string | null;
  licenceConfirmed: boolean;
  openSource: boolean;
  latest: string | null;
  loveVersion: string | null;
  archived: boolean;
  checked: string | null;
  sources: GameLink[];
  downloads: GameDownload[];
}
export interface GamesResult {
  games: GameView[];
  total: number;
  generated: string;
}
export const gamesSearch = (query: string, kind: string, platform: string, openSource: boolean, includeArchived: boolean) =>
  invoke<GamesResult>("games_search", { query, kind, platform, openSource, includeArchived });
export const gamesOpenUrl = (url: string) => invoke<void>("games_open_url", { url });

// ---- Asset Library ----
export interface AssetView {
  id: number;
  name: string;
  kind: string;
  size: number;
  spdx: string;
  status: "confirmed" | "guessed" | "unknown";
  confirmedBy: string;
  commercial: "yes" | "no" | "unknown";
  attributionRequired: boolean;
  shareAlike: boolean;
  author: string;
  sourceUrl: string;
  tags: string[];
  thumb: string | null;
  uses: string[];
}
export interface AssetList {
  assets: AssetView[];
  matched: number;
  total: number;
}
export interface PlannedView {
  hash: string;
  name: string;
  kind: string;
  size: number;
  paths: string[];
  guess: string | null;
  guessFrom: string | null;
  existing: number | null;
  existingLicence: string | null;
}
export interface ImportPlanView {
  project: string;
  assets: PlannedView[];
  duplicateBytes: number;
  unreadable: string[];
}
export interface DecisionIn {
  hash: string;
  licence: string;
  confirmedBy: string;
  note: string;
  author: string;
  sourceUrl: string;
  attribution: string;
}
export interface ImportReportView {
  imported: number;
  linked: number;
  skipped: number;
  conflicts: { name: string; existing: string; proposed: string }[];
  failed: string[];
  bytesSaved: number;
}
export interface ConfirmIn {
  id: number;
  licence: string;
  confirmedBy: string;
  note: string;
  author: string;
  sourceUrl: string;
  attribution: string;
}
export const assetsSearch = (query: string, kind: string, confirmedOnly: boolean, commercialOnly: boolean) =>
  invoke<AssetList>("assets_search", { query, kind, confirmedOnly, commercialOnly });
export const assetsImportPlan = (includeCode: boolean) => invoke<ImportPlanView>("assets_import_plan", { includeCode });
export const assetsImportApply = (includeCode: boolean, decisions: DecisionIn[]) => invoke<ImportReportView>("assets_import_apply", { includeCode, decisions });
export const assetsConfirm = (request: ConfirmIn) => invoke<void>("assets_confirm", { request });
export const assetsUse = (id: number, dest: string) => invoke<string>("assets_use", { id, dest });
