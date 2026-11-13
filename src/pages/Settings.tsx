/**
 * The surviving choices. Everything Ember decides on its own is narrated
 * here rather than offered as a toggle.
 */

import { type CSSProperties, useEffect, useState } from "react";

import { Button, Segmented, Switch, type SegmentedOption } from "../components/ui";
import { api } from "../lib/api";
import { useReady, useStore } from "../lib/store";
import type { Theme } from "../types";

const THEMES = [
  { value: "system" as const, label: "System" },
  { value: "light" as const, label: "Light" },
  { value: "dark" as const, label: "Dark" },
];

const FLOOR_STEPS = ["10", "15", "20", "30"];

/** String-valued choices for the per cent floor; the active one is mirrored back. */
function floorOptionsFor(percent: number): SegmentedOption<string>[] {
  const options = FLOOR_STEPS.map((value) => ({ value, label: `${value}%` }));
  const value = String(percent);
  // A hand-edited settings file may hold an off-step value; show it as-is.
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
    .slice(-8)
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
        .catch(() => {
          // The log is a convenience; never let it break the screen.
        });
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
      .catch(() => {
        // The version line is cosmetic; never let it break the screen.
      });
  }, []);

  const updateHint = updateError
    ? updateError
    : update
      ? `Version ${update.version} is ready to install. Ember restarts to finish.`
      : updateChecking
        ? "Checking GitHub for a newer release…"
        : "You're on the latest version. Ember checks GitHub on its own every few hours.";

  return (
    <div className="page page--settings">
      <header className="pagehead col">
        <h1 className="h1" data-rise style={{ "--i": 0 } as CSSProperties}>
          Settings
        </h1>
        <p className="sub" data-rise style={{ "--i": 1 } as CSSProperties}>
          Preferences for your everyday workflow.
        </p>
      </header>

      <section className="section wide settings-group">
        <h2 className="h2">General</h2>
        <div className="rows">
          <div className="srow" data-rise style={{ "--i": 0 } as CSSProperties}>
            <div>
              <div className="srow__label">Launch at login</div>
              <div className="srow__hint">Start Ember when you sign in to Windows.</div>
            </div>
            <Switch
              label="Launch at login"
              checked={settings.launch_at_login}
              onChange={(next) => apply({ launch_at_login: next })}
            />
          </div>
          <div className="srow" data-rise style={{ "--i": 1 } as CSSProperties}>
            <div>
              <div className="srow__label">Appearance</div>
              <div className="srow__hint">Follow Windows, or hold it light or dark.</div>
            </div>
            <Segmented
              label="Appearance"
              value={settings.theme}
              options={THEMES}
              onChange={(theme: Theme) => apply({ theme })}
            />
          </div>
          <div className="srow" data-rise style={{ "--i": 2 } as CSSProperties}>
            <div>
              <div className="srow__label">Notifications</div>
              <div className="srow__hint">
                A toast and a chime when Ember takes hold and lets go.
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
      <section className="section wide settings-group">
        <h2 className="h2">Power &amp; display</h2>
        <div className="rows">
          <div className="srow" data-rise style={{ "--i": 3 } as CSSProperties}>
            <div>
              <div className="srow__label">Battery floor</div>
              <div className="srow__hint">
                Release the hold below this level.
              </div>
            </div>
            <Segmented
              label="Battery floor"
              value={String(settings.battery_floor_percent)}
              options={floorOptionsFor(settings.battery_floor_percent)}
              onChange={(value: string) => apply({ battery_floor_percent: Number(value) })}
            />
          </div>
          <div className="srow" data-rise style={{ "--i": 4 } as CSSProperties}>
            <div>
              <div className="srow__label">Require charger for lid hold</div>
              <div className="srow__hint">
                Only override the lid-close action while plugged in.
              </div>
            </div>
            <Switch
              label="Require charger for lid hold"
              checked={settings.require_charger_for_lid}
              onChange={(next) => apply({ require_charger_for_lid: next })}
            />
          </div>
          <div className="srow" data-rise style={{ "--i": 5 } as CSSProperties}>
            <div>
              <div className="srow__label">Keep the screen on</div>
              <div className="srow__hint">
                Keep the display lit during a hold. Otherwise, follow Windows.
              </div>
            </div>
            <Switch
              label="Keep the screen on"
              checked={settings.keep_display_on}
              onChange={(next) => apply({ keep_display_on: next })}
            />
          </div>
        </div>
      </section>

      <div className="safety-note">Battery Saver always takes priority. Closed-lid holds are limited to {snapshot.lid.cap_hours} hours.</div>
      {snapshot.capabilities.modern_standby && (
        <div className="safety-note">Windows limits battery-powered holds on this Modern Standby device. Plug in for extended runs.</div>
      )}

      <section className="section settings-updates" data-reveal>
        <h2 className="h2">Updates</h2>
        <p className="meta">
          Ember updates from GitHub. Every download is signature-verified before it runs.
        </p>
        <div className="rows">
          <div className="srow" data-rise style={{ "--i": 0 } as CSSProperties}>
            <div>
              <div className="srow__label">Ember{version ? ` v${version}` : ""}</div>
              <div className="srow__hint">{updateHint}</div>
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

      <details className="section diagnostics">
        <summary>Recent activity</summary>
        <p className="meta">The last decisions from the engine, oldest first.</p>
        {log.length > 0 ? (
          <div className="log">
            {log.map((line, index) => (
              <div
                className="log__line"
                key={index}
                data-rise
                style={{ "--i": index } as CSSProperties}
              >
                {line.t && <span className="log__t">{line.t}</span>}
                <span className="log__m">{line.m}</span>
              </div>
            ))}
          </div>
        ) : (
          <p className="empty-line">Nothing has happened yet.</p>
        )}
      </details>
    </div>
  );
}
