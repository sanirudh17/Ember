/**
 * A one-off custom hold: either "for N minutes" or "until HH:MM".
 *
 * Uses a bespoke Ember time control (direct HH:MM entry plus a clean Ember
 * hour/minute popover) so Windows never pops up the native browser time
 * spinner, and keeps generous spacing between labels, fields, and units.
 */

import { useEffect, useRef, useState } from "react";
import { Clock, Minus, Plus } from "lucide-react";

import { api } from "../lib/api";
import { Button, Segmented } from "./ui";

type Mode = "duration" | "until";

const MINUTES_KEY = "ember.custom-hold-minutes";
const FALLBACK_MINUTES = 45;
const HOURS = Array.from({ length: 24 }, (_, i) => i);
const MINUTE_STEPS = Array.from({ length: 12 }, (_, i) => i * 5);

const pad2 = (n: number) => String(n).padStart(2, "0");

/** The next quarter hour as [hour, minute], so the time picker starts ahead of now. */
function nextQuarterHourParts(): [string, string] {
  const now = new Date();
  const next = new Date(now);
  next.setMinutes(Math.ceil((now.getMinutes() + 1) / 15) * 15, 0, 0);
  return [pad2(next.getHours()), pad2(next.getMinutes())];
}

function storedMinutes(): string {
  try {
    const raw = window.localStorage.getItem(MINUTES_KEY);
    const n = Number(raw);
    if (Number.isFinite(n) && n >= 1 && n <= 1440) return String(Math.floor(n));
  } catch {
    // A missing or blocked storage just means the fallback.
  }
  return String(FALLBACK_MINUTES);
}

export function CustomHoldDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [mode, setMode] = useState<Mode>("duration");
  const [minutes, setMinutes] = useState<string>(storedMinutes);
  const [[initH, initM]] = useState(nextQuarterHourParts);
  const [hour, setHour] = useState<string>(initH);
  const [minute, setMinute] = useState<string>(initM);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const minutesRef = useRef<HTMLInputElement>(null);
  const hourRef = useRef<HTMLInputElement>(null);
  const minuteRef = useRef<HTMLInputElement>(null);
  const pickerRef = useRef<HTMLDivElement>(null);
  const activeHourRef = useRef<HTMLButtonElement>(null);
  const activeMinuteRef = useRef<HTMLButtonElement>(null);

  const close = () => {
    if (busy) return;
    setPickerOpen(false);
    onClose();
  };

  useEffect(() => {
    if (!open) return;
    setError(null);
    setBusy(false);
    setPickerOpen(false);
    const [h, m] = nextQuarterHourParts();
    setHour(h);
    setMinute(m);
    requestAnimationFrame(() => {
      const target = mode === "duration" ? minutesRef.current : hourRef.current;
      target?.focus();
    });
    // Reset defaults each time the dialog opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        if (pickerOpen) {
          setPickerOpen(false);
        } else if (!busy) {
          onClose();
        }
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, pickerOpen, busy, onClose]);

  useEffect(() => {
    if (!pickerOpen) return;
    activeHourRef.current?.scrollIntoView({ block: "center" });
    activeMinuteRef.current?.scrollIntoView({ block: "center" });
    const onPointerDown = (event: MouseEvent) => {
      if (pickerRef.current && !pickerRef.current.contains(event.target as Node)) {
        setPickerOpen(false);
      }
    };
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [pickerOpen]);

  if (!open) return null;

  const stepMinutes = (delta: number) => {
    const current = Math.floor(Number(minutes));
    const base = Number.isFinite(current) && current >= 1 ? current : FALLBACK_MINUTES;
    const next = Math.min(1440, Math.max(5, Math.round((base + delta) / 5) * 5));
    setMinutes(String(next));
    setError(null);
  };

  const parsedHour = () => {
    const h = Number(hour);
    return Number.isInteger(h) && h >= 0 && h <= 23 ? h : null;
  };

  const parsedMinute = () => {
    const m = Number(minute);
    return Number.isInteger(m) && m >= 0 && m <= 59 ? m : null;
  };

  const normalizeHour = () => {
    const h = parsedHour();
    if (h !== null) setHour(pad2(h));
  };

  const normalizeMinute = () => {
    const m = parsedMinute();
    if (m !== null) setMinute(pad2(m));
  };

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
            // Holding matters; remembering the number does not.
          }
          onClose();
        })
        .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
        .finally(() => setBusy(false));
    } else {
      const h = parsedHour();
      const m = parsedMinute();
      if (h === null || m === null) {
        setError("Enter a valid 24-hour time (00:00 – 23:59).");
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

  const selectedH = parsedHour();
  const selectedM = parsedMinute();

  return (
    <div className="scrim" role="presentation" onClick={close}>
      <div
        className="dialog dialog--custom"
        role="dialog"
        aria-modal="true"
        aria-labelledby="custom-hold-title"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="dialog__title" id="custom-hold-title">
          Custom hold
        </h2>
        <p className="dialog__text">A one-off hold; agents still hold on their own.</p>

        <div className="custom-dialog__mode">
          <Segmented<Mode>
            label="Custom hold mode"
            value={mode}
            options={[
              { value: "duration", label: "For a while" },
              { value: "until", label: "Until a time" },
            ]}
            onChange={(next) => {
              setMode(next);
              setPickerOpen(false);
              setError(null);
              requestAnimationFrame(() => {
                (next === "duration" ? minutesRef.current : hourRef.current)?.focus();
              });
            }}
          />
        </div>

        <form
          onSubmit={(event) => {
            event.preventDefault();
            start();
          }}
        >
          {mode === "duration" ? (
            <div className="custom-field">
              <label className="custom-field__label" htmlFor="custom-hold-minutes">
                Duration
              </label>
              <div className="custom-duration">
                <button
                  type="button"
                  className="custom-step"
                  aria-label="Decrease by 15 minutes"
                  disabled={busy}
                  onClick={() => stepMinutes(-15)}
                >
                  <Minus size={13} />
                </button>
                <input
                  id="custom-hold-minutes"
                  ref={minutesRef}
                  className="input custom-duration__input"
                  type="text"
                  inputMode="numeric"
                  pattern="[0-9]*"
                  value={minutes}
                  disabled={busy}
                  onChange={(event) => {
                    const digits = event.target.value.replace(/\D/g, "").slice(0, 4);
                    setMinutes(digits);
                    setError(null);
                  }}
                />
                <button
                  type="button"
                  className="custom-step"
                  aria-label="Increase by 15 minutes"
                  disabled={busy}
                  onClick={() => stepMinutes(15)}
                >
                  <Plus size={13} />
                </button>
                <span className="custom-field__unit">minutes</span>
              </div>
            </div>
          ) : (
            <div className="custom-field" ref={pickerRef}>
              <label className="custom-field__label" htmlFor="custom-hold-hour">
                Hold until
              </label>
              <div className="custom-time">
                <div className={`custom-time__box${pickerOpen ? " is-open" : ""}`}>
                  <input
                    id="custom-hold-hour"
                    ref={hourRef}
                    className="custom-time__seg"
                    type="text"
                    inputMode="numeric"
                    aria-label="Hour"
                    maxLength={2}
                    value={hour}
                    disabled={busy}
                    onFocus={(e) => e.currentTarget.select()}
                    onBlur={normalizeHour}
                    onChange={(event) => {
                      const digits = event.target.value.replace(/\D/g, "").slice(0, 2);
                      setHour(digits);
                      setError(null);
                      if (digits.length === 2) minuteRef.current?.focus();
                    }}
                  />
                  <span className="custom-time__colon" aria-hidden="true">
                    :
                  </span>
                  <input
                    id="custom-hold-minute"
                    ref={minuteRef}
                    className="custom-time__seg"
                    type="text"
                    inputMode="numeric"
                    aria-label="Minute"
                    maxLength={2}
                    value={minute}
                    disabled={busy}
                    onFocus={(e) => e.currentTarget.select()}
                    onBlur={normalizeMinute}
                    onChange={(event) => {
                      const digits = event.target.value.replace(/\D/g, "").slice(0, 2);
                      setMinute(digits);
                      setError(null);
                    }}
                  />
                  <button
                    type="button"
                    className="custom-time__toggle"
                    aria-label="Choose time from list"
                    aria-expanded={pickerOpen}
                    disabled={busy}
                    onClick={() => setPickerOpen((v) => !v)}
                  >
                    <Clock size={14} />
                  </button>
                </div>

                {pickerOpen && (
                  <div className="time-popover" role="listbox" aria-label="Time picker">
                    <div className="time-popover__col">
                      <span className="time-popover__head">Hour</span>
                      <div className="time-popover__list">
                        {HOURS.map((h) => {
                          const active = selectedH === h;
                          return (
                            <button
                              key={h}
                              ref={active ? activeHourRef : undefined}
                              type="button"
                              className={`time-popover__item${active ? " is-selected" : ""}`}
                              onClick={() => {
                                setHour(pad2(h));
                                setError(null);
                              }}
                            >
                              {pad2(h)}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                    <div className="time-popover__col">
                      <span className="time-popover__head">Min</span>
                      <div className="time-popover__list">
                        {MINUTE_STEPS.map((m) => {
                          const active = selectedM === m;
                          return (
                            <button
                              key={m}
                              ref={active ? activeMinuteRef : undefined}
                              type="button"
                              className={`time-popover__item${active ? " is-selected" : ""}`}
                              onClick={() => {
                                setMinute(pad2(m));
                                setError(null);
                                setPickerOpen(false);
                              }}
                            >
                              {pad2(m)}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                  </div>
                )}
              </div>
            </div>
          )}

          {error && (
            <p className="custom-dialog__error" role="alert">
              {error}
            </p>
          )}

          <div className="dialog__actions">
            <Button variant="quiet" disabled={busy} onClick={close}>
              Cancel
            </Button>
            <Button variant="primary" type="submit" disabled={busy}>
              {busy ? "Starting…" : "Start"}
            </Button>
          </div>
        </form>
      </div>
    </div>
  );
}
