/** The two states every screen must survive: loading, and a broken bridge. */

import { type CSSProperties } from "react";

import { Button } from "./ui";

export function PageSkeleton() {
  return (
    <div className="page" aria-busy="true" aria-label="Loading">
      <div className="skel skel--state" />
      <div className="skel skel--line" />
      <div className="skel skel--block" />
    </div>
  );
}

/** With the engine unreachable the instrument still renders — with no signal. */
const DARK_READINGS = [
  { k: "Held for", v: "—" },
  { k: "Agents", v: "—" },
  { k: "Battery", v: "—" },
  { k: "Lid hold", v: "—" },
];

export function PageError({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className="page" role="alert">
      <section className="state col">
        <h1 className="state__line">
          <span className="line">
            <span style={{ "--i": 0 } as CSSProperties}>Ember cannot reach its engine.</span>
          </span>
        </h1>
        <p className="state__sub" data-rise style={{ "--i": 1 } as CSSProperties}>
          The window and the Rust process lost their bridge. Retrying usually fixes it; if it
          keeps happening, restart Ember.
        </p>
        {message && (
          <pre className="code" data-rise style={{ "--i": 2 } as CSSProperties}>
            {message}
          </pre>
        )}
        <div className="actions" data-rise style={{ "--i": 3 } as CSSProperties}>
          <Button variant="primary" onClick={onRetry}>
            Try again
          </Button>
        </div>
      </section>

      <section className="section band" data-reveal>
        <div className="field wide" data-ui>
          <div className="field__grid">
            {DARK_READINGS.map((row, index) => (
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
    </div>
  );
}
