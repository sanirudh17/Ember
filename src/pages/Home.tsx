/**
 * The instrument. One state line (the largest thing in the window), the
 * controls a person actually touches, the live readings, and what is working
 * right now. Everything else Ember decides on its own.
 */

import { useState, type CSSProperties } from "react";

import { AgentIcon } from "../components/AgentIcon";
import { CustomHoldDialog } from "../components/CustomHoldDialog";
import { Button, Switch } from "../components/ui";
import { api } from "../lib/api";
import { duration, plural } from "../lib/format";
import { useReady, useStore } from "../lib/store";
import { useAction } from "../lib/useAction";
import type { Snapshot } from "../types";

interface StateCopy {
  key: string;
  lines: string[];
  sub: string;
}

/**
 * The one authored line per state. It changes rarely, so the masked reveal
 * replays exactly when the truth changes.
 */
function stateCopy(s: Snapshot): StateCopy {
  if (s.paused) {
    return {
      key: "paused",
      lines: ["Paused."],
      sub: s.paused_countdown
        ? `Ember resumes in ${s.paused_countdown}.`
        : "Ember is not holding this PC awake right now.",
    };
  }
  if (!s.watching) {
    return {
      key: "off",
      lines: ["Watching", "is off."],
      sub: "Ember holds nothing awake while watching is off. Turn it back on to follow your agents again.",
    };
  }
  if (s.blocker === "battery_low") {
    return {
      key: "blocked-battery",
      lines: ["Waiting", "for power."],
      sub:
        s.blocker_detail ??
        "The battery is low, so Ember let go to protect your session.",
    };
  }
  if (s.blocker === "battery_saver") {
    return {
      key: "blocked-saver",
      lines: ["Battery saver", "is on."],
      sub:
        s.blocker_detail ??
        "Windows asked for power saving, so Ember let go until it is off again.",
    };
  }
  if (s.blocker === "lid_cap") {
    return {
      key: "blocked-cap",
      lines: ["Safety cap", "reached."],
      sub:
        s.blocker_detail ??
        "Ember restored the lid action after six hours and kept holding the PC.",
    };
  }
  if (s.manual.active) {
    const sub = s.manual.unlimited
      ? "A manual hold is keeping it up until you end it."
      : s.manual.mode === "until"
        ? `A manual hold runs ${s.manual.mode_label.toLowerCase()}.`
        : `A manual hold ends in ${s.manual.countdown}.`;
    return { key: "manual", lines: ["Holding this PC", "awake."], sub };
  }
  if (s.agents_working > 0) {
    const names = [
      ...new Set(
        s.sessions.filter((x) => x.state === "working").map((x) => x.agent_name),
      ),
    ];
    const sub =
      names.length === 1
        ? `${names[0]} is working.`
        : names.length === 2
          ? `${names[0]} and ${names[1]} are working.`
          : `${names.slice(0, 2).join(", ")} and ${names.length - 2} more are working.`;
    return { key: "agents", lines: ["Holding this PC", "awake."], sub };
  }
  return {
    key: "idle",
    lines: ["Standing by."],
    sub:
      s.watched_agents > 0
        ? `Watching ${plural(s.watched_agents, "agent", "agents")}; nothing is working right now.`
        : "No coding agents detected on this machine yet.",
  };
}

function readings(s: Snapshot): { k: string; v: string }[] {
  const battery = !s.battery.present
    ? "Not present"
    : `${s.battery.percent ?? "—"}% · ${s.battery.plugged_in ? "plugged in" : "on battery"}${
        s.battery.saver_on ? " · saver on" : ""
      }`;
  const lid = s.lid.held
    ? `${duration(s.lid.held_secs)} of ${s.lid.cap_hours}h`
    : s.lid.proof_enabled
      ? "Armed"
      : "Off";
  return [
    { k: "Held for", v: s.engaged || s.want_awake ? duration(s.engaged_for_secs) : "—" },
    { k: "Agents", v: `${s.agents_working} working · ${s.watched_agents} watched` },
    { k: "Battery", v: battery },
    { k: "Lid hold", v: lid },
  ];
}

export function HomePage() {
  const { snapshot, settings } = useReady();
  const { apply, setWatching } = useStore();
  const { run, busy, error: actionError } = useAction();
  const [customOpen, setCustomOpen] = useState(false);

  const copy = stateCopy(snapshot);
  const rows = readings(snapshot);

  const lidUsable = snapshot.lid.present && snapshot.capabilities.lid_proof_available;
  const lidHint = !lidUsable
    ? "No lid switch to override on this machine."
    : snapshot.lid.note ||
      "Overrides the lid-close action while Ember holds this PC awake.";

  const sessions = [...snapshot.sessions].sort((a, b) => {
    if (a.state !== b.state) return a.state === "working" ? -1 : 1;
    if (a.state === "working") return b.working_for_secs - a.working_for_secs;
    return a.agent_name.localeCompare(b.agent_name);
  });

  return (
    <div className="page page--awake">
      <section className="state col">
        <h1 className="state__line" key={copy.key}>
          {copy.lines.map((line, index) => (
            <span className="line" key={index}>
              <span style={{ "--i": index } as CSSProperties}>{line}</span>
            </span>
          ))}
        </h1>
        <p
          className="state__sub"
          data-rise
          style={{ "--i": copy.lines.length } as CSSProperties}
        >
          {copy.sub}
        </p>

        <div
          className="actions"
          data-rise
          style={{ "--i": copy.lines.length + 1 } as CSSProperties}
        >
          {snapshot.manual.active ? (
            <>
              <Button
                variant="primary"
                disabled={busy}
                onClick={() => run(() => api.stopManual())}
              >
                End hold
              </Button>
              <Button disabled={busy} onClick={() => run(() => api.extendManual(1800))}>
                +30m
              </Button>
              <Button disabled={busy} onClick={() => run(() => api.extendManual(3600))}>
                +1h
              </Button>
              {!snapshot.manual.unlimited && (
                <span className="actions__count">
                  {snapshot.manual.countdown} left
                </span>
              )}
            </>
          ) : (
            <>
              <span className="actions__label">Hold awake for</span>
              <Button disabled={busy} onClick={() => run(() => api.startManual(1800))}>
                30m
              </Button>
              <Button disabled={busy} onClick={() => run(() => api.startManual(3600))}>
                1h
              </Button>
              <Button disabled={busy} onClick={() => run(() => api.startManual(7200))}>
                2h
              </Button>
              <Button disabled={busy} onClick={() => setCustomOpen(true)}>
                Custom…
              </Button>
              <Button disabled={busy} onClick={() => run(() => api.startManualUnlimited())}>
                Until released
              </Button>
            </>
          )}
        </div>
        {actionError && (
          <p className="meta" role="alert">
            {actionError}
          </p>
        )}
      </section>

      {/* The dialog lives at page level, never inside an animated subtree:
          a frozen entrance transform would capture its fixed scrim. */}
      {customOpen && (
        <CustomHoldDialog open={customOpen} onClose={() => setCustomOpen(false)} />
      )}

      <section className="section band" data-reveal>
        <div className="field wide" data-ui>
          <div className="field__grid">
            {rows.map((row, index) => (
              <div
                className="readout"
                key={row.k}
                data-rise
                style={{ "--i": index } as CSSProperties}
              >
                <div className="readout__k">{row.k}</div>
                <div className="readout__v">{row.v}</div>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="section wide section--ctrls" data-reveal>
        <div className="panel panel--ctrls">
          <div className="ctrl" data-rise style={{ "--i": 0 } as CSSProperties}>
            <div>
              <div className="ctrl__label">Watch for agents</div>
              <div className="ctrl__hint">
                Keeps this PC awake while any agent is working, and lets go when they
                stop.
              </div>
            </div>
            <Switch
              label="Watch for agents"
              checked={settings.watching}
              onChange={(next) => void setWatching(next)}
            />
          </div>
          <div className="ctrl" data-rise style={{ "--i": 1 } as CSSProperties}>
            <div>
              <div className="ctrl__label">Lid-proof</div>
              <div className="ctrl__hint">{lidHint}</div>
            </div>
            <Switch
              label="Lid-proof"
              checked={settings.lid_proof}
              disabled={!lidUsable}
              title={!lidUsable ? "No lid switch to override on this machine." : undefined}
              onChange={(next) => apply({ lid_proof: next })}
            />
          </div>
        </div>
      </section>

      <section className="section wide section--working" data-reveal>
        <h2 className="h2">Working now</h2>
        <p className="meta">
          {snapshot.agents_working > 0
            ? `${plural(snapshot.agents_working, "agent", "agents")} working · ${plural(snapshot.watched_agents, "agent", "agents")} watched`
            : `Nothing working · ${plural(snapshot.watched_agents, "agent", "agents")} watched`}
        </p>
        {sessions.length > 0 ? (
          <table className="table">
            <thead>
              <tr>
                <th>Agent</th>
                <th>Folder</th>
                <th className="hide-sm">State</th>
                <th className="num">For</th>
              </tr>
            </thead>
            <tbody>
              {sessions.map((session, index) => (
                <tr
                  key={session.session_id || `${session.agent}:${session.folder}`}
                  data-rise
                  style={{ "--i": index } as CSSProperties}
                  className={session.state === "working" ? "row--working" : undefined}
                >
                  <td>
                    <span className="agent-cell">
                      <AgentIcon
                        agent={session.agent}
                        monogram={session.monogram}
                        size={16}
                      />
                      <span className="agent-cell__name">{session.agent_name}</span>
                    </span>
                  </td>
                  <td>{session.folder}</td>
                  <td className="hide-sm">
                    {session.state === "working" ? (
                      <>
                        <span className="sdot" />
                        <span className="state-label">Working</span>
                      </>
                    ) : (
                      <>
                        <span className="sdot sdot--idle" />
                        <span className="state-label">Idle</span>
                      </>
                    )}
                  </td>
                  <td className="num">
                    {session.state === "working" ? duration(session.working_for_secs) : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="empty-line">
            Nothing is running right now. Start a coding agent and it will appear here.
          </p>
        )}
      </section>

      <footer className="pagefoot">
        <span>
          Ember keeps this PC awake while its agents work — and gives it back when they
          stop.
        </span>
        <span>{snapshot.capabilities.os_name}</span>
      </footer>
    </div>
  );
}
