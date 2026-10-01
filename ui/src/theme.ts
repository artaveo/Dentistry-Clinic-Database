// Manual theme + performance mode (roadmap 2.1). "system" follows the OS
// (`prefers-color-scheme`, handled in styles.css); light/dark are explicit
// choices persisted per device. Performance mode disables glass/blur effects
// for weak computers; it defaults from `prefers-reduced-transparency`.
import { createContext, useContext } from "react";

export type ThemeChoice = "light" | "dark" | "system";
export type PerfMode = "full" | "reduced";

const THEME_KEY = "artaveo.theme";
const PERF_KEY = "artaveo.perf";

export function storedTheme(): ThemeChoice {
  try {
    const v = localStorage.getItem(THEME_KEY);
    return v === "light" || v === "dark" || v === "system" ? v : "system";
  } catch {
    return "system";
  }
}

export function storedPerf(): PerfMode {
  try {
    const v = localStorage.getItem(PERF_KEY);
    if (v === "full" || v === "reduced") return v;
  } catch {
    /* fall through to the media-query default */
  }
  try {
    return window.matchMedia("(prefers-reduced-transparency: reduce)").matches ? "reduced" : "full";
  } catch {
    return "full";
  }
}

export function saveTheme(t: ThemeChoice) {
  try {
    localStorage.setItem(THEME_KEY, t);
  } catch {
    /* per-device convenience only */
  }
}

export function savePerf(p: PerfMode) {
  try {
    localStorage.setItem(PERF_KEY, p);
  } catch {
    /* per-device convenience only */
  }
}

export function applyTheme(t: ThemeChoice) {
  if (t === "system") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = t;
}

export function applyPerf(p: PerfMode) {
  document.documentElement.dataset.perf = p;
}

export const ThemeContext = createContext<{
  theme: ThemeChoice;
  setTheme: (t: ThemeChoice) => void;
  perf: PerfMode;
  setPerf: (p: PerfMode) => void;
}>({
  theme: "system",
  setTheme: () => {},
  perf: "full",
  setPerf: () => {},
});

export function useTheme() {
  return useContext(ThemeContext);
}
