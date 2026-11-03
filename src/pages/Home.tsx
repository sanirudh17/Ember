/**
 * The instrument. One state line (the largest thing in the window), the
 * controls a person actually touches, the live readings, and what is working
 * right now. Everything else Ember decides on its own.
 */

import { useState } from "react";
import { Laptop, Radar } from "lucide-react";

import { InterfaceIcon } from "../components/InterfaceIcon";
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
  if (!s.watching) {
    return {
      key: "off",
      lines: ["Watching", "is off."],
      sub: "Ember holds nothing awake while watching is off. Turn it back on to follow your agents again.",
    };
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
  if (s.engaged) {
    return { key: "settling", lines: ["Finishing up."], sub: "Releasing the hold as the remaining activity settles." };
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

export function HomePage({ onViewAgents }: { onViewAgents: () => void }) {
  const { snapshot, settings } = useReady();
  const { apply, setWatching } = useStore();
  const { run, busy, error: actionError } = useAction();
  const [customOpen, setCustomOpen] = useState(false);
  const copy = stateCopy(snapshot);
  const rows = readings(snapshot);
  const lidUsable = snapshot.lid.present && snapshot.capabilities.lid_proof_available;
  const sessions = snapshot.sessions.slice().sort((a,b) => Number(b.state === "working") - Number(a.state === "working") || b.working_for_secs-a.working_for_secs);

  return <div className="page page--awake awake-workspace">
    <div className="awake-overview">
    <section className={`hold-surface${snapshot.engaged ? " is-engaged" : ""}`} aria-label="Current hold">
      <div className="hold-surface__status">
        <div>
          <h1 className="state__line">{copy.lines.join(" ")}</h1>
          <p className="state__sub">{copy.sub}</p>
        </div>

      </div>
      <div className="hold-toolbar">
        <span className="hold-toolbar__label">Manual hold</span>
        <div className="actions">
          {snapshot.manual.active ? <>
            <Button variant="primary" disabled={busy} onClick={() => run(() => api.stopManual())}>End hold</Button>
            <Button disabled={busy} onClick={() => run(() => api.extendManual(1800))}>+30m</Button>
            <Button disabled={busy} onClick={() => run(() => api.extendManual(3600))}>+1h</Button>
          </> : <>
            <Button disabled={busy} onClick={() => run(() => api.startManual(1800))}>30m</Button>
            <Button disabled={busy} onClick={() => run(() => api.startManual(3600))}>1h</Button>
            <Button disabled={busy} onClick={() => run(() => api.startManual(7200))}>2h</Button>
            <Button disabled={busy} onClick={() => setCustomOpen(true)}>Custom{"\u2026"}</Button>
            <Button disabled={busy} onClick={() => run(() => api.startManualUnlimited())}>Until released</Button>
          </>}
        </div>
      </div>
      {actionError && <p className="meta" role="alert">{actionError}</p>}
    </section>
    <section className="awake-summary" aria-label="Hold and device status">
      {rows.map((row,index) => <div className="summary-reading" key={row.k}>
        <span>{index === 0 && snapshot.manual.active && !snapshot.manual.unlimited ? "Remaining" : row.k}</span>
        <strong>{index === 0 && snapshot.manual.active && !snapshot.manual.unlimited ? snapshot.manual.countdown : row.v}</strong>
      </div>)}
    </section>
    </div>

    <section className="awake-automation" aria-label="Automatic holds">
      <div className={`automation-option${settings.watching ? " is-enabled" : ""}`}>
        <Radar size={19} className="automation-option__icon" aria-hidden="true" />
        <div className="automation-option__copy">
          <div className="ctrl__label">Watch for agents</div>
          <p className="ctrl__hint">Hold automatically while any of your {snapshot.watched_agents} watched tools is working.</p>
        </div>
        <div className="automation-option__control">
          <span aria-hidden="true">{settings.watching ? "On" : "Off"}</span>
          <Switch label="Watch for agents" checked={settings.watching} onChange={next => void setWatching(next)} />
        </div>
      </div>
      <div className={`automation-option${settings.lid_proof ? " is-enabled" : ""}`}>
        <Laptop size={19} className="automation-option__icon" aria-hidden="true" />
        <div className="automation-option__copy">
          <div className="ctrl__label">Lid-proof</div>
          <p className="ctrl__hint">{!lidUsable ? "No lid switch available." : snapshot.lid.note || "Keep working with the lid closed during a hold."}</p>
        </div>
        <div className="automation-option__control">
          <span aria-hidden="true">{!lidUsable ? "Unavailable" : settings.lid_proof ? "On" : "Off"}</span>
          <Switch label="Lid-proof" checked={settings.lid_proof} disabled={!lidUsable} onChange={next => apply({lid_proof:next})} />
        </div>
      </div>
    </section>

    <section className="awake-sessions">
      <div className="sechead"><h2 className="h2">Sessions <span className="count-label">{sessions.length} open {"\u00b7"} {snapshot.agents_working} working</span></h2>
        <Button variant="ghost" size="sm" onClick={onViewAgents}>View agents</Button></div>
      {sessions.length ? <div className="active-session-list">{sessions.map(session =>
        <div className="active-session" key={`${session.agent}:${session.session_id}`}>
          <AgentIcon agent={session.agent} size={20} />
          <div><span>{session.agent_name}</span><small>{session.folder}</small></div>
          <span className="active-session__time">{session.state === "working" ? duration(session.working_for_secs) : "Idle"}</span>
        </div>
      )}</div> : <div className="awake-empty">
        <InterfaceIcon name="agents" size={30} /><span>No agents working</span>
        <small>Start a coding agent. Its activity will appear here.</small>
      </div>}
    </section>

    {customOpen && <CustomHoldDialog open={customOpen} onClose={() => setCustomOpen(false)} />}
  </div>;
}
