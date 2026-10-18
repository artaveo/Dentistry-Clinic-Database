# Artaveo Dental illustrations (originals)

Illustration set for the people, unit and decorative teeth in the app (owner feedback OF-049, OF-050, OF-051;
planned for **v0.6.1**, before phase 6). Made by the product owner with ChatGPT image generation from the
session's prompts (2026-10-17 to 2026-10-19), with a Dribbble patient-dashboard shot as the *style idea only* —
nothing was copied from it. The images belong to the owner (OpenAI terms: outputs are the user's).

These are the **source files**. The app ships optimised copies (trimmed, resized, WebP, circle-cropped avatars);
keep these originals unchanged so the copies can be made again.

| File | Size | Background | Use |
|---|---|---|---|
| `doctor-female.png` | 1024×1536 | Transparent; soft cyan glow (alpha below 64) to drop when optimising | Doctor profile page (female) |
| `doctor-male.png` | 1024×1536 | Same | Doctor profile page (male) |
| `doctor-female-avatar.png` | 1254×1254 | Transparent, painted blue circle with a ragged edge: crop inside the circle | Doctor list, calendar, queue (female) |
| `doctor-male-avatar.png` | 1254×1254 | Same | Same (male) |
| `patient-man.png` | 1254×1254 | Transparent, painted blue circle | Default patient picture: adult man |
| `patient-woman.png` | 1254×1254 | Transparent, no circle; soft glow to drop | Default patient picture: adult woman |
| `patient-boy.png` | 1254×1254 | Transparent, no circle; soft glow to drop | Default patient picture: boy |
| `patient-girl.png` | 1254×1254 | Transparent, painted blue circle | Default patient picture: girl |
| `dental-unit.png` | 1536×1024 | Transparent; soft glow to drop | Units ("یونیت") page |
| `tooth.png` | 1254×1254 | **Solid magenta `#FF00FF`**: key it out (with spill removal) | Decorative tooth: empty states, cards |
| `tooth-implant.png` | 1254×1254 | **Solid magenta `#FF00FF`**: key it out | Decorative implant illustration |

Rules:
- Give all avatars the same circle background when optimising, in light and dark themes.
- The chart's own teeth (`ui/src/chart/`) stay drawn in code: they are a working tool, not decoration.
- A new image must match this set: semi-realistic soft digital painting, cyan rim light, navy `#0F172A`,
  teal `#0E7490`, cyan `#22D3EE`; transparent PNG (or solid magenta for white subjects).
