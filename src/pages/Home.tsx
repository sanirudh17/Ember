/**
 * Awake — live hold state, framed Solid Glass manual hold toolbar,
 * 4-metric instrument readout, automatic policies, and active sessions.
 */

import { useState } from "react";
import { Laptop, Radar } from "lucide-react";

import { AgentIcon } from "../components/AgentIcon";
import { CustomHoldDialog } from "../components/CustomHoldDialog";
import { Button, Switch } from "../components/ui";
import { api } from "../lib/api";
import { duration, plural } from "../lib/format";
import { useReady, useStore } from "../lib/store";
import { useAction } from "../lib/useAction";
import type { Snapshot } from "../types";

interface StateCopy {
  title: string;
  detail: string;
}

function stateCopy(s: Snapshot): StateCopy {
  if (s.paused) {
    return {
      title: "Paused.",
      detail: s.paused_countdown
        ? `Ember resumes in ${s.paused_countdown}.`
        : "Ember is not holding this PC awake right now.",
    };
  }
  if (s.blocker === "battery_low") {
    return {
      title: "Waiting for power.",
      detail:
        s.blocker_detail ??
        `The battery is below ${s.settings.battery_floor_percent}%, so Ember let go to protect your session.`,
    };
  }
  if (s.blocker === "battery_saver") {
    return {
      title: "Battery saver is on.",
      detail:
        s.blocker_detail ??
        "Windows asked for power saving, so Ember let go until it is off again.",
    };
  }
  if (s.blocker === "lid_cap") {
    return {
      title: "Safety cap reached.",
      detail:
        s.blocker_detail ??
        `Ember restored the lid action after ${s.lid.cap_hours} hours and kept holding the PC.`,
    };
  }
  if (s.manual.active) {
    const detail = s.manual.unlimited
      ? "A manual hold is keeping this PC and display awake until you end it."
      : s.manual.mode === "until"
        ? `A manual hold runs ${s.manual.mode_label.toLowerCase()}.`
        : `A manual hold ends in ${s.manual.countdown}.`;
    return { title: "Holding this PC awake.", detail };
  }
  if (!s.watching) {
    return {
      title: "Watching is off.",
      detail:
        "Ember holds nothing automatically while watching is off. Manual holds remain available.",
    };
  }
  if (s.agents_working > 0) {
    const names = [
      ...new Set(
        s.sessions.filter((x) => x.state === "working").map((x) => x.agent_name),
      ),
    ];
    const detail =
      names.length === 1
        ? `${names[0]} is working.`
        : names.length === 2
          ? `${names[0]} and ${names[1]} are working.`
          : `${names.slice(0, 2).join(", ")} and ${names.length - 2} more are working.`;
    return { title: "Holding this PC awake.", detail };
  }
  if (s.engaged) {
    return {
      title: "Finishing up.",
      detail: "Releasing the hold as the remaining activity settles.",
    };
  }
  return {
    title: "Standing by.",
    detail:
      s.watched_agents > 0
        ? `Watching ${plural(s.watched_agents, "agent", "agents")}; nothing is working right now.`
        : "No coding agents detected on this machine yet.",
  };
}

function readings(s: Snapshot): { k: string; v: string }[] {
  const battery = !s.battery.present
    ? "AC power"
    : `${s.battery.percent ?? "—"}% · ${s.battery.plugged_in ? "Plugged in" : "On battery"}${
        s.battery.saver_on ? " · Saver on" : ""
      }`;
  const lid = s.lid.held
    ? `${duration(s.lid.held_secs)} of ${s.lid.cap_hours}h`
    : s.lid.proof_enabled
      ? "Armed"
      : "Off";
  return [
    {
      k: s.manual.active && !s.manual.unlimited ? "Remaining" : "Held for",
      v:
        s.manual.active && !s.manual.unlimited
          ? s.manual.countdown
          : s.engaged || s.want_awake
            ? duration(s.engaged_for_secs)
            : "—",
    },
    { k: "Agents", v: `${s.agents_working} working · ${s.watched_agents} watched` },
    { k: "Battery", v: battery },
    { k: "Lid hold", v: lid },
  ];
}

export function HomePage({ onViewAgents }: { onViewAgents: () => void }) {
  const { snapshot, settings } = useReady();
  const { apply, setWatching } = useStore();
  const { run, busy, error: actionError } = useAction();
  const [customOpen, setCustomOpen] = useState(false);

  const copy = stateCopy(snapshot);
  const rows = readings(snapshot);
  const lidUsable = snapshot.lid.present && snapshot.capabilities.lid_proof_available;
  const sessions = snapshot.sessions
    .slice()
    .sort(
      (a, b) =>
        Number(b.state === "working") - Number(a.state === "working") ||
        b.working_for_secs - a.working_for_secs,
    );

  return (
    <div className="page page--awake">
      <section
        className={`hold-band${snapshot.engaged ? " is-engaged" : ""}`}
        aria-label="Current hold"
      >
        <div className="hold-band__status">
          <div className="hold-band__headline">
            <span
              className={`status-dot${
                snapshot.engaged ? " is-engaged" : snapshot.blocker ? " is-blocked" : ""
              }`}
              aria-hidden="true"
            />
            <h1 className="h1">{copy.title}</h1>
          </div>
          <p className="status-detail">{copy.detail}</p>
        </div>

        <div className="manual-box glass-pane">
          <span className="manual-box__label">Manual hold</span>
          <div className="actions">
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
              </>
            ) : (
              <>
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
                  Custom{"\u2026"}
                </Button>
                <Button disabled={busy} onClick={() => run(() => api.startManualUnlimited())}>
                  Until released
                </Button>
              </>
            )}
          </div>
        </div>
      </section>

      {actionError && (
        <p className="inline-error" role="alert">
          {actionError}
        </p>
      )}

      <section className="readout-panel glass-pane" aria-label="Hold and device status">
        {rows.map((row) => (
          <div className="readout-cell" key={row.k}>
            <span className="readout-cell__k">{row.k}</span>
            <strong className="readout-cell__v">{row.v}</strong>
          </div>
        ))}
      </section>

      <section className="policy-grid" aria-label="Automatic holds">
        <div className={`policy-option${settings.watching ? " is-enabled" : ""}`}>
          <Radar size={18} className="policy-option__icon" aria-hidden="true" />
          <div className="policy-option__copy">
            <div className="policy-option__label">Watch for agents</div>
            <p className="policy-option__hint">
              Hold automatically while any of your {snapshot.watched_agents} watched tools runs.
            </p>
          </div>
          <div className="policy-option__control">
            <span aria-hidden="true">{settings.watching ? "On" : "Off"}</span>
            <Switch
              label="Watch for agents"
              checked={settings.watching}
              onChange={(next) => void setWatching(next)}
            />
          </div>
        </div>

        <div className={`policy-option${settings.lid_proof ? " is-enabled" : ""}`}>
          <Laptop size={18} className="policy-option__icon" aria-hidden="true" />
          <div className="policy-option__copy">
            <div className="policy-option__label">
              Lid-proof
              {lidUsable && snapshot.lid.held && (
                <span className="policy-row__tag policy-row__tag--active">Held</span>
              )}
            </div>
            <p className="policy-option__hint">
              {!lidUsable
                ? "No lid switch available on this machine."
                : snapshot.lid.note || "Keep working with the lid closed during a hold."}
            </p>
          </div>
          <div className="policy-option__control">
            <span aria-hidden="true">
              {!lidUsable ? "Unavailable" : settings.lid_proof ? "On" : "Off"}
            </span>
            <Switch
              label="Lid-proof"
              checked={settings.lid_proof}
              disabled={!lidUsable}
              onChange={(next) => apply({ lid_proof: next })}
            />
          </div>
        </div>
      </section>

      <section className="sessions-band" aria-label="Sessions">
        <div className="sechead">
          <div className="sechead__title">
            <h2 className="h2">Sessions</h2>
            <span className="count-pill">
              {sessions.length} open {"\u00b7"} {snapshot.agents_working} working
            </span>
          </div>
          <Button variant="ghost" size="sm" onClick={onViewAgents}>
            View agents
          </Button>
        </div>

        {sessions.length > 0 ? (
          <div className="session-list">
            {sessions.map((session) => {
              const working = session.state === "working";
              return (
                <div
                  className={`session-row${working ? " is-working" : ""}`}
                  key={`${session.agent}:${session.session_id}`}
                >
                  <div className="session-row__agent">
                    <AgentIcon agent={session.agent} size={19} />
                    <span className="session-row__name">{session.agent_name}</span>
                  </div>
                  <span className="session-row__folder">{session.folder}</span>
                  <span className={`session-row__state${working ? " is-working" : ""}`}>
                    <span
                      className={`sdot${working ? "" : " sdot--idle"}`}
                      aria-hidden="true"
                    />
                    {working ? duration(session.working_for_secs) : "Idle"}
                  </span>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="empty-state">
            <span>No agents working</span>
            <small>Start a coding agent. Its activity will appear here.</small>
          </div>
        )}
      </section>

      {customOpen && (
        <CustomHoldDialog open={customOpen} onClose={() => setCustomOpen(false)} />
      )}
    </div>
  );
}
