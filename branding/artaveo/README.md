# Artaveo brand assets

Official Artaveo logo files, selected by the product owner's brand folder.
Where the Artaveo brand appears vs. the clinic's own brand: roadmap section **2.1b**.

| File | What it is | Background | Use for |
|---|---|---|---|
| `logo-master.png` | Master logo (3D mark + ARTAVEO + DIGITAL DEVELOPMENT), 1254×1254, transparent | **Dark** | Splash screen, About page in dark theme. The approved reference for colours and shape. |
| `lockup-full-light.png` | Full vertical lockup, 1159×1358 | Light (white, not transparent) | About page in light theme, website, documents about the product |
| `lockup-compact-light.png` | Horizontal mark + ARTAVEO, 2172×724 | Light (white, not transparent) | Login screen and installer header on light backgrounds |
| `lockup-compact-dark.png` | Horizontal mark + ARTAVEO (light text), 2172×724, transparent | **Dark** | Login screen in dark theme |
| `icon.svg` / `icon-1254.png` | **Flat** mark only, transparent | Any | **Source for every app icon** (`.ico`, Tauri icon set, Start menu, taskbar, installer). The flat version stays sharp at 16–32 px; the 3D master does not. |
| `mono-black.svg` / `.png` | Single-colour black mark | Light | Black-and-white printing (small "Artaveo Dental" credit at the bottom of printed documents) |
| `mono-white.svg` / `.png` | Single-colour white mark | Dark | Dark single-colour contexts |

Brand colours taken from the mark: charcoal `#1A1A1A` and gold `#D4A24C`. These are the **Artaveo** colours; the app's
working UI uses the **clinic's** own colours (roadmap 2.1b), so these are for Artaveo-branded surfaces only.

Rules:
- Never stretch, recolour or add effects to the mark; scale proportionally.
- Generate all icon sizes from `icon.svg` (e.g. `npx @tauri-apps/cli icon branding/artaveo/icon-1254.png`), never from the 3D master.
- Originals live in the owner's brand folder (`artaveo/public/brand`); update both places together.
