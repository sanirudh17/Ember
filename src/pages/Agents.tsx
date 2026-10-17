/**
 * Agents — what Ember sees on this machine, and one honest dialog for
 * installing or removing hook integrations. Nothing is applied without a
 * preview first.
 */

import { type CSSProperties, useEffect, useState } from "react";

import { AgentIcon } from "../components/AgentIcon";
import { Button } from "../components/ui";
import { api } from "../lib/api";
import { duration, relativeTime } from "../lib/format";
import { useReady } from "../lib/store";
import type { AgentId, ChangePreview, InstallResult } from "../types";

interface Target {
  id: AgentId;
  op: "install" | "remove";
}

export function AgentsPage() {
  const { snapshot } = useReady();

  const [preview, setPreview] = useState<ChangePreview | null>(null);
  const [result, setResult] = useState<InstallResult | null>(null);
  const [target, setTarget] = useState<Target | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const closeDialog = () => {
    if (busy) return;
    setPreview(null);
    setResult(null);
    setTarget(null);
    setError(null);
  };

  const closeAfterSuccess = () => {
    setPreview(null);
    setResult(null);
    setTarget(null);
    setError(null);
    void api.refreshAgents();
  };

  useEffect(() => {
    if (!preview) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        closeDialog();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [preview, busy]);

  const openPreview = (id: AgentId, op: "install" | "remove") => {
    if (loading || busy) return;
    setLoading(true);
    setPreview(null);
    setResult(null);
    setTarget(null);
    setError(null);
    api
      .previewInstall(id, op)
      .then((next) => {
        setPreview(next);
        setTarget({ id, op });
      })
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => setLoading(false));
  };

  const apply = () => {
    if (!target) return;
    setBusy(true);
    setError(null);
    api
      .applyInstall(target.id, target.op)
      .then((res) => setResult(res))
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => setBusy(false));
  };

  const detectedCount = snapshot.agents.filter((a) => a.detected).length;
  const anyDetected = detectedCount > 0;

  const working = snapshot.sessions.filter((s) => s.state === "working").length;
  const idle = snapshot.sessions.length - working;

  const sessions = [...snapshot.sessions].sort((a, b) => {
    if (a.state !== b.state) return a.state === "working" ? -1 : 1;
    if (a.state === "working") return b.working_for_secs - a.working_for_secs;
    return a.agent_name.localeCompare(b.agent_name);
  });

  return (
    <div className="page">
      <header className="pagehead col">
        <h1 className="h1" data-rise style={{ "--i": 0 } as CSSProperties}>
          Agents
        </h1>
        <p className="sub" data-rise style={{ "--i": 1 } as CSSProperties}>
          Ember watches these tools and holds this PC awake while any of them works.
          Hook integrations give it exact session timing; everything else is watched
          by process activity.
        </p>
      </header>

      <section className="section wide" data-reveal>
        <div className="sechead" data-rise style={{ "--i": 0 } as CSSProperties}>
          <h2 className="h2">Watching</h2>
          <Button variant="ghost" size="sm" onClick={() => void api.refreshAgents()}>
            Rescan
          </Button>
        </div>
        <p className="meta">
          {detectedCount} of {snapshot.agents.length} detected
        </p>
        {error && !preview && (
          <p className="meta" role="alert">
            {error}
          </p>
        )}
        {anyDetected ? (
          <table className="table">
            <thead>
              <tr>
                <th>Agent</th>
                <th>Integration</th>
                <th>Since</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {snapshot.agents.map((a, index) => (
                <tr key={a.id} data-rise style={{ "--i": index } as CSSProperties}>
                  <td>
                    <span className="agent-cell">
                      <AgentIcon agent={a.id} monogram={a.monogram} size={16} />
                      <span className="agent-cell__name">{a.name}</span>
                    </span>
                  </td>
                  <td>
                    {!a.detected
                      ? "Not found"
                      : a.installed
                        ? "Hooks installed"
                        : a.supported
                          ? "Hooks available"
                          : "Process watch"}
                  </td>
                  <td>
                    {a.detected && a.last_event_unix
                      ? relativeTime(a.last_event_unix)
                      : "—"}
                  </td>
                  <td>
                    {!a.detected || !a.supported ? null : (
                      <Button
                        size="sm"
                        disabled={loading}
                        onClick={() =>
                          openPreview(
                            a.id as AgentId,
                            a.installed && !a.needs_repair ? "remove" : "install",
                          )
                        }
                      >
                        {a.installed
                          ? a.needs_repair
                            ? "Repair"
                            : "Remove"
                          : "Install hooks"}
                      </Button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="empty-line">
            No coding agents found on this machine yet. Install one, then Rescan.
          </p>
        )}
      </section>

      <section className="section wide" data-reveal>
        <h2 className="h2" data-rise style={{ "--i": 0 } as CSSProperties}>
          Sessions
        </h2>
        <p className="meta">
          {working > 0 ? `${working} working · ${idle} idle` : "Nothing running"}
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
              {sessions.map((s, index) => (
                <tr
                  key={s.session_id || `${s.agent}:${s.folder}`}
                  data-rise
                  style={{ "--i": index } as CSSProperties}
                  className={s.state === "working" ? "row--working" : undefined}
                >
                  <td>
                    <span className="agent-cell">
                      <AgentIcon agent={s.agent} monogram={s.monogram} size={16} />
                      <span className="agent-cell__name">{s.agent_name}</span>
                    </span>
                  </td>
                  <td>{s.folder}</td>
                  <td className="hide-sm">
                    {s.state === "working" ? (
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
                    {s.state === "working" ? duration(s.working_for_secs) : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="empty-line">Nothing is running right now.</p>
        )}
      </section>

      {preview && (
        <div
          className="scrim"
          role="dialog"
          aria-modal="true"
          aria-labelledby="agents-dialog-title"
          onClick={(e) => {
            if (e.target === e.currentTarget) closeDialog();
          }}
        >
          <div className="dialog">
            {result ? (
              <>
                <h3 className="dialog__title" id="agents-dialog-title">Done.</h3>
                <p className="dialog__text">{result.message}</p>
                <div className="dialog__actions">
                  <Button variant="primary" onClick={closeAfterSuccess}>
                    Close
                  </Button>
                </div>
              </>
            ) : (
              <>
                <h3 className="dialog__title" id="agents-dialog-title">
                  {target?.op === "install" ? "Install" : "Remove"} {preview.name}{" "}
                  hooks?
                </h3>
                <p className="dialog__text">{preview.summary}</p>
                <p className="dialog__text">{preview.note}</p>
                {preview.file && (
                  <p className="dialog__text">File: {preview.file}</p>
                )}
                {preview.diff.length > 0 && (
                  <div className="diff">
                    {preview.diff.map((l, i) => (
                      <div
                        key={i}
                        className={
                          l.kind === "context"
                            ? "diff__line"
                            : `diff__line diff__line--${l.kind}`
                        }
                      >
                        {l.text}
                      </div>
                    ))}
                  </div>
                )}
                {error && (
                  <p className="meta" role="alert">
                    {error}
                  </p>
                )}
                <div className="dialog__actions">
                  <Button variant="quiet" disabled={busy} onClick={closeDialog}>
                    Cancel
                  </Button>
                  <Button variant="primary" disabled={busy} onClick={() => void apply()}>
                    {busy
                      ? "Working…"
                      : target?.op === "install"
                        ? "Install hooks"
                        : "Remove"}
                  </Button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
