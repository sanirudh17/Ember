/**
 * Unified window chrome: frameless title bar + quiet Linear-style navigation sidebar.
 */

import { useEffect, useState } from "react";
import { Copy, Flame, Minus, Settings as SettingsIcon, Square, SquareTerminal, X } from "lucide-react";

import { api, IS_TAURI } from "../lib/api";
import { useStore } from "../lib/store";

export type Route = "awake" | "agents" | "settings";

const LINKS: { id: Route; label: string }[] = [
  { id: "awake", label: "Awake" },
  { id: "agents", label: "Agents" },
  { id: "settings", label: "Settings" },
];

function NavIcon({ id, active }: { id: Route; active: boolean }) {
  const props = { size: 16, strokeWidth: 1.75, className: "navlink__icon", "aria-hidden": true as const };
  if (id === "awake") {
    return (
      <Flame
        {...props}
        fill={active ? "currentColor" : "none"}
        fillOpacity={active ? 0.18 : 0}
      />
    );
  }
  if (id === "agents") {
    return (
      <SquareTerminal
        {...props}
        fill={active ? "currentColor" : "none"}
        fillOpacity={active ? 0.14 : 0}
      />
    );
  }
  return <SettingsIcon {...props} />;
}

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
            width="20"
            height="20"
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
            <span className="control-label">Update v{update.version}</span>
          </button>
        )}
        <button
          type="button"
          className="tb-btn"
          aria-label="Minimize"
          onClick={() => control("minimize")}
        >
          <Minus size={14} strokeWidth={1.75} />
        </button>
        <button
          type="button"
          className="tb-btn"
          aria-label={maximized ? "Restore" : "Maximize"}
          onClick={() => control("toggle_maximize")}
        >
          {maximized ? <Copy size={12.5} strokeWidth={1.75} /> : <Square size={11.5} strokeWidth={1.75} />}
        </button>
        <button
          type="button"
          className="tb-btn tb-btn--close"
          aria-label="Close"
          onClick={() => control("close")}
        >
          <X size={14.5} strokeWidth={1.75} />
        </button>
      </header>

      <aside className="sidebar">
        <nav className="sidebar__nav" aria-label="Primary">
          {LINKS.map((link) => {
            const active = route === link.id;
            return (
              <button
                key={link.id}
                type="button"
                className={`navlink${active ? " is-active" : ""}`}
                aria-current={active ? "page" : undefined}
                onClick={() => onNavigate(link.id)}
              >
                <NavIcon id={link.id} active={active} />
                <span className="control-label">{link.label}</span>
              </button>
            );
          })}
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
