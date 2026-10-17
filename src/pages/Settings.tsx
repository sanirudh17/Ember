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
    confirmQuit,
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
          A handful of choices. Everything else Ember decides on its own — the list below
          tells you what those decisions are.
        </p>
      </header>

      <section className="section wide">
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
          <div className="srow" data-rise style={{ "--i": 3 } as CSSProperties}>
            <div>
              <div className="srow__label">Battery floor</div>
              <div className="srow__hint">
                Ember releases the hold when the battery drops below this.
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
              <div className="srow__label">Closed-lid hold requires the charger</div>
              <div className="srow__hint">
                When on, Ember only changes the lid-close action while the laptop is plugged in.
              </div>
            </div>
            <Switch
              label="Closed-lid hold requires the charger"
              checked={settings.require_charger_for_lid}
              onChange={(next) => apply({ require_charger_for_lid: next })}
            />
          </div>
          <div className="srow" data-rise style={{ "--i": 5 } as CSSProperties}>
            <div>
              <div className="srow__label">Keep the screen on</div>
              <div className="srow__hint">
                When on, the screen stays lit while Ember holds. Off, Windows may turn the screen
                off while the PC stays awake.
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

      <section className="section" data-reveal>
        <h2 className="h2">Always on</h2>
        <p className="meta">Protections Ember applies on its own, without asking.</p>
        <div className="facts">
          <div className="fact" data-rise style={{ "--i": 0 } as CSSProperties}>
            <div className="fact__k">Battery floor</div>
            <div className="fact__v">{settings.battery_floor_percent}%</div>
            <div className="fact__note">
              Ember lets go below {settings.battery_floor_percent}% and while Windows Battery Saver
              is on.
            </div>
          </div>
          <div className="fact" data-rise style={{ "--i": 1 } as CSSProperties}>
            <div className="fact__k">Lid safety cap</div>
            <div className="fact__v">{snapshot.lid.cap_hours} hours</div>
            <div className="fact__note">
              The lid action is never overridden for longer than that.
            </div>
          </div>
          <div className="fact" data-rise style={{ "--i": 2 } as CSSProperties}>
            <div className="fact__k">The display</div>
            <div className="fact__v">{settings.keep_display_on ? "Kept on" : "Hands off"}</div>
            <div className="fact__note">
              {settings.keep_display_on
                ? "The screen stays awake while Ember holds the PC."
                : "Your screen follows the Windows power plan."}
            </div>
          </div>
        </div>
      </section>

      <section className="section" data-reveal>
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

      <section className="section col" data-reveal>
        <h2 className="h2">Recent activity</h2>
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
      </section>

      <footer className="pagefoot">
        <span>Ember on {snapshot.capabilities.os_name}.</span>
        <span className="pagefoot__links">
          <button type="button" className="tlink" onClick={() => void api.resetWindow()}>
            Reset window position
          </button>
          <button type="button" className="tlink" onClick={confirmQuit}>
            Quit Ember
          </button>
        </span>
      </footer>
    </div>
  );
}
