/** Editor preferences that are not appearance. Kept in the browser's storage, per person and machine. */

const FORMAT_KEY = "joyride.formatOnSave";

/** Format Lua files with StyLua when they are saved (off by default: saving never rewrites a file unasked). */
export function formatOnSave(): boolean {
  try {
    return localStorage.getItem(FORMAT_KEY) === "1";
  } catch {
    return false;
  }
}

export function setFormatOnSave(on: boolean): void {
  try {
    localStorage.setItem(FORMAT_KEY, on ? "1" : "0");
  } catch {
    // Storage can be unavailable; the choice then lasts only until the window closes.
  }
}
