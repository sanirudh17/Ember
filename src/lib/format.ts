/** Small display helpers shared by every page. */

/** `05:09`, `1:00:00`, `1d 03:00:00` — mirrors the Rust `clock`. */
export function clock(totalSeconds: number): string {
  const total = Math.max(0, Math.floor(totalSeconds));
  const s = total % 60;
  const m = Math.floor(total / 60) % 60;
  const h = Math.floor(total / 3600) % 24;
  const d = Math.floor(total / 86400);
  const pad = (n: number) => String(n).padStart(2, "0");
  if (d > 0) return `${d}d ${pad(h)}:${pad(m)}:${pad(s)}`;
  if (total >= 3600) return `${h}:${pad(m)}:${pad(s)}`;
  return `${pad(m)}:${pad(s)}`;
}

/** `15m`, `1h`, `1h 30m`, `8h`. */
export function presetLabel(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m}m`;
  if (m === 0) return `${h}h`;
  return `${h}h ${m}m`;
}

/** `just now`, `4m ago`, `2h ago`, `3d ago`. */
export function relativeTime(unixSeconds: number | null | undefined): string {
  if (!unixSeconds) return "never";
  const diff = Math.max(0, Math.floor(Date.now() / 1000) - unixSeconds);
  if (diff < 5) return "just now";
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

/** `1 agent`, `2 agents`. */
export function plural(count: number, singular: string, pluralForm = `${singular}s`): string {
  return `${count} ${count === 1 ? singular : pluralForm}`;
}

/** `2h 05m 09s ago` style duration for the "working for" column. */
export function duration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return m === 0 ? `${h}h` : `${h}h ${m}m`;
}

/** `Today 14:30` from an absolute epoch second. */
export function absoluteTime(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/** `HH:MM` in the visitor's own zone. */
export function hhmm(hour: number, minute: number): string {
  return `${String(hour).padStart(2, "0")}:${String(minute).padStart(2, "0")}`;
}

export const CHIME_LABELS: Record<string, string> = {
  glass: "Glass",
  droplet: "Droplet",
  bell: "Bell",
  marimba: "Marimba",
  pulse: "Pulse",
  sparkle: "Sparkle",
  low_tone: "Low Tone",
  double_tap: "Double Tap",
  rise: "Rise",
  fall: "Fall",
};

export const CHIMES = Object.keys(CHIME_LABELS) as (keyof typeof CHIME_LABELS)[];
