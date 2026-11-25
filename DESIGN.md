# Nexora design — v2 "Obsidian-faithful"

Status: locked 2026-11-24 by `ARCHITECTURE.md` §85. This file replaces v1;
the complete previous text is preserved in `DESIGN_v1_ARCHIVE.md`. The change
is an explicit user override of the v1 identity lock (§15/§35) and of the
design-director's earlier REJECT verdict — not an agent approval.

Measured source of every colour: the 11 screenshots in `examples/`
(Pillow/numpy pixel sampling). Full role table and contrast numbers:
`DESIGN_V2_SPEC.md`.

## 1. Principles

1. **One authored identity.** Four presets share the same semantic roles;
   no picker marketplace, no per-note theme, no runtime network.
2. **Workspace, not dashboard.** A narrow icon rail and a note tree carry
   navigation; the document area is the protagonist.
3. **Type and space do the hierarchy.** Lavender is reserved for headings,
   wikilinks and the active state; status colours stay semantic.
4. **Quiet chrome.** 1 px lines, 8 px corners, shadows only on floating
   layers. A faint gradient is allowed on the active panel only.

## 2. Palette (Dark roles, exact sampled values)

| Role | HEX | Use |
|---|---|---|
| bg | `#121212` | rail, deepest shell |
| panel | `#171717` | toolbars, sidebars |
| surface | `#222222` | document and table area |
| raised | `#2D2D2D` | active tab, buttons |
| tag | `#2D2838` | tag pills, property rows |
| tag_hover | `#453B5F` | hovered tag |
| text | `#E0E0E0` | body |
| muted | `#C8C8C8` | metadata, paths |
| accent | `#C9BAFF` | H1, wikilinks, active control |
| accent_dim | `#4A445C` | selected/active fill |
| info | `#273535` | teal callout accent role (fill unavailable in 0.26) |
| info_fg | `#A4D4F4` | callout label/icon on `surface` |
| note | `#222B39` | blue note accent role |
| link | `#C9BAFF` | internal and external links |
| sage / amber / rust | `#8FA98A` / `#D8A657` / `#E06B6B` | ok / warning / error |
| line | `#363636` | quiet borders |

Presets: Dark, Light (`#F7F7F9` / `#FFFFFF`, accent `#5B43A8`),
Sepia (`#F4EEE3` / `#FBF6EC`, accent `#6B4FA8`), HighContrast
(`#000000` / `#0A0A0A`, accent `#E4D7FF`). Derived values were approved by
the user; every text role measures ≥4.5:1 against the surfaces it is drawn on.
`theme.idx` uses `NEXORA-THEME-v2`; a v1 sidecar surfaces one error and falls
back to Dark.

## 3. Typography

- Embedded OFL fonts: Inter (proportional), JetBrains Mono (monospace).
- Scale: H1 30 / body 14.5 / button 13.5 / small 12 / mono 13 px.
- H1 weight 600; metadata muted, never bold. Cyrillic is covered by Inter.
- Reading view content column capped near 78 characters.

## 4. Layout

- Functional icon rail 48 px; note tree 240 px; context panel 240 px;
  toolbar 40 px; status 30 px; 12 px inner margins; 26 px compact rows.
- Tabs: active gets `raised` fill and a 2 px lavender underline; inactive
  are flat labels with a hover-only close target named "× <title>".
- Controls: 8 px corner radius, 1 px quiet strokes; `0 4 12` shadow on
  modals/floating layers only; faint vertical gradient on the active panel.
- Focus ring: 2 px accent (3 px in HighContrast) on every interactive row.

## 5. Icon rail

Seven hand-drawn linear icons (no icon pack, no emoji): Today, Health,
Graph, Canvas, Base, Vectors, Capture. Each is a real button with a tooltip
and a keyboard-accessible click target; the active view gets the accent
colour. Source-mode Tab keeps egui's native focus behaviour.

## 6. Reading view and callouts

- Rendered by `egui_commonmark` with network and image features disabled.
  External URLs are intercepted and blocked; `nexora://` navigates internally.
- Callouts use the GFM alert hooks with letter icons (`i + * ! x`) and
  palette accents; nothing changes in source Markdown and no new renderer
  feature was introduced. `egui_commonmark` 0.26 exposes no blockquote fill
  hook, so the measured `info`/`note` roles colour the accent rule and
  foreground instead of a block background (deviation in §85).
- Known renderer limitation carried from v1: `egui_commonmark` 0.26 keeps a
  visible `#` before H1 and interpolates H2/H3 from H1/body. No per-level
  style hook exists; fixing it needs a renderer patch (deferred).
- Autosave stays active in Reading view; the view itself is not editable.

## 7. Non-goals for v2

Theme editor/marketplace, icon-pack dependency, custom renderer features,
math/Mermaid, Live Preview, mobile layout, network access at runtime.

## 8. Acceptance gate

Every identity change requires: raw `cargo test --workspace` output in chat
(271-test baseline, 0 failed), `cargo build -p nexora-app`, GUI smoke at
1400×850 and 1920×1080 in all four presets, two-axis code review, and
updates to ARCHITECTURE.md / CHECKLIST.md / README.md / ROADMAP.md.
