/** Node only (uses node:fs). The shipped IBM VGA 8x16 glyph atlas, for `renderCells`. */
export function loadAtlas(): { w: number; h: number; rgba: Uint8Array };
