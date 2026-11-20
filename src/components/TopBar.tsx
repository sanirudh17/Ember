/**
 * Unified window chrome: frameless title bar + quiet navigation sidebar.
 */

import { useEffect, useState } from "react";
import { Copy, Minus, Square, X } from "lucide-react";

import { InterfaceIcon } from "./InterfaceIcon";
import { api, IS_TAURI } from "../lib/api";
import { useStore } from "../lib/store";

export type Route = "awake" | "agents" | "settings";

const LINKS: { id: Route; label: string }[] = [
  { id: "awake", label: "Awake" },
  { id: "agents", label: "Agents" },
  { id: "settings", label: "Settings" },
];

export function TopBar({
  route,
  onNavigate,
}: {
  route: Route;
  onNavigate: (route: Route) => void;
}) {
  const { update, snapshot } = useStore();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const onResize = () => {
      void api.windowControl("state").then(setMaximized).catch(() => {});
    };
    onResize();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  async function control(action: "minimize" | "toggle_maximize" | "close") {
    try {
      const isMaximized = await api.windowControl(action);
      if (action === "toggle_maximize") setMaximized(isMaximized);
    } catch {
      // Window is closing.
    }
  }

  return (
    <>
      <header className="topbar" data-tauri-drag-region>
        <div className="topbar__lead" data-tauri-drag-region>
          <img
            src={new URL("../assets/ember.png", import.meta.url).href}
            width="22"
            height="22"
            alt=""
          />
          <span className="topbar__name">Ember</span>
        </div>
        {!IS_TAURI && <span className="preview-note">Preview</span>}
        <div className="topbar__spacer" data-tauri-drag-region />
        {update && (
          <button
            type="button"
            className="tb-update"
            title={`Ember ${update.version} is ready to install`}
            onClick={() => onNavigate("settings")}
          >
            Update v{update.version}
          </button>
        )}
        <button
          type="button"
          className="tb-btn"
          aria-label="Minimize"
          onClick={() => control("minimize")}
        >
          <Minus size={14} />
        </button>
        <button
          type="button"
          className="tb-btn"
          aria-label={maximized ? "Restore" : "Maximize"}
          onClick={() => control("toggle_maximize")}
        >
          {maximized ? <Copy size={12.5} /> : <Square size={11.5} />}
        </button>
        <button
          type="button"
          className="tb-btn tb-btn--close"
          aria-label="Close"
          onClick={() => control("close")}
        >
          <X size={14.5} />
        </button>
      </header>

      <aside className="sidebar">
        <nav className="sidebar__nav" aria-label="Primary">
          {LINKS.map((link) => (
            <button
              key={link.id}
              type="button"
              className={`navlink${route === link.id ? " is-active" : ""}`}
              aria-current={route === link.id ? "page" : undefined}
              onClick={() => onNavigate(link.id)}
            >
              <InterfaceIcon name={link.id} size={17} />
              <span>{link.label}</span>
            </button>
          ))}
        </nav>

        <div className="sidebar__status">
          <span className={`sidebar__signal${snapshot?.engaged ? " is-awake" : ""}`} />
          <span className="sidebar__state">
            {snapshot?.engaged ? "Keeping awake" : "Standing by"}
          </span>
        </div>
      </aside>
    </>
  );
}
