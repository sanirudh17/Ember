import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { StoreProvider } from "./lib/store";
import "./styles/global.css";

// Before the first paint: resolve "system" to the OS theme and arm the enter
// animations. Both have to happen before React renders anything.
const rootEl = document.documentElement;
if (!rootEl.dataset.theme) {
  rootEl.dataset.theme = window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}
rootEl.classList.add("js");
rootEl.dataset.env = "__TAURI_INTERNALS__" in window ? "tauri" : "browser";

const container = document.getElementById("root");

if (!container) {
  // Nothing can render without the mount point, and a silent blank window
  // would be the worst possible failure mode for a keep-awake tool.
  throw new Error("Ember could not find its #root element");
}

createRoot(container).render(
  <StrictMode>
    <StoreProvider>
      <App />
    </StoreProvider>
  </StrictMode>,
);
