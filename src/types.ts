/**
 * TypeScript mirrors of Ember's Rust snapshot types.
 *
 * Every enum here matches the `serde(rename_all = ...)` on the Rust side
 * exactly, so the compiler catches any drift between the two halves.
 *
 * After the simplification the contract is deliberately small: `Settings`
 * has six fields and everything else is fixed policy decided in Rust.
 */

export type Theme = "system" | "light" | "dark";

/** Why Ember is holding back (mirrors `engine::decision::Guard`). */
export type Guard = "battery_low" | "battery_saver" | "lid_cap";

export type ManualMode = "timer" | "until" | "unlimited";
export type SessionState = "working" | "idle";
export type DiffKind = "add" | "remove" | "context";
export type IntegrationMode = "hooks" | "process";

export type AgentId =
  | "claude-code"
  | "codex"
  | "opencode"
  | "pi"
  | "oh-my-pi"
  | "antigravity"
  | "cline";

export interface WindowState {
  x: number | null;
  y: number | null;
  width: number;
  height: number;
  maximized: boolean;
  /** False until the user has moved or resized the window once. */
  placed: boolean;
}

/** Notifications are all-or-nothing now: one mute switch gates the lot. */
export interface Notifications {
  mute: boolean;
}

/** The surviving user options. Everything else is fixed policy. */
export interface Settings {
  theme: Theme;
  launch_at_login: boolean;
  watching: boolean;
  lid_proof: boolean;
  battery_floor_percent: number;
  require_charger_for_lid: boolean;
  keep_display_on: boolean;
  notifications: Notifications;
  window: WindowState;
}

export interface ManualSnapshot {
  active: boolean;
  mode: ManualMode;
  ends_unix: number | null;
  remaining_secs: number;
  total_secs: number;
  elapsed_secs: number;
  countdown: string;
  unlimited: boolean;
  mode_label: string;
}

export interface BatterySnapshot {
  present: boolean;
  percent: number | null;
  plugged_in: boolean;
  saver_on: boolean;
  rule: string | null;
  percent_label: string;
}

export interface LidSnapshot {
  present: boolean;
  proof_enabled: boolean;
  held: boolean;
  modern_standby: boolean;
  cap_hours: number;
  held_secs: number;
  cap_remaining_secs: number;
  closed: boolean | null;
  note: string;
}

/** Kept to mirror the Rust snapshot; the interface no longer shows it. */
export interface DisplaySnapshot {
  policy_label: string;
  display_kept_on: boolean;
  turned_off: boolean;
  note: string;
}

export interface SessionSnapshot {
  agent: string;
  agent_name: string;
  monogram: string;
  hue: number;
  session_id: string;
  folder: string;
  state: SessionState;
  working_for_secs: number;
  last_event_unix: number;
  approximate: boolean;
  stale: boolean;
  pid: number | null;
}

export interface AgentSnapshot {
  id: string;
  name: string;
  monogram: string;
  hue: number;
  detected: boolean;
  detect_detail: string;
  installed: boolean;
  needs_repair: boolean;
  supported: boolean;
  install_file: string | null;
  install_detail: string;
  mode: IntegrationMode;
  note: string;
  watching: boolean;
  working: number;
  sessions: number;
  last_event_unix: number | null;
}

export interface Capabilities {
  lid_present: boolean;
  modern_standby: boolean;
  mica_available: boolean;
  os_build: number;
  os_name: string;
  blocking: string;
  power_requests: boolean;
  lid_proof_available: boolean;
}

/** One snapshot describes everything a screen needs; emitted once a second. */
export interface Snapshot {
  unix_ms: number;
  engaged: boolean;
  engaged_for_secs: number;
  want_awake: boolean;
  paused: boolean;
  paused_remaining_secs: number | null;
  paused_countdown: string | null;
  watching: boolean;
  agents_working: number;
  watched_agents: number;
  blocker: Guard | null;
  blocker_label: string | null;
  blocker_detail: string | null;
  reason: string;
  status_word: string;
  title: string;
  manual: ManualSnapshot;
  battery: BatterySnapshot;
  lid: LidSnapshot;
  display: DisplaySnapshot;
  capabilities: Capabilities;
  sessions: SessionSnapshot[];
  agents: AgentSnapshot[];
  settings: Settings;
}

export interface DiffLine {
  kind: DiffKind;
  text: string;
}

export interface ChangePreview {
  agent: string;
  name: string;
  mode: IntegrationMode;
  supported: boolean;
  file: string | null;
  format: string;
  creates_file: boolean;
  summary: string;
  note: string;
  backup: string | null;
  diff: DiffLine[];
  installed: boolean;
  needs_repair: boolean;
  can_install: boolean;
  can_remove: boolean;
  can_repair: boolean;
}

export interface InstallResult {
  ok: boolean;
  message: string;
  file: string | null;
  backup: string | null;
  diff: DiffLine[];
  installed: boolean;
  needs_repair: boolean;
}

/** What an "Until HH:MM" timer resolved to. */
export interface UntilTarget {
  seconds: number;
  tomorrow: boolean;
  label: string;
}

/** A newer signed release offered by the update checker. */
export interface UpdateInfo {
  version: string;
  current_version: string;
  notes: string | null;
}

/** Download progress for an update, emitted while the installer is fetched. */
export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}
