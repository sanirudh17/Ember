/**
 * Settings — minimal, calm preference ledger with subtle Solid Glass controls
 * and hardware-accelerated sliding segmented selectors.
 */

import { useEffect, useState } from "react";

import { Button, Segmented, Switch, type SegmentedOption } from "../components/ui";
import { api } from "../lib/api";
import { useReady, useStore } from "../lib/store";
import type { Theme } from "../types";

const THEMES: SegmentedOption<Theme>[] = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

const FLOOR_STEPS = ["10", "15", "20", "30"];

function floorOptionsFor(percent: number): SegmentedOption<string>[] {
  const options = FLOOR_STEPS.map((value) => ({ value, label: `${value}%` }));
  const value = String(percent);
  if (!options.some((option) => option.value === value)) {
    options.push({ value, label: `${value}%` });
  }
  return options;
}

interface LogLine {
  t: string | null;
  m: string;
}

function parseLog(text: string): LogLine[] {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .slice(-10)
    .map((line) => {
      if (line.startsWith("[")) {
        const end = line.indexOf("]");
        if (end > 0) {
          return { t: line.slice(1, end), m: line.slice(end + 1).trim() };
        }
      }
      return { t: null, m: line };
    });
}

export function SettingsPage() {
  const { snapshot, settings } = useReady();
  const {
    apply,
    update,
    updateChecking,
    updateError,
    updateProgress,
    checkUpdate,
    installUpdate,
  } = useStore();
  const [log, setLog] = useState<LogLine[]>([]);
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    const pull = () => {
      api
        .getRecentLog()
        .then((text) => {
          if (alive) setLog(parseLog(text));
        })
        .catch(() => {});
    };
    pull();
    const timer = setInterval(pull, 5000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    void api
      .appVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  const updateHint = updateError
    ? updateError
    : update
      ? `Version ${update.version} is ready to install. Ember restarts to finish.`
      : updateChecking
        ? "Checking GitHub for a newer release…"
        : "You're on the latest version. Ember checks GitHub automatically.";

  return (
    <div className="page page--settings">
      <header className="pagehead pagehead--stack">
        <h1 className="h1">Settings</h1>
        <p className="pagehead__sub">Preferences for your everyday workflow.</p>
      </header>

      <section className="settings-group">
        <h2 className="h2">General</h2>
        <div className="settings-rows">
          <div className="srow">
            <div className="srow__copy">
              <div className="srow__label">Launch at login</div>
              <div className="srow__hint">Start Ember automatically when you sign in to Windows.</div>
            </div>
            <Switch
              label="Launch at login"
              checked={settings.launch_at_login}
              onChange={(next) => apply({ launch_at_login: next })}
            />
          </div>
          <div className="srow">
            <div className="srow__copy">
              <div className="srow__label">Appearance</div>
              <div className="srow__hint">Follow Windows, or hold the interface light or dark.</div>
            </div>
            <Segmented
              label="Appearance"
              value={settings.theme}
              options={THEMES}
              onChange={(theme: Theme) => apply({ theme })}
            />
          </div>
          <div className="srow">
            <div className="srow__copy">
              <div className="srow__label">Notifications</div>
              <div className="srow__hint">
                Show a toast and chime when Ember takes hold or lets go.
              </div>
            </div>
            <Switch
              label="Notifications"
              checked={!settings.notifications.mute}
              onChange={(next) =>
                apply({ notifications: { ...settings.notifications, mute: !next } })
              }
            />
          </div>
        </div>
      </section>

      <section className="settings-group">
        <h2 className="h2">Power &amp; display</h2>
        <div className="settings-rows">
          <div className="srow">
            <div className="srow__copy">
              <div className="srow__label">Battery floor</div>
              <div className="srow__hint">
                Release holds automatically when unplugged below this level.
              </div>
            </div>
            <Segmented
              label="Battery floor"
              value={String(settings.battery_floor_percent)}
              options={floorOptionsFor(settings.battery_floor_percent)}
              onChange={(value: string) => apply({ battery_floor_percent: Number(value) })}
            />
          </div>
          <div className="srow">
            <div className="srow__copy">
              <div className="srow__label">Require charger for lid hold</div>
              <div className="srow__hint">
                Only override the lid-close action while connected to power.
              </div>
            </div>
            <Switch
              label="Require charger for lid hold"
              checked={settings.require_charger_for_lid}
              onChange={(next) => apply({ require_charger_for_lid: next })}
            />
          </div>
          <div className="srow">
            <div className="srow__copy">
              <div className="srow__label">Keep screen on for agents</div>
              <div className="srow__hint">
                Keep the display lit during agent holds; manual holds always keep it lit.
              </div>
            </div>
            <Switch
              label="Keep screen on for agents"
              checked={settings.keep_display_on}
              onChange={(next) => apply({ keep_display_on: next })}
            />
          </div>
        </div>

        <p className="policy-foot">
          Battery Saver always takes priority · Closed-lid holds cap at {snapshot.lid.cap_hours}{" "}
          hours
          {snapshot.capabilities.modern_standby
            ? " · Modern Standby limits battery-powered holds"
            : ""}
        </p>
      </section>

      <section className="settings-group">
        <h2 className="h2">Updates</h2>
        <div className="settings-rows">
          <div className="srow">
            <div className="srow__copy">
              <div className="srow__label">Ember{version ? ` v${version}` : ""}</div>
              <div className={`srow__hint${updateError ? " is-error" : ""}`}>{updateHint}</div>
            </div>
            {update ? (
              <Button
                variant="primary"
                onClick={() => void installUpdate()}
                disabled={updateProgress !== null}
              >
                {updateProgress !== null
                  ? `Installing ${updateProgress}%`
                  : `Install v${update.version}`}
              </Button>
            ) : (
              <Button variant="quiet" onClick={() => void checkUpdate()} disabled={updateChecking}>
                {updateChecking ? "Checking…" : "Check now"}
              </Button>
            )}
          </div>
        </div>
      </section>

      <details className="diagnostics">
        <summary>Recent activity</summary>
        <p className="diagnostics__meta">The last decisions from the engine, oldest first.</p>
        {log.length > 0 ? (
          <div className="log glass-pane">
            {log.map((line, index) => (
              <div className="log__line" key={index}>
                {line.t && <span className="log__t">{line.t}</span>}
                <span className="log__m">{line.m}</span>
              </div>
            ))}
          </div>
        ) : (
          <div className="empty-state">No recent activity yet.</div>
        )}
      </details>
    </div>
  );
}
