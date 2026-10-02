# Artaveo Dental — product brand assets

Product sub-brand of Artaveo ("endorsed brand"): a molar formed by a folded ribbon, in the Artaveo design language
(charcoal `#1A1A1A`, gold `#D4A24C`, the Artaveo geometric wordmark). Approved by the product owner on 2026-10-02.
Where each brand level appears: roadmap section **2.1b**. The parent company brand stays in `../artaveo/`.

## Source files (`source/`, as delivered — opaque backgrounds)

| File | What it is | Use for |
|---|---|---|
| `icon-flat.png` (1254², white bg) | **Flat** two-colour tooth mark | **Source of every app icon**: `.ico`, Tauri icon set, Start menu, taskbar, window title, installer; small mark in the sidebar footer next to the version |
| `lockup-horizontal-light.png` (2171×724, white bg) | Mark + ARTAVEO / DENTAL | Login screen (light theme), installer header |
| `lockup-horizontal-dark.png` (2172×724, black bg) | Same lockup for dark backgrounds (light-grey mark, white text) | Login screen (dark theme) |
| `splash-dark.png` (1254², black bg) | Premium 3D mark + ARTAVEO + "DENTAL CLINIC SOFTWARE" | **Splash screen**; About page in dark theme |
| `primary-stacked-light.png` (1254², white bg) | Premium 3D mark + ARTAVEO + DENTAL, stacked | About page in light theme, website/marketing |
| `mono-black.png` / `mono-white.png` (1254²) | Single-colour mark | Black-and-white printing (tiny credit at the bottom of printed clinic documents), single-colour contexts |

## Processing required (Phase 3, OF-014)

1. Make transparent versions (remove the plain white / black background; keep the thin white separation lines **inside** the mark — only remove background connected to the image edges). Trim to content and add consistent padding. Save next to `source/` as `icon.png`, `lockup-light.png`, `lockup-dark.png`, `mono-black.png`, `mono-white.png`.
2. Generate all app icon sizes from the transparent `icon.png` (`tauri icon`), 16–256 px. Check 16 and 32 px: if the thin separation lines blur into noise, use a simplified small-size variant (fewer/wider gaps) for ≤32 px.
3. Splash and About may use the opaque `splash-dark.png` as-is on a black panel.
4. Replace every Artaveo company logo inside the app per roadmap 2.1b; the company logo appears only as a small "by Artaveo" line on the About page.

Rules: never stretch, recolour or add effects; scale proportionally; keep originals in `source/` unchanged.
