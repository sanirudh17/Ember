/**
 * One shared snapshot for the whole app.
 *
 * The store owns three things the spec cares about: the loading state before
 * the first snapshot lands, the error state when the bridge fails, and an
 * optimistic settings override so a toggle flips the instant it is clicked
 * instead of a quarter of a second later.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";

import { api, onCloseRequested, onStateChanged, onUpdateProgress } from "./api";
import type { Settings, Snapshot, UpdateInfo } from "../types";

interface Store {
  snapshot: Snapshot | null;
  settings: Settings | null;
  loading: boolean;
  error: string | null;
  closeRequested: boolean;
  reload: () => void;
  /** Merge a partial settings change and persist the whole document. */
  apply: (patch: Partial<Settings>) => void;
  /** Apply a full Settings object returned by the backend. */
  applySettings: (next: Settings) => void;
  /** Robustly toggle master watching with immediate optimistic state. */
  setWatching: (enabled: boolean) => Promise<void>;
  /** The newer release GitHub is offering, once a check found one. */
  update: UpdateInfo | null;
  updateChecking: boolean;
  updateError: string | null;
  /** 0–100 while an update downloads, null otherwise. */
  updateProgress: number | null;
  checkUpdate: () => Promise<void>;
  installUpdate: () => Promise<void>;
  dismissClose: () => void;
  /** Signal that the user really does want to quit. */
  confirmQuit: () => void;
}

const StoreContext = createContext<Store | null>(null);

export function useStore(): Store {
  const store = useContext(StoreContext);
  if (!store) throw new Error("useStore must be used inside <StoreProvider>");
  return store;
}

/** `settings` is guaranteed non-null inside the ready UI. */
export function useReady(): { snapshot: Snapshot; settings: Settings } {
  const { snapshot, settings } = useStore();
  if (!snapshot || !settings) throw new Error("the UI is not ready yet");
  return { snapshot, settings };
}

export function StoreProvider({ children }: { children: ReactNode }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [override, setOverride] = useState<Settings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [closeRequested, setCloseRequested] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [updateChecking, setUpdateChecking] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [updateProgress, setUpdateProgress] = useState<number | null>(null);

  const pendingVersion = useRef(0);
  const overrideRef = useRef<Settings | null>(null);
  overrideRef.current = override;

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;

    setLoading(true);
    setError(null);

    api
      .snapshot()
      .then((first) => {
        if (disposed) return;
        setSnapshot(first);
        setLoading(false);
        void onStateChanged((next) => {
          if (disposed) return;
          setSnapshot((prev) => {
            if (!next) return prev;
            if (pendingVersion.current > 0 && overrideRef.current) {
              return {
                ...next,
                watching: overrideRef.current.watching,
                settings: {
                  ...next.settings,
                  ...overrideRef.current,
                },
              };
            }
            return next;
          });
        }).then((off) => {
          if (disposed) off();
          else unsubscribe = off;
        });
      })
      .catch((e: unknown) => {
        if (disposed) return;
        setError(e instanceof Error ? e.message : String(e));
        setLoading(false);
      });

    void onCloseRequested(() => setCloseRequested(true)).then((off) => {
      if (disposed) off();
    });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, [attempt]);

  const reload = useCallback(() => setAttempt((n) => n + 1), []);

  const checkUpdate = useCallback(async () => {
    setUpdateChecking(true);
    try {
      const found = await api.checkUpdate();
      setUpdate(found);
      setUpdateError(null);
    } catch (e: unknown) {
      setUpdateError(e instanceof Error ? e.message : String(e));
    } finally {
      setUpdateChecking(false);
    }
  }, []);

  const installUpdate = useCallback(async () => {
    setUpdateError(null);
    setUpdateProgress(0);
    try {
      await api.installUpdate();
      // On success the installer takes over and Ember restarts; reaching this
      // line means the command returned without restarting.
      setUpdateProgress(null);
    } catch (e: unknown) {
      setUpdateError(e instanceof Error ? e.message : String(e));
      setUpdateProgress(null);
    }
  }, []);

  // Every launch checks GitHub once as soon as the app is up, and every
  // four hours after that. Failures are stored, never thrown, and nothing
  // blocks on it.
  useEffect(() => {
    void checkUpdate();
    const repeat = window.setInterval(() => void checkUpdate(), 4 * 60 * 60 * 1000);
    return () => window.clearInterval(repeat);
  }, [checkUpdate]);

  // Download progress for an installer, relayed from the updater plugin.
  useEffect(() => {
    let disposed = false;
    let off: (() => void) | undefined;
    void onUpdateProgress((progress) => {
      if (disposed) return;
      setUpdateProgress(
        progress.total && progress.total > 0
          ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
          : null,
      );
    }).then((unsubscribe) => {
      if (disposed) unsubscribe();
      else off = unsubscribe;
    });
    return () => {
      disposed = true;
      off?.();
    };
  }, []);

  const applySettings = useCallback((next: Settings) => {
    pendingVersion.current++;
    setOverride(next);
    setSnapshot((prev) => (prev ? { ...prev, settings: next, watching: next.watching } : prev));
  }, []);

  const apply = useCallback(
    (patch: Partial<Settings>) => {
      const base = overrideRef.current ?? snapshot?.settings;
      if (!base) return;
      const next: Settings = { ...base, ...patch };
      const currentVer = ++pendingVersion.current;
      setOverride(next);
      setSnapshot((prev) => (prev ? { ...prev, settings: next, watching: next.watching } : prev));
      api
        .updateSettings(next)
        .then((saved) => {
          if (pendingVersion.current === currentVer) {
            pendingVersion.current = 0;
            setOverride(null);
            setSnapshot((prev) =>
              prev ? { ...prev, settings: saved, watching: saved.watching } : prev,
            );
          }
        })
        .catch((e: unknown) => {
          if (pendingVersion.current === currentVer) {
            pendingVersion.current = 0;
            setOverride(null);
            // The engine kept its previous values (the write failed); put the
            // pre-edit copy back instead of leaving the failed edit painted on.
            setSnapshot((prev) =>
              prev ? { ...prev, settings: base, watching: base.watching } : prev,
            );
          }
          setError(e instanceof Error ? e.message : String(e));
        });
    },
    [snapshot],
  );

  const setWatching = useCallback(
    async (enabled: boolean) => {
      const base = overrideRef.current ?? snapshot?.settings;
      if (!base) return;
      const next: Settings = { ...base, watching: enabled };
      const currentVer = ++pendingVersion.current;
      setOverride(next);
      setSnapshot((prev) => (prev ? { ...prev, settings: next, watching: enabled } : prev));
      try {
        const saved = await api.setWatching(enabled);
        if (pendingVersion.current === currentVer) {
          pendingVersion.current = 0;
          setOverride(null);
          setSnapshot((prev) =>
            prev ? { ...prev, settings: saved, watching: saved.watching } : prev,
          );
        }
      } catch (e: unknown) {
        if (pendingVersion.current === currentVer) {
          pendingVersion.current = 0;
          setOverride(null);
          setSnapshot((prev) =>
            prev ? { ...prev, settings: base, watching: base.watching } : prev,
          );
        }
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [snapshot],
  );

  const confirmQuit = useCallback(() => {
    void api.markClosing().then(() => api.windowControl("close"));
  }, []);

  const value: Store = {
    snapshot,
    settings: override ?? snapshot?.settings ?? null,
    loading,
    error,
    closeRequested,
    reload,
    apply,
    applySettings,
    setWatching,
    update,
    updateChecking,
    updateError,
    updateProgress,
    checkUpdate,
    installUpdate,
    dismissClose: () => setCloseRequested(false),
    confirmQuit,
  };

  return <StoreContext.Provider value={value}>{children}</StoreContext.Provider>;
}
