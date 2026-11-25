# Nexora design — "authored, not generated"

Status: locked with §15 of `ARCHITECTURE.md` on 2026-11-22. First slice
implemented the same day. Verified by screenshot; remaining gaps are marked
unchecked below. Drift in panel widths, toolbar/status heights, typography,
modal borders, and tab/row chrome was corrected on 2026-11-23 without changing
the locked identity; verified in dark, light, and high-contrast presets.

## 0. What went wrong in v1

v1 used egui's default visuals everywhere: blue-grey accent, uniform 4 px
spacing, identical button chrome, no typographic hierarchy, and raw note ids in
the sidebar. It worked, but it reads as a generated demo. The fix is not a
theme picker (out of scope) — it is one carefully tuned look.

## 1. Principles

1. **One authored palette, dark first.** No gradients, no glassmorphism, no
   neon, no emoji as decoration. Warm neutrals with a single restrained accent.
2. **Type does the hierarchy.** Size and weight separate headings, body, and
   metadata; colour is a secondary signal only.
3. **Space is a feature.** Side panels breathe; controls are not packed.
4. **Chrome recedes.** Frames are quiet 1 px lines; only the active tab and the
   primary action get a filled background.
5. **Real labels.** "Untitled-2" and "Context" are not enough. Show note titles
   with path hints, and name panels after what they contain: Links, Backlinks,
   History.
6. **No fake affordances.** Every visible button does something. Disabled or
   decorative controls are not drawn.

## 2. Palette (dark, default)

| Role | Hex | Usage |
|---|---|---|
| Ink | `#191817` | window background |
| Panel | `#201F1D` | side panels, toolbar, status bar |
| Surface | `#272523` | editor / reading canvas |
| Raised | `#302D29` | hovered controls, active tab |
| Line | `#3A3733` | separators, panel borders |
| Text | `#E8E3DA` | primary text |
| Muted | `#A9A196` | secondary text, paths, timestamps |
| Accent | `#C98B6B` | active tab underline, links, focus ring |
| Accent dim | `#7A5543` | quiet accent backgrounds |
| Sage | `#8FA98A` | saved / ok |
| Amber | `#D8A657` | warnings, snapshots |
| Rust | `#D06B5B` | errors, broken links |

High-contrast and light presets are v1.2+, not v1.1. Do not ship a colour
picker.

## 3. Typography

- Proportional: system UI stack (Segoe UI / system default), body 14.5 px,
  heading H1 22 px, H2 18 px, H3 16 px, small 12 px.
- Monospace: Consolas / system monospace, 13 px, used for code and note paths.
- Line length in Reading view is capped near 78 characters by centring the
  content column inside the editor rect.
- Metadata (paths, timestamps, snapshot names) is 12 px muted, never bold.

## 4. Layout

```
┌────────────────────────────────────────────────────────────────────┐
│ NEXORA   [search............]  Today  Health  Graph  Capture  ⌘P  │  toolbar, 44 px
├───────────────┬──────────────────────────────────────┬─────────────┤
│ NOTES         │  ● Project    Draft ×    Graph ×     │ LINKS       │
│  All notes    │ ─────────────                        │ Backlinks   │
│ #school #idea │                                      │ Tags        │
│               │      [ Source | Reading ]            │ History     │
│ Title         │                                      │             │
│ path/to/note  │      rendered / raw markdown         │             │
├───────────────┴──────────────────────────────────────┴─────────────┤
│ Saved 03:14 · 7 snapshots · 412 words        vault: my-first-vault │  status
└────────────────────────────────────────────────────────────────────┘
```

- Panels: left 232 px, right 236 px, both resizable, 12 px inner margin.
- Toolbar and status bar are 44 px / 30 px, no heavy borders.
- Tabs: active tab gets accent underline and Raised fill; inactive tabs are
  flat text with a quiet `×` on hover.
- View switch is a small segmented control inside the editor header, not a
  hidden menu. `Ctrl+E` toggles Source/Reading.

## 5. Reading view

- Rendered through `egui_commonmark` with network and image features disabled.
- Obsidian-flavoured source is adapted *for display only*, never rewritten on
  disk: `[[target|alias]]` becomes a styled link label, `[[target]]` likewise;
  `==highlight==` becomes muted emphasis; `> [!note]` callouts become a quote
  with a bold kind label.
- Broken links are shown in Rust red with a dashed underline and are still
  clickable (opens the "create note" command).
- Ctrl+Click on a rendered wikilink follows the same navigation path as the
  Source-mode link handler.
- Autosave stays active in both modes; Reading view is never editable.

## 6. Acceptance checklist for the redesign

- [x] No default egui blue remains in panels, tabs, or selection.
- [x] Named text styles are configured (H1 22 / body 14.5 / button 13.5 / small 12 / mono 13).
- [ ] §3's exact H2 18 / H3 16 are not reachable with `egui_commonmark` 0.26:
      it interpolates heading sizes from H1(22)/Body(14.5), giving ≈20.8/19.5 px
      and exposes no per-level style hook. Exact values need a renderer patch.
- [x] Sidebar rows show title + muted path, not just a filename.
- [x] Source/Reading toggle works with `Ctrl+E` and with the mouse.
- [x] Reading view renders lists, task lists, quotes, code, and wikilinks.
- [ ] Heading visual weight still needs tuning: the renderer keeps a visible
      `#` marker before H1. Cosmetic, tracked for v1.1 polish.
- [ ] Broken links are marked `(missing)` in text and navigable; red dashed
      underline is not implemented because the renderer has no per-link hook.
- [x] Status bar shows save state, snapshot count, and word count.
- [x] No network request is made by the renderer; all `OpenUrl` commands are
      intercepted, `nexora://` navigates internally, external URLs are blocked.

## 7. Explicit non-goals for this pass

No theme editor, no icon pack, no custom font binary, no rendered images, no
math, no Live Preview, no mobile layout. These are roadmap items, not omissions
silently smuggled into scope.
