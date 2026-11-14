/**
 * Window sizes that fit the editor's font. The shell is cut to whole 8 by 16 cells, and inside it Monaco shows whole characters and
 * whole lines of the chosen coding font; what is left over (a sliver of a character at the right, half a line at the bottom) is the
 * awkward part. A resize that ends with such a leftover is nudged, in whole shell cells, to the nearest size where there is none.
 * All sums are in device pixels (whole numbers), so a 1.5x or 2x screen works the same as 1x.
 */

/**
 * The fewest `step`-sized moves k (positive or negative) that make `content + k * step` a whole number of `cell`s, or null when no
 * move does (the cell and the step share no way to line up) or the nearest would leave less than `minCells` cells.
 */
export function snapMoves(content: number, cell: number, step: number, minCells = 20): number | null {
  if (!(cell > 0) || !(step > 0)) return null;
  for (let d = 0; d <= cell + minCells; d++) {
    for (const k of d === 0 ? [0] : [d, -d]) {
      const c = content + k * step;
      if (c >= minCells * cell && c % cell === 0) return k;
    }
  }
  return null;
}

export interface Fit {
  /** Whole-device-pixel measures of the editor's text area and its cell. */
  contentW: number;
  contentH: number;
  cellW: number;
  cellH: number;
  /** The shell's cell in device pixels (8 and 16 CSS px times dpr). */
  stepW: number;
  stepH: number;
}

/** How much to change the window's inner size, in CSS px, so the editor's text area is whole cells. Zero when it already is. */
export function snapDelta(fit: Fit, dpr: number): { dw: number; dh: number } {
  const kw = snapMoves(fit.contentW, fit.cellW, fit.stepW);
  const kh = snapMoves(fit.contentH, fit.cellH, fit.stepH, 4);
  return { dw: kw === null ? 0 : (kw * fit.stepW) / dpr, dh: kh === null ? 0 : (kh * fit.stepH) / dpr };
}
