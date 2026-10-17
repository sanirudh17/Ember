/**
 * A complete in-memory stand-in for the Rust backend.
 *
 * Ember's UI must survive an empty machine (nothing detected, nothing
 * running), a failing bridge, and a busy machine. This mock lets all of them
 * be exercised in a plain browser — `npm run dev` then `?state=empty`,
 * `?state=idle`, `?state=manual`, `?state=blocked`, `?state=off` or
 * `?state=error`. Inside Tauri it is never used: `api.ts` prefers `invoke`.
 */

import type {
  AgentId,
  AgentSnapshot,
  ChangePreview,
  DiffLine,
  InstallResult,
  ManualSnapshot,
  SessionSnapshot,
  Settings,
  Snapshot,
  UntilTarget,
  UpdateProgress,
} from "../types";

type DemoState = "default" | "idle" | "manual" | "blocked" | "off" | "empty" | "error";
type Handler = (snapshot: Snapshot) => void;

const VERSION_LOG = 8;

// ---------------------------------------------------------------------------
// Static definitions
// ---------------------------------------------------------------------------

interface AgentDef {
  id: AgentId;
  name: string;
  monogram: string;
  hue: number;
  mode: "hooks" | "process";
  supported: boolean;
  note: string;
  file: string | null;
}

const AGENT_DEFS: AgentDef[] = [
  {
    id: "claude-code",
    name: "Claude Code",
    monogram: "CC",
    hue: 18,
    mode: "hooks",
    supported: true,
    note: "Hooks in ~/.claude/settings.json (SessionStart, UserPromptSubmit, PreToolUse, Stop).",
    file: "~/.claude/settings.json",
  },
  {
    id: "codex",
    name: "Codex",
    monogram: "CX",
    hue: 210,
    mode: "hooks",
    supported: true,
    note: "Hooks can be added to ~/.codex/config.toml; until then it is watched by process.",
    file: "~/.codex/config.toml",
  },
  {
    id: "opencode",
    name: "OpenCode",
    monogram: "OC",
    hue: 264,
    mode: "hooks",
    supported: true,
    note: "A small plugin in ~/.config/opencode/plugins/ember.js.",
    file: "~/.config/opencode/plugins/ember.js",
  },
  {
    id: "pi",
    name: "pi",
    monogram: "PI",
    hue: 140,
    mode: "process",
    supported: false,
    note: "Watched by process activity.",
    file: null,
  },
  {
    id: "oh-my-pi",
    name: "Oh My Pi",
    monogram: "OP",
    hue: 32,
    mode: "process",
    supported: false,
    note: "Watched by process activity.",
    file: null,
  },
  {
    id: "antigravity",
    name: "Antigravity",
    monogram: "AG",
    hue: 200,
    mode: "process",
    supported: false,
    note: "Watched by process activity.",
    file: null,
  },
  {
    id: "cline",
    name: "Cline",
    monogram: "CL",
    hue: 160,
    mode: "process",
    supported: false,
    note: "Watched inside VS Code by process activity.",
    file: null,
  },
];

// ---------------------------------------------------------------------------
// Small builders
// ---------------------------------------------------------------------------

function clock(total: number): string {
  const t = Math.max(0, Math.floor(total));
  const s = t % 60;
  const m = Math.floor(t / 60) % 60;
  const h = Math.floor(t / 3600);
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

const NO_MANUAL: ManualSnapshot = {
  active: false,
  mode: "timer",
  ends_unix: null,
  remaining_secs: 0,
  total_secs: 0,
  elapsed_secs: 0,
  countdown: "",
  unlimited: false,
  mode_label: "",
};

function session(
  agent: AgentId,
  name: string,
  monogram: string,
  hue: number,
  folder: string,
  state: "working" | "idle",
  workingFor: number,
): SessionSnapshot {
  return {
    agent,
    agent_name: name,
    monogram,
    hue,
    session_id: `${agent}-${folder}`,
    folder,
    state,
    working_for_secs: workingFor,
    last_event_unix: Math.floor(Date.now() / 1000) - 40,
    approximate: agent !== "claude-code" && agent !== "opencode",
    stale: false,
    pid: 12000 + Math.floor(Math.random() * 1000),
  };
}

// ---------------------------------------------------------------------------
// The world
// ---------------------------------------------------------------------------

interface World {
  state: DemoState;
  settings: Settings;
  agents: AgentSnapshot[];
  sessions: SessionSnapshot[];
  manual: ManualSnapshot;
  battery: { present: boolean; percent: number | null; plugged_in: boolean; saver_on: boolean };
  lid: { present: boolean; held: boolean; held_secs: number; closed: boolean | null };
  engaged_secs: number;
  maximized: boolean;
}

function defaultSettings(overrides: Partial<Settings> = {}): Settings {
  return {
    theme: "system",
    launch_at_login: true,
    watching: true,
    lid_proof: true,
    battery_floor_percent: 15,
    require_charger_for_lid: false,
    keep_display_on: false,
    notifications: { mute: false },
    window: { x: null, y: null, width: 1120, height: 720, maximized: false, placed: false },
    ...overrides,
  };
}

function buildAgents(
  detected: AgentId[],
  installed: AgentId[],
  workingCounts: Map<AgentId, number>,
  sessionCounts: Map<AgentId, number>,
  watching: boolean,
): AgentSnapshot[] {
  return AGENT_DEFS.map((def) => {
    const isDetected = detected.includes(def.id);
    const isInstalled = installed.includes(def.id);
    return {
      id: def.id,
      name: def.name,
      monogram: def.monogram,
      hue: def.hue,
      detected: isDetected,
      detect_detail: isDetected ? def.note : "Not found on this machine.",
      installed: isInstalled,
      needs_repair: false,
      supported: def.supported,
      install_file: def.file,
      install_detail: isInstalled ? "Hooks installed." : def.supported ? "Hooks available." : "Process watch only.",
      mode: def.mode,
      note: def.note,
      watching: watching && isDetected,
      working: workingCounts.get(def.id) ?? 0,
      sessions: sessionCounts.get(def.id) ?? 0,
      last_event_unix: isDetected ? Math.floor(Date.now() / 1000) - 60 : null,
    };
  });
}

function buildWorld(state: DemoState): World {
  const base: World = {
    state,
    settings: defaultSettings({ lid_proof: state !== "off" }),
    agents: [],
    sessions: [],
    manual: { ...NO_MANUAL },
    battery: { present: true, percent: 87, plugged_in: true, saver_on: false },
    lid: { present: true, held: false, held_secs: 0, closed: null },
    engaged_secs: 0,
    maximized: false,
  };

  const claude = session("claude-code", "Claude Code", "CC", 18, "windows-tool", "working", 1134);
  const codex = session("codex", "Codex", "CX", 210, "api-gateway", "working", 402);
  const opencodeIdle = session("opencode", "OpenCode", "OC", 264, "dotfiles", "idle", 0);

  const counts = (sessions: SessionSnapshot[]) => {
    const working = new Map<AgentId, number>();
    const total = new Map<AgentId, number>();
    for (const s of sessions) {
      const id = s.agent as AgentId;
      total.set(id, (total.get(id) ?? 0) + 1);
      if (s.state === "working") working.set(id, (working.get(id) ?? 0) + 1);
    }
    return { working, total };
  };

  switch (state) {
    case "empty": {
      base.battery = { present: true, percent: 41, plugged_in: false, saver_on: false };
      base.agents = buildAgents([], [], new Map(), new Map(), true);
      break;
    }
    case "idle": {
      base.battery = { present: true, percent: 100, plugged_in: true, saver_on: false };
      base.agents = buildAgents(
        ["claude-code", "codex", "opencode", "pi"],
        ["claude-code", "opencode"],
        new Map(),
        new Map(),
        true,
      );
      break;
    }
    case "manual": {
      base.battery = { present: true, percent: 62, plugged_in: false, saver_on: false };
      base.sessions = [opencodeIdle];
      const { working, total } = counts(base.sessions);
      base.agents = buildAgents(
        ["claude-code", "codex", "opencode"],
        ["claude-code", "opencode"],
        working,
        total,
        true,
      );
      base.manual = {
        active: true,
        mode: "timer",
        ends_unix: Math.floor(Date.now() / 1000) + 2388,
        remaining_secs: 2388,
        total_secs: 3600,
        elapsed_secs: 1212,
        countdown: clock(2388),
        unlimited: false,
        mode_label: "1h",
      };
      base.lid.held = true;
      base.lid.held_secs = 1212;
      base.engaged_secs = 1212;
      break;
    }
    case "blocked": {
      base.battery = { present: true, percent: 12, plugged_in: false, saver_on: false };
      base.sessions = [claude, codex];
      const { working, total } = counts(base.sessions);
      base.agents = buildAgents(
        ["claude-code", "codex", "opencode"],
        ["claude-code", "opencode"],
        working,
        total,
        true,
      );
      break;
    }
    case "off": {
      base.settings = defaultSettings({ watching: false, lid_proof: false });
      base.sessions = [claude, codex];
      const { working, total } = counts(base.sessions);
      base.agents = buildAgents(
        ["claude-code", "codex", "opencode"],
        ["claude-code", "opencode"],
        working,
        total,
        false,
      );
      break;
    }
    case "default":
    case "error":
    default: {
      base.sessions = [claude, codex, opencodeIdle];
      const { working, total } = counts(base.sessions);
      base.agents = buildAgents(
        ["claude-code", "codex", "opencode", "pi"],
        ["claude-code", "opencode"],
        working,
        total,
        true,
      );
      base.lid.held = true;
      base.lid.held_secs = 1840;
      base.engaged_secs = 1840;
      break;
    }
  }

  return base;
}

// ---------------------------------------------------------------------------
// Snapshot emission
// ---------------------------------------------------------------------------

function taskbarTitle(engaged: boolean, paused: boolean, working: number): string {
  if (paused) return "Ember·Paused";
  if (!engaged) return "Ember·Idle";
  if (working === 0) return "Ember·Awake";
  return working === 1 ? "Ember·Awake·1 agent working" : `Ember·Awake·${working} agents working`;
}

function buildSnapshot(world: World): Snapshot {
  const manual = world.manual;
  const working = world.sessions.filter((s) => s.state === "working").length;
  const watched = world.agents.filter((a) => a.detected).length;

  let blocker: Snapshot["blocker"] = null;
  let blocker_label: string | null = null;
  let blocker_detail: string | null = null;
  if (world.settings.watching) {
    if (world.battery.present && (world.battery.percent ?? 100) < world.settings.battery_floor_percent) {
      blocker = "battery_low";
      blocker_label = "Battery too low";
      blocker_detail = `Battery is at ${world.battery.percent}%; Ember released the hold to protect your session.`;
    } else if (world.battery.saver_on) {
      blocker = "battery_saver";
      blocker_label = "Battery saver is on";
      blocker_detail = "Windows asked for power saving, so Ember let go until it is off again.";
    }
  }

  const want = manual.active || (world.settings.watching && working > 0);
  const engaged = want && blocker === null;

  const reason = engaged
    ? manual.active
      ? "Manual hold"
      : `${working} agent${working === 1 ? "" : "s"} working`
    : blocker_label ?? "Nothing to hold";

  const heldMinutes = Math.floor(world.lid.held_secs / 60);
  const lidNote = !world.lid.present
    ? "No lid switch on this machine."
    : world.lid.held
      ? `The lid-close action is overridden — ${heldMinutes}m of 6h used.`
      : world.settings.lid_proof
        ? "Armed: the lid-close action will be overridden while Ember is awake."
        : "Off: closing the lid follows your Windows power plan.";

  return {
    unix_ms: Date.now(),
    engaged,
    engaged_for_secs: engaged ? world.engaged_secs : 0,
    want_awake: want,
    paused: false,
    paused_remaining_secs: null,
    paused_countdown: null,
    watching: world.settings.watching,
    agents_working: working,
    watched_agents: watched,
    blocker,
    blocker_label,
    blocker_detail,
    reason,
    status_word: engaged ? "Awake" : blocker ? "Blocked" : "Idle",
    title: taskbarTitle(engaged, false, working),
    manual,
    battery: {
      present: world.battery.present,
      percent: world.battery.percent,
      plugged_in: world.battery.plugged_in,
      saver_on: world.battery.saver_on,
      rule: null,
      percent_label: world.battery.percent === null ? "" : `${world.battery.percent}%`,
    },
    lid: {
      present: world.lid.present,
      proof_enabled: world.settings.lid_proof,
      held: world.lid.held,
      modern_standby: false,
      cap_hours: 6,
      held_secs: world.lid.held_secs,
      cap_remaining_secs: 6 * 3600 - world.lid.held_secs,
      closed: world.lid.closed,
      note: lidNote,
    },
    display: {
      policy_label: world.settings.keep_display_on
        ? "The screen stays on"
        : "Windows manages the display",
      display_kept_on: world.settings.keep_display_on,
      turned_off: false,
      note: world.settings.keep_display_on
        ? "Ember also keeps the display awake while it holds the PC."
        : "Windows may turn the screen off while Ember keeps the PC awake.",
    },
    capabilities: {
      lid_present: world.lid.present,
      modern_standby: false,
      mica_available: true,
      os_build: 22631,
      os_name: "Windows 11",
      blocking: "System and display requests are available.",
      power_requests: true,
      lid_proof_available: world.lid.present,
    },
    sessions: world.sessions.map((s) => ({ ...s })),
    agents: world.agents.map((a) => ({ ...a })),
    settings: {
      ...world.settings,
      notifications: { ...world.settings.notifications },
      window: { ...world.settings.window },
    },
  };
}

// ---------------------------------------------------------------------------
// Install previews
// ---------------------------------------------------------------------------

function diffFor(op: "install" | "remove"): DiffLine[] {
  const lines: DiffLine[] =
    op === "install"
      ? [
          { kind: "context", text: '  "hooks": {' },
          {
            kind: "add",
            text: '+    "SessionStart": [{ "hooks": [{ "type": "command", "command": "ember-hook --event session-start" }] }],',
          },
          {
            kind: "add",
            text: '+    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "ember-hook --event prompt" }] }],',
          },
          {
            kind: "add",
            text: '+    "PreToolUse": [{ "matcher": "*", "hooks": [{ "type": "command", "command": "ember-hook --event tool" }] }],',
          },
          {
            kind: "add",
            text: '+    "Stop": [{ "hooks": [{ "type": "command", "command": "ember-hook --event stop" }] }]',
          },
          { kind: "context", text: "  }," },
        ]
      : [
          { kind: "context", text: '  "hooks": {' },
          {
            kind: "remove",
            text: '-    "SessionStart": [{ "hooks": [{ "type": "command", "command": "ember-hook --event session-start" }] }],',
          },
          {
            kind: "remove",
            text: '-    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "ember-hook --event prompt" }] }],',
          },
          {
            kind: "remove",
            text: '-    "PreToolUse": [{ "matcher": "*", "hooks": [{ "type": "command", "command": "ember-hook --event tool" }] }],',
          },
          {
            kind: "remove",
            text: '-    "Stop": [{ "hooks": [{ "type": "command", "command": "ember-hook --event stop" }] }]',
          },
          { kind: "context", text: "  }," },
        ];
  return lines;
}

function previewFor(def: AgentDef, op: "install" | "remove", installed: boolean): ChangePreview {
  const install = op === "install";
  return {
    agent: def.id,
    name: def.name,
    mode: def.mode,
    supported: def.supported,
    file: def.file,
    format: def.id === "codex" ? "toml" : "json",
    creates_file: false,
    summary: install
      ? "Adds four hook entries that tell Ember when a session starts, when you prompt, when tools run, and when it stops."
      : "Removes Ember's hook entries and leaves the rest of the file untouched.",
    note: "A backup of the current file is kept next to it.",
    backup: null,
    diff: diffFor(op),
    installed,
    needs_repair: false,
    can_install: !installed && install,
    can_remove: installed && !install,
    can_repair: false,
  };
}

// ---------------------------------------------------------------------------
// The backend
// ---------------------------------------------------------------------------

export function createMockBackend() {
  const params = new URLSearchParams(window.location.search);
  const state = (params.get("state") ?? "default") as DemoState;
  const world = buildWorld(state);
  // The review browser may not follow the OS scheme; `?theme=` forces one.
  const theme = params.get("theme");
  if (theme === "light" || theme === "dark") world.settings.theme = theme;
  const handlers = new Set<Handler>();

  const notify = () => {
    const snap = buildSnapshot(world);
    handlers.forEach((handler) => handler(snap));
  };

  const ticker = window.setInterval(() => {
    if (world.manual.active && !world.manual.unlimited) {
      world.manual.remaining_secs = Math.max(0, world.manual.remaining_secs - 1);
      world.manual.elapsed_secs += 1;
      world.manual.countdown = clock(world.manual.remaining_secs);
      if (world.manual.remaining_secs === 0) world.manual = { ...NO_MANUAL };
    }
    if (world.lid.held) world.lid.held_secs += 1;
    for (const s of world.sessions) {
      if (s.state === "working") s.working_for_secs += 1;
    }
    const want =
      world.manual.active ||
      (world.settings.watching && world.sessions.some((s) => s.state === "working"));
    const clear =
      !(world.battery.present && (world.battery.percent ?? 100) < world.settings.battery_floor_percent) &&
      !world.battery.saver_on;
    world.engaged_secs = want && clear ? world.engaged_secs + 1 : 0;
    notify();
  }, 1000);

  // The ticker is a design harness; never let it hold a page open.
  void ticker;

  function requireDef(agent: string): AgentDef {
    const def = AGENT_DEFS.find((d) => d.id === agent);
    if (!def) throw new Error(`mock: unknown agent ${agent}`);
    return def;
  }

  function recomputeAgents() {
    const working = new Map<AgentId, number>();
    const total = new Map<AgentId, number>();
    for (const s of world.sessions) {
      const id = s.agent as AgentId;
      total.set(id, (total.get(id) ?? 0) + 1);
      if (s.state === "working") working.set(id, (working.get(id) ?? 0) + 1);
    }
    const detected = world.agents.filter((a) => a.detected).map((a) => a.id as AgentId);
    const installed = world.agents.filter((a) => a.installed).map((a) => a.id as AgentId);
    world.agents = buildAgents(detected, installed, working, total, world.settings.watching);
  }

  function recentLog(): string {
    const now = Date.now();
    const lines = [
      `[${new Date(now - 320000).toISOString().replace("T", " ").slice(0, 19)}] ENGINE Ember started`,
      `[${new Date(now - 280000).toISOString().replace("T", " ").slice(0, 19)}] AGENTS detected 4 tools (claude-code, codex, opencode, pi)`,
      `[${new Date(now - 240000).toISOString().replace("T", " ").slice(0, 19)}] ENGINE powers requests armed, display untouched`,
      `[${new Date(now - 180000).toISOString().replace("T", " ").slice(0, 19)}] SESSION claude-code · windows-tool — working`,
      `[${new Date(now - 90000).toISOString().replace("T", " ").slice(0, 19)}] SESSION codex · api-gateway — working`,
      `[${new Date(now - 60000).toISOString().replace("T", " ").slice(0, 19)}] LID the lid-close action is overridden`,
      `[${new Date(now - 30000).toISOString().replace("T", " ").slice(0, 19)}] POWER SYSTEM awake requested`,
      `[${new Date(now - 8000).toISOString().replace("T", " ").slice(0, 19)}] ENGINE state-changed (engaged, 2 agents)`,
    ];
    return lines.slice(-VERSION_LOG).join("\n");
  }

  async function invoke(command: string, args: Record<string, unknown>): Promise<unknown> {
    switch (command) {
      case "get_snapshot": {
        await new Promise((resolve) => setTimeout(resolve, 300));
        if (world.state === "error") throw new Error("mock: the engine is unreachable (?state=error)");
        return buildSnapshot(world);
      }

      case "update_settings": {
        const next = args.settings as Settings;
        world.settings = {
          ...next,
          notifications: { ...next.notifications },
          window: { ...next.window },
        };
        recomputeAgents();
        notify();
        return world.settings;
      }

      case "set_watching": {
        const enabled = Boolean(args.enabled);
        world.settings = { ...world.settings, watching: enabled };
        recomputeAgents();
        notify();
        return world.settings;
      }

      case "start_manual": {
        const seconds = Number(args.seconds);
        world.manual = {
          active: true,
          mode: "timer",
          ends_unix: Math.floor(Date.now() / 1000) + seconds,
          remaining_secs: seconds,
          total_secs: seconds,
          elapsed_secs: 0,
          countdown: clock(seconds),
          unlimited: false,
          mode_label: seconds >= 3600 ? `${Math.round(seconds / 3600)}h` : `${Math.round(seconds / 60)}m`,
        };
        notify();
        return undefined;
      }

      case "start_manual_until": {
        const hour = Number(args.hour);
        const minute = Number(args.minute);
        const now = new Date();
        const target = new Date(now);
        target.setHours(hour, minute, 0, 0);
        if (target <= now) target.setDate(target.getDate() + 1);
        const seconds = Math.floor((target.getTime() - now.getTime()) / 1000);
        const label = `Until ${String(hour).padStart(2, "0")}:${String(minute).padStart(2, "0")}`;
        world.manual = {
          active: true,
          mode: "until",
          ends_unix: Math.floor(target.getTime() / 1000),
          remaining_secs: seconds,
          total_secs: seconds,
          elapsed_secs: 0,
          countdown: clock(seconds),
          unlimited: false,
          mode_label: label,
        };
        notify();
        const until: UntilTarget = {
          seconds,
          tomorrow: target.getDate() !== now.getDate(),
          label,
        };
        return until;
      }

      case "start_manual_unlimited": {
        world.manual = {
          active: true,
          mode: "unlimited",
          ends_unix: null,
          remaining_secs: 0,
          total_secs: 0,
          elapsed_secs: 0,
          countdown: "∞",
          unlimited: true,
          mode_label: "No limit",
        };
        notify();
        return undefined;
      }

      case "stop_manual": {
        world.manual = { ...NO_MANUAL };
        notify();
        return undefined;
      }

      case "extend_manual": {
        if (world.manual.active) {
          world.manual.remaining_secs += Number(args.seconds);
          world.manual.total_secs += Number(args.seconds);
          world.manual.countdown = clock(world.manual.remaining_secs);
          notify();
        }
        return undefined;
      }

      case "release_lid_now":
      case "clear_lid_cap":
      case "refresh_agents":
      case "mark_closing":
      case "hide_window":
      case "show_window":
      case "focus_window":
      case "reset_window":
      case "quit_app":
      case "window_resize":
        return undefined;

      case "window_control": {
        if (args.action === "toggle_maximize") {
          world.maximized = !world.maximized;
          return world.maximized;
        }
        return world.maximized;
      }

      case "preview_install": {
        const def = requireDef(String(args.agent));
        const op = args.op === "remove" ? "remove" : "install";
        const current = world.agents.find((a) => a.id === def.id);
        return previewFor(def, op, current?.installed ?? false);
      }

      case "apply_install": {
        const def = requireDef(String(args.agent));
        const op = args.op === "remove" ? "remove" : "install";
        const install = op === "install";
        world.agents = world.agents.map((a) =>
          a.id === def.id
            ? {
                ...a,
                installed: install,
                needs_repair: false,
                install_detail: install ? "Hooks installed." : "Hooks available.",
              }
            : a,
        );
        notify();
        const result: InstallResult = {
          ok: true,
          message: install ? `Hooks installed into ${def.file}.` : `Hooks removed from ${def.file}.`,
          file: def.file,
          backup: null,
          diff: diffFor(op),
          installed: install,
          needs_repair: false,
        };
        return result;
      }

      case "engine_is_armed":
        return true;

      case "get_recent_log":
        return recentLog();

      case "app_version":
        return "1.0.0";

      case "check_update": {
        // `?update=1` makes a fresh release appear so the row can be reviewed.
        const params = new URLSearchParams(window.location.search);
        if (params.get("update") === "1") {
          return {
            version: "1.1.0",
            current_version: "1.0.0",
            notes: "A quieter watchdog and a faster hook path.",
          };
        }
        return null;
      }

      case "install_update":
        throw new Error("mock: updates only install in the real app");

      default:
        throw new Error(`mock: unknown command ${command}`);
    }
  }

  return {
    invoke,
    subscribe(handler: Handler): () => void {
      handlers.add(handler);
      return () => handlers.delete(handler);
    },
    onCloseRequested(_handler: () => void): () => void {
      // A browser tab never asks Ember to close.
      return () => {};
    },
    onUpdateProgress(_handler: (progress: UpdateProgress) => void): () => void {
      // Nothing downloads inside a browser tab.
      return () => {};
    },
  };
}
