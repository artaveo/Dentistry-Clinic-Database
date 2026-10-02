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

## Processed files (done — Phase 3, OF-014 ✅)

Transparent versions next to `source/` (background connected to the image edges removed via border flood-fill;
the thin white separation lines **inside** the mark, and the wordmark's letter counters, were handled separately —
see `icon_boundary` in the processing script below):

| File | What changed |
|---|---|
| `icon.png` (1254², transparent) | from `source/icon-flat.png`; internal white separator lines kept opaque |
| `icon-small.png` (1254², transparent) | same mark with the separator gap morphologically thickened (dilated ~35px) so it still reads at 16–32px, where the original's thin lines blur into noise — used only for the ≤32px app-icon frames |
| `lockup-light.png` / `lockup-dark.png` (transparent) | from the two `lockup-horizontal-*.png`; wordmark letter counters (the "O", etc.) made transparent too — only the icon-mark portion keeps its deliberate internal lines |
| `mono-black.png` / `mono-white.png` (transparent) | solid single-colour silhouette (its internal gap network turned out fully connected to the border in the source art, so it resolves to a clean solid mark — appropriate for print/mono use) |

`splash-dark.png` and `primary-stacked-light.png` are used **as delivered** (opaque, on their matching panel colour), per the original plan.

App icon set generated from `icon.png` (full detail, ≥48px) and `icon-small.png` (≤32px) into `app/icons/`:
`32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png` (512px), and a hand-built multi-resolution `icon.ico`
(16/24/32 from the small variant, 48/256 from the full-detail one — Pillow's single-source ICO writer can't mix
sources, so the ICO is assembled manually with each frame PNG-embedded at its own resolution).

UI consumption: `ui/src/assets/brand-dental/` (resized for bundle size: icon/mono 512px, lockups 1000px wide,
splash/primary 900px) via `ui/src/ui/Brand.tsx` (`DentalMark`, `DentalLockup`, `DentalPremiumArt`) — app icon,
sidebar footer mark, login screen, splash screen and the About page hero, per roadmap 2.1b. The company mark
(`../artaveo/icon.svg` + `mono-white.svg`) now appears **only** as the small "by Artaveo" credit on the About page.

Rules: never stretch, recolour or add effects; scale proportionally; keep originals in `source/` unchanged.
