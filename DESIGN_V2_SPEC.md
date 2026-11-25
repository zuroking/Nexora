# Nexora — DESIGN v2 SPEC (approved)

Status: approved 2026-11-24. Concept A chosen by the user; font download
exception granted; derived Light/Sepia/HighContrast palettes approved.
Implemented as `ARCHITECTURE.md` §85.
`DESIGN.md` v1 and `ARCHITECTURE.md` §15/§35 were explicitly overridden by the
user's 2026-11-24 decision and recorded in amendment §85.

## 1. Reference extraction (measured, not guessed)

Source: 11 screenshots in `examples/` (Obsidian, dark theme). Sampled with
Pillow/numpy; values below are exact modal RGB values from the pixel data:

| Role | HEX | Where measured |
|---|---|---|
| rail / sidebar base | `#121212` | rail and sidebar dominant |
| toolbar | `#141414`–`#171717` | top strip |
| active tab / raised | `#2D2D2D` | tab pill |
| main surface | `#222222` | document area |
| tag pill / property accent | `#2D2838` | tag pills, property rows |
| tag pill hover | `#453B5F` | same region |
| H1 / wikilink lavender | `#C9BAFF` | heading glyphs (exact mode) |
| body text | `#E0E0E0` | paragraphs |
| secondary text | `#C8C8C8` | metadata, graph nodes |
| muted accent | `#6A6482` | dim lavender |
| teal callout | `#273535` | callout block |
| blue callout | `#222B39` | note callout |
| blue link | `#135594` | link glyphs |
| graph edge | `#363636`–`#4B4B4B` | graph screenshot |
| warning icon (ref) | `#A8802C` | toolbar icons |
| error icon (ref) | `#A42C2C` | toolbar icons |

Measured WCAG ratios on `#222222`: body 12.05:1, secondary 9.51:1,
lavender 9.05:1, teal-on-teal `#A4D4F4` on `#273535` 8.08:1.
The reference warning `#A8802C` is 4.39:1 and reference error `#A42C2C`
is 2.25:1 — both too low for text; v2 uses `#D8A657` (7.21:1) and
`#E06B6B` (4.91:1) as text colours, keeping the reference hues for icons.

Open, non-blocking: Inter and JetBrains Mono are **not** present anywhere on
this machine (checked `C:\Windows\Fonts`, Program Files, AppData, npm). The
prompt forbids network downloads. Either supply the OFL font files locally or
approve system-stack fallback (Segoe UI / Consolas) before implementation.

## 2. Semantic palette — Dark (lock candidate)

`bg #121212`, `panel #171717`, `surface #222222`, `raised #2D2D2D`,
`tag #2D2838`, `tag_hover #453B5F`, `text #E0E0E0`, `muted #C8C8C8`,
`accent #C9BAFF`, `accent_dim #4A445C`, `info #273535`,
`info_fg #A4D4F4`, `note #222B39`, `link #C9BAFF`,
`warning #D8A657`, `error #E06B6B`, `line #363636`.

The first draft proposed `accent_dim #6A6482` and `link #135594`, but both
failed this spec's own ≥4.5:1 rule (4.23:1 and 2.09:1 on Dark surfaces). The
approved final values are the darker `#4A445C` and lavender `#C9BAFF`;
recorded as a deliberate deviation in `ARCHITECTURE.md` §85.

Other presets are derived from the same role table, preserving hue families:
- **Light** — paper `#F7F7F9`, surface `#FFFFFF`, text `#1F1D26`,
  accent `#5B43A8` (lavender preserved, darkened for contrast),
  info `#DCECEC`, warning `#7A5200`, error `#A32121`.
- **Sepia** — warm paper `#F4EEE3`, surface `#FBF6EC`, text `#2B241B`,
  accent `#6B4FA8`, info `#E3EDE5`, warning `#8A5A00`, error `#9E2E2E`.
- **HighContrast** — `#000000` / `#FFFFFF`, accent `#E4D7FF`,
  info `#B8F0F0`, warning `#FFD166`, error `#FF8080`, no gradients,
  focus ring 3 px.

Every role pair used for text must be ≥4.5:1; a generated contrast report is
committed with the code.

## 3. Typography

- Local Inter (OFL) for UI; JetBrains Mono for code, paths, dates.
- Scale: H1 30 / H2 22 / H3 17 / body 14.5 / small 12 / mono 13 px.
- Headings weight 600, body 400; metadata muted, never bold.
- Cyrillic coverage required for all files supplied.
- Line length in Reading view capped near 78 characters.

## 4. Layout and components

- Icon rail 48 px, left tree 240 px, right context 240 px, toolbar 40 px,
  status 30 px; 12 px inner margin, compact 26 px rows.
- Radius 8 px; soft shadow `0 4 12` only on modals/floating layers.
- Very subtle vertical gradient on the active panel background only.
- Tabs: active `raised` fill + 2 px lavender underline; inactive flat text.
- Buttons: flat fill on hover, 8 px radius, no heavy borders.
- Tag pills and property rows: `tag` background, `accent` text.
- Callouts: teal / blue accent roles with small linear icon and accent label.
  `egui_commonmark` 0.26 has no blockquote fill hook, so the fill cannot be
  painted without vendoring the renderer (forbidden by §6). This is a recorded
  deviation in `ARCHITECTURE.md` §85; labels are checked on `surface`.
- Focus: 2 px `accent` ring (3 px in HC), visible on every interactive row.
- Graph: `#222222` field, `#C8C8C8` nodes, thin `#363636` edges.

## 5. Functional icon rail

Seven linear icons, keyboard-reachable: Today, Health, Graph, Canvas,
Base, Vectors, Capture. Icons drawn with egui `Shape`/`Path`, no icon pack,
no emoji. Tooltips carry the same labels for screen readers.

## 6. Scope boundaries

- Styling only for callouts and property rows: no new renderer features.
- Core logic, NoteId, vault, snapshots, commands, tests unchanged.
- `theme.idx`: new MAGIC `NEXORA-THEME-v2`; v1 is ignored with a single
  surfaced error, default preset = new Dark. No theme marketplace/picker.
- Offline, CPU-only, no secrets, no push. 271 tests are the baseline.

## 7. Concepts (concept A approved)

- **A. Obsidian-faithful (recommended).** Exact measured Dark palette,
  `#C9BAFF` accent, graphite shells, teal/blue callout accents, rail + tree.
- **B. Cool instrument.** Same graphite surfaces but accent shifts to a
  cooler periwinkle `#9FB4FF`; teal callouts remain. Slightly more technical.
- **C. Warm archive.** `#222222` shell with lavender accent but warm
  `#2D2838`→`#342C24` tag surfaces and sepia undertones in Light/Sepia.

## 8. Acceptance gate

After approval: lock §85 in `ARCHITECTURE.md`, implement palette + presets +
shell/views + icons, two-axis review, raw `cargo test --workspace` (271
baseline, 0 failed), `cargo build -p nexora-app`, GUI smoke at 1400×850 and
1920×1080 in all four presets with screenshots, docs updated. No code before
the concept choice.
