/**
 * Awake — live hold state, framed manual hold toolbar, 4-metric instrument
 * readout, automatic holds, and active sessions.
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
      title: "Paused",
      detail: s.paused_countdown ? `Resumes in ${s.paused_countdown}` : "All holds paused",
    };
  }
  if (s.blocker === "battery_low") {
    return {
      title: "Waiting for power",
      detail: `Below ${s.settings.battery_floor_percent}% battery floor`,
    };
  }
  if (s.blocker === "battery_saver") {
    return {
      title: "Battery saver on",
      detail: "Hold paused by Windows",
    };
  }
  if (s.blocker === "lid_cap") {
    return {
      title: "Lid cap reached",
      detail: `Restored after ${s.lid.cap_hours}h`,
    };
  }
  if (s.manual.active) {
    const detail = s.manual.unlimited
      ? "Manual hold · Until released"
      : s.manual.mode === "until"
        ? `Manual hold · ${s.manual.mode_label}`
        : `Manual hold · ${s.manual.countdown} left`;
    return { title: "Holding awake", detail };
  }
  if (!s.watching) {
    return {
      title: "Watching off",
      detail: "Manual holds remain available",
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
        ? `${names[0]} working`
        : names.length === 2
          ? `${names[0]} & ${names[1]} working`
          : `${names.slice(0, 2).join(", ")} +${names.length - 2} working`;
    return { title: "Holding awake", detail };
  }
  if (s.engaged) {
    return { title: "Finishing up", detail: "Releasing hold" };
  }
  return {
    title: "Standing by",
    detail:
      s.watched_agents > 0
        ? `Watching ${plural(s.watched_agents, "agent", "agents")}`
        : "No agents detected",
  };
}

type ReadingTone = "default" | "accent" | "ok" | "warn" | "muted";

function readings(s: Snapshot): { k: string; v: string; tone: ReadingTone }[] {
  const battery = !s.battery.present
    ? "AC power"
    : `${s.battery.percent ?? "—"}% · ${s.battery.plugged_in ? "Plugged in" : "On battery"}${
        s.battery.saver_on ? " · Saver" : ""
      }`;
  const batteryTone: ReadingTone = !s.battery.present
    ? "ok"
    : s.battery.saver_on || (s.battery.percent !== null && s.battery.percent <= s.settings.battery_floor_percent)
      ? "warn"
      : s.battery.plugged_in
        ? "ok"
        : "default";

  const lid = s.lid.held
    ? `${duration(s.lid.held_secs)} of ${s.lid.cap_hours}h`
    : s.lid.proof_enabled
      ? "Armed"
      : "Off";
  const lidTone: ReadingTone = s.lid.held ? "accent" : s.lid.proof_enabled ? "ok" : "muted";

  const holding = s.engaged || s.want_awake;
  return [
    {
      k: s.manual.active && !s.manual.unlimited ? "Remaining" : "Held for",
      v:
        s.manual.active && !s.manual.unlimited
          ? s.manual.countdown
          : holding
            ? duration(s.engaged_for_secs)
            : "—",
      tone: holding ? "accent" : "muted",
    },
    {
      k: "Agents",
      v: `${s.agents_working} working · ${s.watched_agents} watched`,
      tone: s.agents_working > 0 ? "ok" : "default",
    },
    { k: "Battery", v: battery, tone: batteryTone },
    { k: "Lid hold", v: lid, tone: lidTone },
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
    <div className={`page page--awake${snapshot.engaged ? " is-engaged" : ""}`}>
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

        <div className="manual-box surface-pane">
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

      <section className="readout-panel surface-pane" aria-label="Hold and device status">
        {rows.map((row) => (
          <div className={`readout-cell readout-cell--${row.tone}`} key={row.k}>
            <span className="readout-cell__k">{row.k}</span>
            <strong className="readout-cell__v">{row.v}</strong>
          </div>
        ))}
      </section>

      <section className="policy-grid" aria-label="Automatic holds">
        <div className={`policy-row${settings.watching ? " is-enabled" : ""}`}>
          <div className="policy-row__lead">
            <Radar size={17} strokeWidth={1.75} className="policy-row__icon" aria-hidden="true" />
            <span className="policy-row__label">Watch for agents</span>
          </div>
          <Switch
            label="Watch for agents"
            checked={settings.watching}
            onChange={(next) => void setWatching(next)}
          />
        </div>

        <div className={`policy-row${settings.lid_proof ? " is-enabled" : ""}`}>
          <div className="policy-row__lead">
            <Laptop size={17} strokeWidth={1.75} className="policy-row__icon" aria-hidden="true" />
            <span className="policy-row__label">Lid-proof</span>
            {!lidUsable && <span className="policy-row__tag">Unavailable</span>}
            {lidUsable && snapshot.lid.held && (
              <span className="policy-row__tag policy-row__tag--active">Held</span>
            )}
          </div>
          <Switch
            label="Lid-proof"
            checked={settings.lid_proof}
            disabled={!lidUsable}
            onChange={(next) => apply({ lid_proof: next })}
          />
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
                    <AgentIcon agent={session.agent} size={18} />
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
          <div className="empty-state">No open sessions</div>
        )}
      </section>

      {customOpen && (
        <CustomHoldDialog open={customOpen} onClose={() => setCustomOpen(false)} />
      )}
    </div>
  );
}
