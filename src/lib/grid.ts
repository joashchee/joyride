/**
 * The text-mode grid (docs/text-mode-ui.md, from the ANSIapps theme's "The
 * grid"): every character sits in a cell of the 8 by 16 grid, counted from
 * the screen's top left. The stylesheet does the placing; this file does the
 * two things CSS can't.
 */
export const CELL_WIDTH = 8;
export const CELL_HEIGHT = 16;

/**
 * Scrolling comes to rest on a whole row, so scrolled text is on the grid
 * too. One listener for every scrolling box on the screen (Monaco keeps its
 * own scrolling, which is already in whole lines). Returns the remover.
 */
export function installRowSnap(): () => void {
  const timers = new Map<Element, number>();
  const onScroll = (e: Event) => {
    const el = e.target;
    if (!(el instanceof HTMLElement) || el.closest(".monaco-editor")) return;
    window.clearTimeout(timers.get(el));
    // Once the scrolling has stopped, not during it: that would fight the trackpad.
    timers.set(
      el,
      window.setTimeout(() => {
        timers.delete(el);
        const snapped = Math.round(el.scrollTop / CELL_HEIGHT) * CELL_HEIGHT;
        if (snapped !== el.scrollTop) el.scrollTop = snapped;
      }, 120),
    );
  };
  document.addEventListener("scroll", onScroll, true);
  return () => {
    document.removeEventListener("scroll", onScroll, true);
    timers.forEach((t) => window.clearTimeout(t));
  };
}

export interface GridReport {
  checked: number;
  /** What's off the grid, in words: the text and where its first character is. */
  off: string[];
}

const MARK = "grid-off";

/**
 * Dev only (`window.checkGrid()` in the console of `npm run tauri dev`):
 * measures every piece of text showing, and every field and button, and lists
 * the ones that aren't on a cell boundary, outlining them. Run it on each
 * panel after changing the stylesheet. Monaco's own text is not measured.
 */
export function checkGrid(root: HTMLElement = document.body): GridReport {
  root.querySelectorAll(`.${MARK}`).forEach((el) => el.classList.remove(MARK));
  const offBy = (v: number, cell: number) => {
    const r = ((v % cell) + cell) % cell;
    return Math.min(r, cell - r) > 0.05;
  };
  const showing = (el: Element | null) => {
    for (let e = el; e && e !== document.body; e = e.parentElement) {
      const s = getComputedStyle(e);
      if (s.display === "none" || s.visibility === "hidden" || e.classList.contains("monaco-editor") || e.classList.contains("sr-only")) return false;
    }
    return true;
  };
  const report: GridReport = { checked: 0, off: [] };
  const flag = (el: Element, what: string, x: number, y: number) => {
    el.classList.add(MARK);
    report.off.push(`"${what}" at ${(x / CELL_WIDTH + 1).toFixed(2)}, ${(y / CELL_HEIGHT + 1).toFixed(2)}`);
  };
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node as Text;
    const first = text.data.search(/\S/);
    if (first < 0 || !text.parentElement || !showing(text.parentElement)) continue;
    const range = document.createRange();
    range.setStart(text, first);
    range.setEnd(text, text.data.length);
    for (const rect of range.getClientRects()) {
      if (rect.width === 0) continue;
      report.checked++;
      // A width that isn't whole cells means a character came from another font.
      if (offBy(rect.left, CELL_WIDTH) || offBy(rect.top, CELL_HEIGHT) || offBy(rect.width, CELL_WIDTH)) {
        flag(text.parentElement, text.data.trim().slice(0, 24), rect.left, rect.top);
        break;
      }
    }
  }
  for (const el of root.querySelectorAll("input, textarea, select, button, .progress")) {
    if (!showing(el)) continue;
    const rect = el.getBoundingClientRect();
    if (rect.width === 0) continue;
    report.checked++;
    if (offBy(rect.left, CELL_WIDTH) || offBy(rect.top, CELL_HEIGHT) || offBy(rect.width, CELL_WIDTH) || offBy(rect.height, CELL_HEIGHT)) {
      flag(el, el.getAttribute("aria-label") ?? el.getAttribute("placeholder") ?? el.tagName.toLowerCase(), rect.left, rect.top);
    }
  }
  return report;
}
