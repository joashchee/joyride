import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./ansiapps-theme.css";
import { applyAppearance, loadPalette, loadSwatch } from "./lib/theme";
import { checkGrid, installRowSnap } from "./lib/grid";

// The appearance is also applied by index.html before first paint; this keeps
// the attributes and the stored choice in step with it.
applyAppearance(loadPalette(), loadSwatch());
installRowSnap();
// Dev only: window.checkGrid() lists what is off the 8 by 16 grid and outlines it.
if (import.meta.env.DEV) (window as unknown as { checkGrid: typeof checkGrid }).checkGrid = checkGrid;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
