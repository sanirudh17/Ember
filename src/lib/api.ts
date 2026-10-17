/**
 * The single bridge between React and Rust.
 *
 * Inside Tauri every call is a plain `invoke`. In a plain browser (design
 * review, `npm run dev`) the same call goes to the in-memory mock so no view
 * ever has to special-case its data source.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { createMockBackend } from "./mock";
import type {
  AgentId,
  ChangePreview,
  InstallResult,
  Settings,
  Snapshot,
  UntilTarget,
  UpdateInfo,
  UpdateProgress,
} from "../types";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const IS_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const mock = IS_TAURI ? null : createMockBackend();

function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (mock) return mock.invoke(command, args ?? {}) as Promise<T>;
  return invoke<T>(command, args);
}

/** Subscribe to the engine's `state-changed` snapshots. Returns an unsubscribe. */
export async function onStateChanged(handler: (snapshot: Snapshot) => void): Promise<() => void> {
  if (mock) return mock.subscribe(handler);
  return listen<Snapshot>("state-changed", (event) => handler(event.payload));
}

/** Fired when the user closes the window while Ember is still holding the PC awake. */
export async function onCloseRequested(handler: () => void): Promise<() => void> {
  if (mock) return mock.onCloseRequested(handler);
  return listen("close-requested", () => handler());
}

/** Subscribe to update download progress. Returns an unsubscribe. */
export async function onUpdateProgress(
  handler: (progress: UpdateProgress) => void,
): Promise<() => void> {
  if (mock) return mock.onUpdateProgress(handler);
  return listen<UpdateProgress>("update-progress", (event) => handler(event.payload));
}

export const api = {
  snapshot: () => call<Snapshot>("get_snapshot"),

  updateSettings: (settings: Settings) => call<Settings>("update_settings", { settings }),
  setWatching: (enabled: boolean) => call<Settings>("set_watching", { enabled }),

  startManual: (seconds: number) => call<void>("start_manual", { seconds }),
  startManualUntil: (hour: number, minute: number) =>
    call<UntilTarget>("start_manual_until", { hour, minute }),
  startManualUnlimited: () => call<void>("start_manual_unlimited"),
  stopManual: () => call<void>("stop_manual"),
  extendManual: (seconds: number) => call<void>("extend_manual", { seconds }),
  setPaused: (paused: boolean) => call<void>("set_paused", { paused }),
  pauseFor: (seconds: number) => call<void>("pause_for", { seconds }),

  releaseLidNow: () => call<void>("release_lid_now"),
  clearLidCap: () => call<void>("clear_lid_cap"),

  previewInstall: (agent: AgentId, op: "install" | "remove") =>
    call<ChangePreview>("preview_install", { agent, op }),
  applyInstall: (agent: AgentId, op: "install" | "remove") =>
    call<InstallResult>("apply_install", { agent, op }),
  refreshAgents: () => call<void>("refresh_agents"),

  hideWindow: () => call<void>("hide_window"),
  showWindow: () => call<void>("show_window"),
  focusWindow: () => call<void>("focus_window"),
  resetWindow: () => call<void>("reset_window"),
  quitApp: () => call<void>("quit_app"),
  markClosing: () => call<void>("mark_closing"),
  windowControl: (action: "minimize" | "toggle_maximize" | "close") =>
    call<boolean>("window_control", { action }),
  windowResize: (direction: "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw") =>
    call<void>("window_resize", { direction }),
  engineIsArmed: () => call<boolean>("engine_is_armed"),
  getRecentLog: () => call<string>("get_recent_log"),

  appVersion: () => call<string>("app_version"),
  checkUpdate: () => call<UpdateInfo | null>("check_update"),
  installUpdate: () => call<void>("install_update"),
};
