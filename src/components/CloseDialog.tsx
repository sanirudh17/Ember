/**
 * The inline close confirmation dialog.
 *
 * Pressing Esc always means "keep running", which is the safe answer for a
 * window that may still be holding the PC awake.
 */

import { useEffect } from "react";

import { plural } from "../lib/format";
import { useStore } from "../lib/store";
import { Button } from "./ui";

export function CloseDialog() {
  const { closeRequested, dismissClose, confirmQuit, snapshot } = useStore();

  useEffect(() => {
    if (!closeRequested) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        dismissClose();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [closeRequested, dismissClose]);

  if (!closeRequested) return null;

  const reasons: string[] = [];
  if (snapshot?.manual.active) reasons.push("a manual hold is running");
  if (snapshot && snapshot.agents_working > 0) {
    reasons.push(
      `${plural(snapshot.agents_working, "agent is", "agents are")} working`,
    );
  }
  const held = snapshot?.engaged ?? false;

  return (
    <div className="scrim" role="presentation" onClick={dismissClose}>
      <div
        className="dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="close-title"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="dialog__title" id="close-title">
          {held ? "Quit Ember? It is holding this PC awake." : "Quit Ember?"}
        </h2>
        <p className="dialog__text">
          {held
            ? `Closing the window stops Ember completely${reasons.length > 0 ? ` — ${reasons.join(", ")}` : ""}. This PC will follow its own power plan again.`
            : "Ember will stop watching for agents until you start it again."}
        </p>
        <div className="dialog__actions">
          <Button variant="quiet" onClick={dismissClose}>
            Keep running
          </Button>
          <Button variant="primary" onClick={confirmQuit}>
            Quit Ember
          </Button>
        </div>
      </div>
    </div>
  );
}
