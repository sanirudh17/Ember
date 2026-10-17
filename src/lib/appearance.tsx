/**
 * Theme resolution, and nothing else.
 *
 * The material/backdrop dance is gone — Ember always paints a solid ground —
 * so this provider resolves "system" against the OS and writes the result on
 * <html> for the stylesheet to pick up. Everything else is plain CSS.
 */

import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";

import { useStore } from "./store";
import type { Theme } from "../types";

export function resolveTheme(theme: Theme): "light" | "dark" {
  if (theme === "light" || theme === "dark") return theme;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

interface AppearanceValue {
  resolvedTheme: "light" | "dark";
}

const AppearanceContext = createContext<AppearanceValue | null>(null);

export function useAppearance(): AppearanceValue {
  const ctx = useContext(AppearanceContext);
  if (!ctx) throw new Error("useAppearance must be used within AppearanceProvider");
  return ctx;
}

export function AppearanceProvider({ children }: { children: ReactNode }) {
  const { settings } = useStore();
  const theme = settings?.theme ?? "system";
  const [resolved, setResolved] = useState<"light" | "dark">(() => resolveTheme(theme));

  useEffect(() => {
    setResolved(resolveTheme(theme));
    if (theme !== "system") return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setResolved(media.matches ? "dark" : "light");
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [theme]);

  useEffect(() => {
    document.documentElement.dataset.theme = resolved;
  }, [resolved]);

  return (
    <AppearanceContext.Provider value={{ resolvedTheme: resolved }}>
      {children}
    </AppearanceContext.Provider>
  );
}
