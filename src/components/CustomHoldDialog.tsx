/**
 * Custom hold dialog: duration in minutes or wall-clock time.
 */

import { useEffect, useRef, useState } from "react";

import { api } from "../lib/api";
import { Button, Segmented } from "./ui";

type Mode = "duration" | "until";

const MINUTES_KEY = "ember.custom-hold-minutes";
const FALLBACK_MINUTES = 45;

function nextQuarterHour(): string {
  const now = new Date();
  const next = new Date(now);
  next.setMinutes(Math.ceil((now.getMinutes() + 1) / 15) * 15, 0, 0);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(next.getHours())}:${pad(next.getMinutes())}`;
}

function storedMinutes(): string {
  try {
    const raw = window.localStorage.getItem(MINUTES_KEY);
    const n = Number(raw);
    if (Number.isFinite(n) && n >= 1 && n <= 1440) return String(Math.floor(n));
  } catch {
    // Ignore storage access errors.
  }
  return String(FALLBACK_MINUTES);
}

export function CustomHoldDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [mode, setMode] = useState<Mode>("duration");
  const [minutes, setMinutes] = useState<string>(storedMinutes);
  const [time, setTime] = useState<string>(nextQuarterHour);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const minutesRef = useRef<HTMLInputElement>(null);
  const timeRef = useRef<HTMLInputElement>(null);

  const close = () => {
    if (busy) return;
    onClose();
  };

  useEffect(() => {
    if (!open) return;
    setError(null);
    setBusy(false);
    const target = mode === "duration" ? minutesRef.current : timeRef.current;
    target?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        close();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  if (!open) return null;

  const start = () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    if (mode === "duration") {
      const n = Math.floor(Number(minutes));
      if (!Number.isFinite(n) || n < 1 || n > 1440) {
        setError("Enter 1 to 1440 minutes.");
        setBusy(false);
        return;
      }
      api
        .startManual(n * 60)
        .then(() => {
          try {
            window.localStorage.setItem(MINUTES_KEY, String(n));
          } catch {
            // Ignore storage errors.
          }
          onClose();
        })
        .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
        .finally(() => setBusy(false));
    } else {
      const [h, m] = time.split(":").map(Number);
      if (!Number.isFinite(h) || !Number.isFinite(m)) {
        setError("Pick a time.");
        setBusy(false);
        return;
      }
      api
        .startManualUntil(h, m)
        .then(() => onClose())
        .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
        .finally(() => setBusy(false));
    }
  };

  return (
    <div className="scrim" role="presentation" onClick={close}>
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="custom-hold-title"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="dialog__title" id="custom-hold-title">
          Custom hold
        </h2>
        <Segmented<Mode>
          label="Custom hold mode"
          value={mode}
          options={[
            { value: "duration", label: "Duration" },
            { value: "until", label: "Until time" },
          ]}
          onChange={(next) => {
            setMode(next);
            setError(null);
            requestAnimationFrame(() => {
              (next === "duration" ? minutesRef.current : timeRef.current)?.focus();
            });
          }}
        />
        <div className="dialog__field">
          {mode === "duration" ? (
            <>
              <label htmlFor="custom-hold-minutes">Minutes</label>
              <input
                id="custom-hold-minutes"
                ref={minutesRef}
                className="input"
                type="number"
                min={1}
                max={1440}
                value={minutes}
                disabled={busy}
                onChange={(event) => setMinutes(event.target.value)}
              />
            </>
          ) : (
            <>
              <label htmlFor="custom-hold-time">Until</label>
              <input
                id="custom-hold-time"
                ref={timeRef}
                className="input"
                type="time"
                value={time}
                disabled={busy}
                onChange={(event) => setTime(event.target.value)}
              />
            </>
          )}
        </div>
        {error && (
          <p className="inline-error" role="alert">
            {error}
          </p>
        )}
        <div className="dialog__actions">
          <Button variant="quiet" disabled={busy} onClick={close}>
            Cancel
          </Button>
          <Button variant="primary" disabled={busy} onClick={start}>
            {busy ? "Starting…" : "Start"}
          </Button>
        </div>
      </div>
    </div>
  );
}
