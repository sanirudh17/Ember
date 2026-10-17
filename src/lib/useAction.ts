/** Run an async action with a busy flag and a readable error, for any page. */

import { useCallback, useState } from "react";

export interface ActionState {
  busy: boolean;
  error: string | null;
  message: string | null;
  clearError: () => void;
  setMessage: (msg: string | null) => void;
  run: (action: () => Promise<unknown>) => void;
}

export function useAction(): ActionState {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback((action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    void action()
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setBusy(false));
  }, []);

  const clearError = useCallback(() => setError(null), []);

  return {
    busy,
    error,
    message: error,
    clearError,
    setMessage: setError,
    run,
  };
}
