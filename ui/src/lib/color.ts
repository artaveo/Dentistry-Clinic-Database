// The working UI uses the clinic's own colours (roadmap 2.1b), but a clinic
// may pick any colour — pale yellow, near-black navy… This derives, per theme,
// a button colour, the text colour on it, and a link/active-text colour that
// always meet WCAG AA (4.5:1) on the surfaces they sit on (design-direction §3-4).

type Rgb = [number, number, number];

export function parseHex(hex: string): Rgb {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  const v = m ? m[1] : "0e7490";
  return [0, 2, 4].map((i) => parseInt(v.slice(i, i + 2), 16)) as Rgb;
}

export function toHex([r, g, b]: Rgb): string {
  return "#" + [r, g, b].map((c) => Math.round(Math.min(255, Math.max(0, c))).toString(16).padStart(2, "0")).join("");
}

function channel(c: number) {
  const s = c / 255;
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
}

export function luminance(hex: string): number {
  const [r, g, b] = parseHex(hex);
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

export function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

/** Mixes `hex` toward `target` by `t` (0–1). */
export function mix(hex: string, target: string, t: number): string {
  const a = parseHex(hex);
  const b = parseHex(target);
  return toHex(a.map((c, i) => c + (b[i] - c) * t) as Rgb);
}

/** Smallest step toward `target` that reaches `min` contrast against `against`. */
function untilContrast(hex: string, target: string, against: string, min: number): string {
  for (let t = 0; t <= 1.0001; t += 0.04) {
    const c = mix(hex, target, t);
    if (contrast(c, against) >= min) return c;
  }
  return target;
}

export function rgba(hex: string, alpha: number): string {
  const [r, g, b] = parseHex(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

const WHITE = "#ffffff";
const INK = "#0b1220";
/** Worst-case surfaces text sits on (glass cards over the tinted base). */
export const SURFACE = { light: "#f1f5f9", dark: "#1a2233" } as const;

export type BrandPalette = {
  accent: string;
  accentHover: string;
  accentPressed: string;
  onAccent: string;
  accentText: string;
  accentSoft: string;
  accentRing: string;
};

export function brandPalette(hex: string, theme: "light" | "dark"): BrandPalette {
  const base = toHex(parseHex(hex));
  const surface = SURFACE[theme];
  // Dark theme: the button must stand out from the dark surface (3:1, WCAG 1.4.11).
  let accent = theme === "dark" ? untilContrast(base, WHITE, surface, 3) : base;
  let onAccent = contrast(WHITE, accent) >= contrast(INK, accent) ? WHITE : INK;
  if (contrast(onAccent, accent) < 4.5) {
    accent = untilContrast(accent, onAccent === WHITE ? "#000000" : WHITE, onAccent, 4.5);
  }
  const darker = onAccent === WHITE;
  const accentText =
    theme === "light" ? untilContrast(base, "#000000", surface, 4.5) : untilContrast(base, WHITE, surface, 4.5);
  return {
    accent,
    accentHover: mix(accent, darker ? "#000000" : WHITE, 0.1),
    accentPressed: mix(accent, darker ? "#000000" : WHITE, 0.18),
    onAccent,
    accentText,
    accentSoft: rgba(accentText, theme === "light" ? 0.1 : 0.16),
    accentRing: rgba(accentText, 0.38),
  };
}

/** Writes both themes' brand tokens; styles.css picks the right set per theme. */
export function applyBrand(primary: string, accent: string) {
  const root = document.documentElement.style;
  for (const theme of ["light", "dark"] as const) {
    const p = brandPalette(primary, theme);
    const k = theme === "light" ? "l" : "d";
    root.setProperty(`--brand-${k}-accent`, p.accent);
    root.setProperty(`--brand-${k}-accent-hover`, p.accentHover);
    root.setProperty(`--brand-${k}-accent-pressed`, p.accentPressed);
    root.setProperty(`--brand-${k}-on-accent`, p.onAccent);
    root.setProperty(`--brand-${k}-accent-text`, p.accentText);
    root.setProperty(`--brand-${k}-accent-soft`, p.accentSoft);
    root.setProperty(`--brand-${k}-accent-ring`, p.accentRing);
  }
  root.setProperty("--glow-1", rgba(primary, 1));
  root.setProperty("--glow-2", rgba(accent, 1));
}

const BRAND_KEY = "artaveo.brand";

/** Clinic colours remembered per device so the splash and login already wear them. */
export function rememberBrand(primary: string, accent: string) {
  applyBrand(primary, accent);
  try {
    localStorage.setItem(BRAND_KEY, JSON.stringify([primary, accent]));
  } catch {
    /* per-device convenience only */
  }
}

export function storedBrand(): [string, string] {
  try {
    const v = JSON.parse(localStorage.getItem(BRAND_KEY) ?? "null");
    if (Array.isArray(v) && v.length === 2) return v as [string, string];
  } catch {
    /* fall through */
  }
  return ["#0e7490", "#f59e0b"];
}
