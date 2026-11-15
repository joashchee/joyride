import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./ansiapps-theme.css";
import { applyAppearance, loadPalette, loadSwatch } from "./lib/theme";
import { reportFrontendError } from "./lib/backend";
import { checkGrid, installRowSnap, installScrollMarker } from "./lib/grid";

// The appearance is also applied by index.html before first paint; this keeps
// the attributes and the stored choice in step with it.
applyAppearance(loadPalette(), loadSwatch());
installRowSnap();
installScrollMarker();
// Errors nobody caught go to the error log, and the first few become crash reports (docs/recovery.md).
window.addEventListener("error", (e) => {
  void reportFrontendError(e.message || "error", e.error instanceof Error ? e.error.stack : undefined, e.filename ? `${e.filename}:${e.lineno}:${e.colno}` : undefined);
});
window.addEventListener("unhandledrejection", (e) => {
  const r = e.reason;
  void reportFrontendError(`unhandled rejection: ${r instanceof Error ? r.message : String(r)}`, r instanceof Error ? r.stack : undefined);
});
// Dev only: window.checkGrid() lists what is off the 8 by 16 grid and outlines it.
if (import.meta.env.DEV) (window as unknown as { checkGrid: typeof checkGrid }).checkGrid = checkGrid;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
