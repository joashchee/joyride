import { inTauri } from "./backend";
import { snapDelta, type Fit } from "./snap";

export interface Snapper {
  /** Snap now (after the font or its size changes). */
  now: () => void;
  stop: () => void;
}

const nothing: Snapper = { now: () => {}, stop: () => {} };

/**
 * When a resize of the window ends, nudges it by whole shell cells (8 by 16 px) to the nearest size at which the editor's text area is
 * whole characters wide and whole lines tall (`snap.ts`). Does nothing when the window is maximized or full screen (the system chose
 * that size), when no editor is on screen, or when no nearby size lines up. It gives up after two nudges that did not settle, so a
 * layout that moves under it cannot make the window jump about.
 */
export async function installSnap(getFit: () => Fit | null): Promise<Snapper> {
  if (!inTauri) return nothing;
  const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let ours: { w: number; h: number } | null = null;
  let nudges = 0;
  let busy = false;

  const run = async () => {
    if (busy) return;
    busy = true;
    try {
      if ((await win.isMaximized()) || (await win.isFullscreen())) return;
      const fit = getFit();
      if (!fit) return;
      const d = snapDelta(fit, window.devicePixelRatio || 1);
      if (!d.dw && !d.dh) {
        nudges = 0;
        return;
      }
      if (nudges >= 2) return;
      const scale = await win.scaleFactor();
      const inner = await win.innerSize();
      const w = Math.round(inner.width / scale + d.dw);
      const h = Math.round(inner.height / scale + d.dh);
      ours = { w, h };
      nudges++;
      await win.setSize(new LogicalSize(w, h));
    } catch {
      // the window refused (no permission, or it is closing): leave it as it is
    } finally {
      busy = false;
    }
  };

  const off = await win.onResized(async (e) => {
    const scale = await win.scaleFactor();
    const w = e.payload.width / scale;
    const h = e.payload.height / scale;
    // A resize we made ourselves keeps the count; one the person made starts it again.
    if (!(ours && Math.abs(w - ours.w) <= 1 && Math.abs(h - ours.h) <= 1)) nudges = 0;
    clearTimeout(timer);
    timer = setTimeout(() => void run(), 250);
  });
  return {
    now: () => {
      nudges = 0;
      clearTimeout(timer);
      timer = setTimeout(() => void run(), 400);
    },
    stop: () => {
      clearTimeout(timer);
      off();
    },
  };
}
