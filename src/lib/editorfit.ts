import type { Fit } from "./snap";

/** The editor lives in a lazily loaded chunk (fast start); it registers how to measure itself here when it loads, so the window
 *  snapper can ask without importing Monaco. */
let provider: (() => Fit | null) | null = null;
export const setFitProvider = (p: (() => Fit | null) | null) => void (provider = p);
export const currentFit = (): Fit | null => provider?.() ?? null;
