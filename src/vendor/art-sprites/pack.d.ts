export const PACK_SCHEMA: 'art-sprites.pack/1';
export interface PackEntry { id?: string; recipe: string; params?: Record<string, any>; [meta: string]: any }
export interface PackList { name?: string; entries: PackEntry[] }
export interface PackFile { file: string; sha256: string }
export interface PackRecord {
  id: string; recipe: string; params: Record<string, any>;
  /** `params` with every default written in; `explicit` is false for a recipe with no resolver. */
  look: { recipe: string; params: Record<string, any>; engine: string; explicit: boolean };
  /** Everything in the entry other than `id`, `recipe` and `params`, as the game wrote it. */
  meta: Record<string, any>;
  style: 'pixel'; projection: string | null; size: { w: number; h: number }; alt: string;
  anchors: Record<string, any> | null; bounds: { x: number; y: number; w: number; h: number } | null; paletteGroups: Record<string, string> | null;
  /** The world data of the art where it has any: `footprint`, `occlusion`, `states` (a prop); `props`, `blocked`, `walkable`, `unreachable`, `entry`, `tile` (a dressed level); `legend`, `cell`, `origin` (a map). */
  data: Record<string, any>;
  fps: number; loop: boolean; reduced: PackFile[]; golden: string; modelledOn: string[]; atlas: { x: number; y: number; w: number; h: number };
  files: { frames: PackFile[]; clips: Record<string, { fps: number; loop: boolean; events: any[]; anchors: any[] | null; frames: PackFile[]; reduced: PackFile[] }>; layers: Record<string, PackFile[]>; /** A boss in parts: each part's file with its offset and the joint it hangs from. */ parts?: (PackFile & { id: string; x: number; y: number; joint: [number, number] })[]; /** The fog mask of a map (1 where unknown). */ mask?: PackFile };
}
export interface PackManifest {
  schema: typeof PACK_SCHEMA; name: string; engine: string; palette: 'DB32';
  atlas: { file: string; w: number; h: number; sha256: string };
  source: { creator: string; packName: string }; license: { licenseName: string; status: string; attributionRequired: boolean; evidenceNotes: string };
  entries: PackRecord[];
}
/** Write `manifest.json`, `goldens.json`, `atlas.png` and every frame, clip and layer PNG into `dir`. Throws on an unknown or ANSI-only recipe and on a duplicate id. */
export function writePack(list: PackList, dir: string, options?: { creator?: string }): PackManifest;
