/**
 * The one strip at the top: the ember mark and wordmark on the shared axis,
 * the three plain links, and the window buttons.
 *
 * The window is undecorated, so the strip drags the window and the buttons
 * are routed through Rust; the frontend needs no window permissions.
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
    const onResize = () => { void api.windowControl("state").then(setMaximized).catch(() => {}); };
    onResize();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  async function control(action: "minimize" | "toggle_maximize" | "close") {
    try {
      const isMaximized = await api.windowControl(action);
      if (action === "toggle_maximize") setMaximized(isMaximized);
    } catch {
      // The window is going away; there is nothing useful to say.
    }
  }



  return (
    <>
    <header className="topbar" data-tauri-drag-region>
      <div className="topbar__lead" data-tauri-drag-region>
        <img src={new URL("../assets/ember.png", import.meta.url).href} width="24" height="24" alt="" />
        <span className="topbar__name">Ember</span>
      </div>
      {!IS_TAURI && <span className="preview-note">Design preview - sample data</span>}
      <div className="topbar__spacer" data-tauri-drag-region />
      {update && (
        <button
          type="button"
          className="tb-update"
          title={`Ember ${update.version} is ready to install`}
          onClick={() => onNavigate("settings")}
        >
          Update
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
        {maximized ? <Copy size={13} /> : <Square size={12} />}
      </button>
      <button
        type="button"
        className="tb-btn tb-btn--close"
        aria-label="Close"
        onClick={() => control("close")}
      >
        <X size={15} />
      </button>
    </header>
    <aside className="sidebar">
      <nav className="sidebar__nav" aria-label="Primary">
        {LINKS.map((link) => {
          return <button key={link.id} type="button"
            className={`navlink${route === link.id ? " is-active" : ""}`}
            aria-current={route === link.id ? "page" : undefined}
            onClick={() => onNavigate(link.id)}>
            <InterfaceIcon name={link.id} /><span>{link.label}</span>
          </button>;
        })}
      </nav>
      <div className="sidebar__status">
        <span className={`sidebar__signal${snapshot?.engaged ? " is-awake" : ""}`} />
        <div><span>{snapshot?.engaged ? "Keeping awake" : "Standing by"}</span>
        <small>{snapshot?.watched_agents ?? 0} tools watched</small></div>
      </div>
    </aside>
    </>
  );
}
