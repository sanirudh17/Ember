/**
 * Agents — detected coding tools, hook integrations, and live sessions.
 */

import { useEffect, useState } from "react";

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
  const working = snapshot.sessions.filter((s) => s.state === "working").length;
  const idle = snapshot.sessions.length - working;

  const sessions = [...snapshot.sessions].sort((a, b) => {
    if (a.state !== b.state) return a.state === "working" ? -1 : 1;
    if (a.state === "working") return b.working_for_secs - a.working_for_secs;
    return a.agent_name.localeCompare(b.agent_name);
  });

  return (
    <div className="page page--agents">
      <header className="pagehead">
        <h1 className="h1">Agents</h1>
        <Button variant="quiet" size="sm" onClick={() => void api.refreshAgents()}>
          Rescan
        </Button>
      </header>

      <section className="section">
        <div className="sechead">
          <div className="sechead__title">
            <h2 className="h2">Watching</h2>
            <span className="count-pill">
              {detectedCount} / {snapshot.agents.length} detected
            </span>
          </div>
        </div>

        {error && !preview && (
          <p className="inline-error" role="alert">
            {error}
          </p>
        )}

        {detectedCount > 0 ? (
          <table className="table">
            <thead>
              <tr>
                <th>Agent</th>
                <th>Integration</th>
                <th className="num">Last active</th>
                <th className="col-action" />
              </tr>
            </thead>
            <tbody>
              {snapshot.agents.map((a) => (
                <tr key={a.id} className={!a.detected ? "row--muted" : undefined}>
                  <td>
                    <span className="agent-cell">
                      <AgentIcon agent={a.id} monogram={a.monogram} size={18} />
                      <span className="agent-cell__name">{a.name}</span>
                    </span>
                  </td>
                  <td>
                    <span
                      className={`integration-status${
                        a.needs_repair
                          ? " is-warn"
                          : a.installed
                            ? " is-ok"
                            : !a.detected
                              ? " is-missing"
                              : ""
                      }`}
                    >
                      {!a.detected
                        ? "Not found"
                        : a.needs_repair
                          ? "Needs repair"
                          : a.installed
                            ? "Hooks installed"
                            : a.supported
                              ? "Hooks available"
                              : "Process watch"}
                    </span>
                  </td>
                  <td className="num">
                    {a.detected && a.last_event_unix ? relativeTime(a.last_event_unix) : "—"}
                  </td>
                  <td className="col-action">
                    {a.detected && a.supported ? (
                      <Button
                        size="sm"
                        variant={a.needs_repair ? "primary" : "quiet"}
                        disabled={loading}
                        onClick={() =>
                          openPreview(
                            a.id as AgentId,
                            a.installed && !a.needs_repair ? "remove" : "install",
                          )
                        }
                      >
                        {a.installed ? (a.needs_repair ? "Repair" : "Remove") : "Install"}
                      </Button>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <div className="empty-state">No coding tools detected</div>
        )}
      </section>

      <section className="section">
        <div className="sechead">
          <div className="sechead__title">
            <h2 className="h2">Sessions</h2>
            <span className="count-pill">
              {working > 0
                ? `${working} working · ${idle} idle`
                : sessions.length > 0
                  ? `${idle} idle`
                  : "None"}
            </span>
          </div>
        </div>

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
              {sessions.map((s) => {
                const isWorking = s.state === "working";
                return (
                  <tr
                    key={s.session_id || `${s.agent}:${s.folder}`}
                    className={isWorking ? "row--working" : undefined}
                  >
                    <td>
                      <span className="agent-cell">
                        <AgentIcon agent={s.agent} monogram={s.monogram} size={16} />
                        <span className="agent-cell__name">{s.agent_name}</span>
                      </span>
                    </td>
                    <td>{s.folder}</td>
                    <td className="hide-sm">
                      <span className="state-cell">
                        <span
                          className={`sdot${isWorking ? "" : " sdot--idle"}`}
                          aria-hidden="true"
                        />
                        <span className="state-label">{isWorking ? "Working" : "Idle"}</span>
                      </span>
                    </td>
                    <td className="num">{isWorking ? duration(s.working_for_secs) : "—"}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        ) : (
          <div className="empty-state">No open sessions</div>
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
                <h3 className="dialog__title" id="agents-dialog-title">
                  Done
                </h3>
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
                  {target?.op === "install" ? "Install" : "Remove"} {preview.name} hooks
                </h3>
                <p className="dialog__text">{preview.summary}</p>
                {preview.file && <p className="dialog__path">{preview.file}</p>}
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
                  <p className="inline-error" role="alert">
                    {error}
                  </p>
                )}
                <div className="dialog__actions">
                  <Button variant="quiet" disabled={busy} onClick={closeDialog}>
                    Cancel
                  </Button>
                  <Button variant="primary" disabled={busy} onClick={() => void apply()}>
                    {busy ? "Working…" : target?.op === "install" ? "Install" : "Remove"}
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
