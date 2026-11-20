/**
 * The shell: top bar, one scrolled content area, frameless resize strips,
 * and the close dialog.
 *
 * Routing is three local screens kept in the URL query so a browser reload
 * (or the inspector) lands on the same screen.
 */

import { useEffect, useRef, useState } from "react";

import { CloseDialog } from "./components/CloseDialog";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { PageError, PageSkeleton } from "./components/Page";
import { TopBar, type Route } from "./components/TopBar";
import { api, IS_TAURI } from "./lib/api";
import { AppearanceProvider } from "./lib/appearance";
import { useStore } from "./lib/store";
import { AgentsPage } from "./pages/Agents";
import { HomePage } from "./pages/Home";
import { SettingsPage } from "./pages/Settings";

const ROUTES: Route[] = ["awake", "agents", "settings"];
const DIRECTIONS = ["n", "s", "e", "w", "ne", "nw", "se", "sw"] as const;

function routeFromLocation(): Route {
  const query = new URLSearchParams(window.location.search).get("route");

  return (ROUTES as string[]).includes(query ?? "") ? (query as Route) : "awake";
}

export function App() {
  const { snapshot, settings, loading, error, reload } = useStore();
  const [route, setRoute] = useState<Route>(routeFromLocation);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const ready = Boolean(snapshot && settings);
  const revealed = useRef(false);

  useEffect(() => {
    if (!IS_TAURI || revealed.current || (!ready && loading)) return;
    // Two frames let the theme and the initial surface reach the compositor.
    let second = 0;
    const first = requestAnimationFrame(() => {
      second = requestAnimationFrame(() => {
        void api.showWindow().then(() => { revealed.current = true; });
      });
    });
    return () => { cancelAnimationFrame(first); cancelAnimationFrame(second); };
  }, [ready, loading]);

  // Keep the URL in step so a reload lands on the same screen.
  useEffect(() => {
    const url = new URL(window.location.href);
    if (route === "awake") url.searchParams.delete("route");
    else url.searchParams.set("route", route);
    window.history.replaceState(null, "", url);
  }, [route]);

  // Each screen starts at the top when switching routes.
  useEffect(() => {
    contentRef.current?.scrollTo({ top: 0 });
  }, [route]);

  return (
    <AppearanceProvider>
      <div className="app">
        <TopBar route={route} onNavigate={setRoute} />
        <div className="content" ref={contentRef}>
          {!ready ? (
            loading ? (
              <PageSkeleton />
            ) : (
              <PageError message={error ?? "The engine did not answer."} onRetry={reload} />
            )
          ) : (
            <ErrorBoundary key={route}>
              {route === "awake" && <HomePage onViewAgents={() => setRoute("agents")} />}
              {route === "agents" && <AgentsPage />}
              {route === "settings" && <SettingsPage />}
            </ErrorBoundary>
          )}
        </div>
        {IS_TAURI && (
          <div className="resize-frame" aria-hidden="true">
            {DIRECTIONS.map((direction) => (
              <div
                key={direction}
                className={`resize-handle resize-handle--${direction}`}
                onMouseDown={() => void api.windowResize(direction)}
              />
            ))}
          </div>
        )}
      </div>
      <CloseDialog />
    </AppearanceProvider>
  );
}
