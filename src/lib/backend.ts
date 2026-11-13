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
