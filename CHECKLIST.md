# Nexora — progress checklist

Status as of 2026-11-25. `[x]` means: architecture amendment locked before
code, implemented, raw `cargo test` pasted, GUI smoke described, code review
run, findings fixed, and docs updated. `[ ]` means planned or deferred.
Current test count: **279 passing** (measured from raw `cargo test --workspace`,
2026-11-25). Published under MIT at `https://github.com/zuroking/Nexora`.

## Foundation

- [x] M0 — `Nexora/ARCHITECTURE.md` created and locked; root deviation logged
- [x] M1 — core model, vault discovery, path identity, autosave, 10 snapshots/note
- [x] M2 — wikilinks, backlinks, tags, search, daily notes, inbox, health
- [x] M3 — deterministic graph layout
- [x] M4 — GUI shell: egui/eframe, panels, tabs, autosave wiring
- [x] M5 — full v1 UI: palette, search, daily/tags, graph, health UI
- [x] M6 — final test + GUI smoke
- [x] `AGENTS.md` with commands, invariants, egui 0.37 gotchas, verification workflow
- [x] AI-generated Obsidian overview stored in repo root and marked unverified
- [x] `RESEARCH.md`, `ROADMAP.md` goals G1–G9, `README.md`, this checklist

## v1.1 (amendments §15–§24) — complete

- [x] V1.1-0 — Reading view + authored dark visual preset, `Ctrl+E` (§15)
- [x] V1.1-1 — external-change detection, 1 Hz poll, dirty conflict banner (§16)
- [x] Zed/VS Code style folder-tree explorer with collapse/expand (§17)
- [x] V1.1-2 — F2 rename with dry-run preview and exact wikilink rewriting (§18)
- [x] V1.1-4 — snapshot timeline with hand-rolled line diff and restore (§19)
- [x] V1.1-5 — actionable health report with line jump (§20)
- [x] V1.1-6 — fuzzy quick switcher (`Ctrl+O`) + go-to-heading (`Ctrl+Shift+O`) (§21)
- [x] V1.1-7 — Properties/frontmatter modal, BOM and CRLF preserved (§22)
- [x] V1.1-3 — inline `[[` autocomplete at cursor, Tab/Enter to insert (§23)
- [x] V1.1-8 — relative last-saved time in status bar (§24)

## v2 part A and G2/G3 — done so far

- [x] V2-1 part A — dependency-free lexical vector index, Keyword/Semantic/Hybrid search, sidecar in app data (§25, DD-N5)
- [x] V2-2 slice 0 — related notes with visible lexical/tag/graph factors (§26)
- [x] V2-5 — deterministic canvas: cards, wikilink edges, drag, reset, sidecar (§27)
- [x] V2-6 — nested tags (`#school/math` + parent `school`) and frontmatter aliases (§28)
- [x] G2 — Bases-lite: table/card views, tag filter, sort, group-by; guarded
      single-cell edits added in §56
- [x] G3 — unlinked mentions with line numbers and jump (§30)
- [x] G3 — `![[note]]` embeds in Reading view, depth 3 + visited set (§31)
- [x] G3 — page preview tooltips on note rows in Explorer/Context/Base/Quick Open (§32)
- [x] G3 — bookmarks: app-data sidecar, toggle command, Explorer section, rename/prune hooks (§33)
- [x] G3 — outgoing links panel with line numbers and broken-link marking (§34)
- [x] V2-8 — authored theme presets with WCAG contrast, app-data sidecar and
      palette cycle (§35); extended to four presets in DESIGN v2 (§85)
- [x] V2-7 — templates with closed `{{date}}`/`{{title}}` placeholders, app-data sidecar, palette command (§36)
- [x] V2-9 — daily note auto-applies the template on first creation, never rewrites (§37)
- [x] V2-10 — heading/block-scoped wikilinks, anchor navigation, rename preservation (§38)
- [x] V2-11 — boolean/field search (`Query` mode), test-first core parser (§39)
- [x] V2-12 — Note Composer: extract lines with auto-link, merge with link rewrite (§40)
- [x] V2-13 — saved queries over the `Query` parser, source text only, palette save/apply (§41)
- [x] G5 — saved workspaces per vault, tab descriptors only, palette save/apply (§42)
- [x] G3 — block-level embeds `![[note#heading]]` / `![[note#^id]]`, visible bad-anchor marker (§43)
- [x] G6 — offline CLI (`search`, `query`, `rename`, `daily:path`) as a thin core wrapper (§44)
- [x] G4 — local image embeds `![[pic.png]]` via `bytes://`, safe paths, missing marker (§45)
- [x] V3-3 — SM-2 resurface queue, flat review frontmatter, palette review/grade (§46)
- [x] BOM before H1 fixed in title extraction; aliases resolved in Reading view

## UI polish — locked identity (2026-11-23, historical)

No new amendment: restored the v1 DESIGN values that had drifted, with no
palette, font, icon, or component-identity change. `design-director` approved
that scope. At the time: 161 passed, 0 failed; smoke in three presets.
**Superseded by §85 (DESIGN v2)**: four presets, panels 240/240 px and
toolbar 40 px; the status bar remains 30 px with error-aware colour.

- [x] Panel widths restored (v1: 232/236 px; §85: 240/240 px)
- [x] Toolbar height (v1: 44 px; §85: 40 px); status bar 30 px, single line
- [ ] Exact H2 18 / H3 16 blocked: `egui_commonmark` interpolates from H1/Body
      (≈20.8/19.5 px) and has no per-level style hook; dead named styles removed
- [x] Modal windows unified with quiet 1 px `Line` border (§1/§4)
- [x] Active tab: `Raised` fill + 2 px `Accent` underline, quiet `×` on hover (§4)
- [x] Note rows: title + 12 px muted path in Explorer and sidebar (§1.5/§3)
- [x] Empty states: vault/tag filter, context sections, search query, command palette
- [x] Command palette keyboard navigation (↑/↓/Enter) and explicit Cancel
- [x] Quick capture explicit Cancel; malformed boolean query shown in amber

## Hardening 2026-11-23

- [x] §47 — Apply workspace flushes dirty tabs before rebuild; a failed flush
      aborts and keeps the tabs; conflict buttons target the conflicting note
- [x] §48 — Save query, Save workspace, Note Composer and Apply query keep the
      window open on failure and show the reason inline; typed input preserved
- [x] §49 — Health broken-link rows get a `create` button reusing the existing
      CreateLinkTarget command; nested targets create parent folders (test added)
- [x] §50 — `[[` autocomplete shows "No matching notes" instead of nothing;
      keyboard handling safe on an empty list
- [x] §51 — empty review queue explains how to set `review_due` frontmatter
- [x] §52 — workspace active index clamps to surviving tabs (regression test)
- [x] §53 — grading refreshes the open tab; dirty tabs refuse; partial and
      hostile review frontmatter can no longer be silently overwritten or panic
- [x] §54 — `restore_snapshot` returns the bytes it wrote; properties and
      restore refresh tabs through the shared helper; all guarded writes share
      GM-4 conflict recovery; restore checks every tab for unsaved buffers;
      byte-exact restore covered for BOM/CRLF/UTF-8
- [x] §55 — plain-text list mechanics: Enter continuation/exit, Tab/Shift+Tab
      indentation, Ctrl+Enter task toggle; 17 core cases cover BOM/fences/CRLF
- [x] §43 regression — depth-3 bound is covered for anchored embeds too
- [x] §56 — Base table single-cell frontmatter edits: created/tags/aliases/group,
      guarded write with snapshot, dirty-tab refusal, stable per-row/column
      editor ids, 9 core cases for lossless edits and grouping
- [x] §57 — plain-text pipe-table mechanics: Enter row continuation/exit,
      Tab/Shift+Tab cell navigation, escaped pipes, per-cell padding, BOM/CRLF;
      16 core cases and GUI smoke with autosave verification
- [x] §58 — custom Base columns: validated column keys, scalar frontmatter
      projection, backward-compatible sidecar, chips and dynamic table/card
      columns; 13 core cases and GUI smoke with snapshot verification
- [x] §59 — sort Base by frontmatter field: `field` mode, empty-last ordering,
      id tiebreak, title fallback, key normalisation and 6-line sidecar;
      5 core cases and GUI smoke including invalid-key recovery
- [x] §60 — Base sort direction: asc/desc toggle, stable id tiebreak, missing
      values last in both directions, 7-line sidecar; 4 core cases and GUI smoke
- [x] §61 — blockquote mechanics: exact marker continuation, nested markers,
      empty-line exit, caret split, BOM/CRLF; 9 core cases and GUI smoke
- [x] §62 — Base text filter: case-insensitive match over title/path/values,
      8-line sidecar; 4 core cases and GUI smoke reducing 15 rows to 1
- [x] §63 — Base filter recovery: has/clear filters in core, conditional reset
      control and empty-state explanation, settings preserved; 2 core cases and
      GUI smoke restoring 15 rows from both reset paths
- [x] §64 — Search result keyboard navigation: ↑/↓ selection, Enter to open,
      signature reset, modal guard including snapshot restore; GUI smoke with
      8 results, two arrows → Card, Enter opened it, palette unaffected
- [x] §65 — Saved query management: core rename/remove with validation, inline
      rename and confirmed delete in the manager window, error reset; 3 core
      cases and GUI smoke with sidecar verification
- [x] §66 — Workspace management: scoped core rename/remove, inline rename and
      confirmed delete, manager stays open on failure, success clears error;
      2 core cases and GUI smoke with sidecar verification
- [x] §67 — Fuzzy command palette: shared fuzzy scorer over labels and shortcuts,
      deterministic registry-order ties, selection reset on query change; 3 core
      cases and GUI smoke with Enter running the highlighted command
- [x] §68 — Explorer keyboard navigation: ↑/↓ over visible note rows only,
      focus-lock filter, destination-row scroll, Enter opens the focused note;
      GUI smoke with two arrows and Enter
- [x] §69 — Review queue keyboard navigation: ↑/↓ selection with wrap and
      scroll, Enter opens the due note; GUI smoke with three due notes
- [x] §70 — Template management: core save/upsert/rename/remove with daily
      protection, byte-length sidecar, GUI editor and confirmed delete; 2 core
      cases and GUI smoke with create/rename/delete
- [x] §71 — Bookmark manager: pure remove/move operations, palette command,
      window with Open/↑/↓/Remove and inline errors; core cases, registry count
      and GUI smoke with sidecar verification
- [x] §72 — Health report keyboard navigation: core selection key, ↑/↓ with
      scroll and clamp, Enter to open, modal guard; 6 core cases and GUI smoke
- [x] §73 — Cyclic tab navigation: core wrap helper, Ctrl+Tab/Ctrl+Shift+Tab,
      modal guard; 5 core cases and GUI smoke across three tabs
- [x] §74 — Named Base views: core TSV sidecar and manager with
      save/apply/rename/delete, guarded writes and legacy base.idx format;
      4 core cases and full GUI smoke
- [x] §75 — Insert template into current note: pure char-indexed block insert,
      Source-mode picker, guarded write and caret restore; 4 core cases and GUI smoke
- [x] §76 — Canvas keyboard navigation: core selection key, ←/↑/→/↓ with wrap,
      Enter to open, accent stroke; core case and GUI smoke
- [x] §77 — Graph keyboard navigation: core selection key, ←/↑/→/↓ with wrap,
      Enter to open, accent ring, click-to-open; 2 core cases and GUI smoke
- [x] §78 — Composer write parity: extract new-note-first ordering, merge
      change list, dirty guards, tab refresh and grade inline errors;
      6 core cases incl. forced-conflict retry and GUI smoke
- [x] §79 — Safe tab close and best-effort snapshot pruning: dirty flush
      abort keeps GM-4 recovery, locked snapshots cannot block writes;
      1 core case and GUI smoke
- [x] §80 — Base table keyboard navigation: clamped grid helper, selection
      key, Raised fill, Enter-to-edit; core case and GUI smoke
- [x] §81 — guarded write parity: Properties dirty refusal + loaded anchor,
      clone-save-commit for query/workspace, template picker stays open on
      failure; GUI smoke with inline rust reason and no false conflict
- [x] §82 — Saved-query manager keyboard navigation: core selection key,
      Up/Down wrap, Enter applies, rename-editor focus guard; 2 core cases and
      GUI smoke with three queries
- [x] §83 — Audit fixes: all sidecar loads surface errors; global shortcuts
      suppressed while a modal owns the keyboard; workspace descriptors
      validated before `tabs.clear()`; 31-command verification note
- [x] §84 — Production/UX audit findings: dotted note ids, dirty-rename guard,
      startup fingerprint error, 2 s conflict cooldown, empty-merge and
      missing-note guards, explicit control labels and side-effect status;
      GUI smoke + raw suite 271 passed
- [x] §85 — DESIGN v2 identity replacement: measured Obsidian palette, four
      presets with v2 sidecar, embedded OFL fonts, 48 px functional rail,
      8 px chrome, soft shadows, active-panel gradient, styled callouts;
      GUI smoke in four presets at two resolutions, raw suite 271 passed
- [x] §86 — Vault index recovery: startup failure flag, 1 Hz retry after a
      successful poll, status "Vault index restored"; GUI smoke with a blocked
      vault path turning into a real note, raw suite 271 passed
- [x] §87 — Pre-release bug sweep: 13 confirmed audit findings (drive-prefix
      traversal, three Unicode panics, BOM/fence parser drift, duplicate tree
      ids, corrupt sidecar allocation, capture/modal/popup state bugs); 8 new
      regression tests, raw suite 279 passed

Remaining deferred UI follow-ups (separate decision): Reading-view inline link
hover. The palette/font/component identity replacement is no longer deferred —
it was approved and implemented as §85.

## Remaining — v2

- [ ] V2-1 part B — opt-in neural `fastembed` backend; needs explicit decision on heavy deps and consent-gated model download (DD-N5)
- [ ] G3 — hover preview over links inside rendered Reading view (`CommonMarkViewer` exposes no hover targets; §32.1)
- [ ] G4 — visual table editor, math, Mermaid (each needs its own amendment/dependency decision; plain-text list, pipe-table and blockquote mechanics done in §55/§57/§61)
- [ ] G6 — `nexora://` URI registration (separate decision: OS-level side effects)
- [ ] V2-4 — local-only plugin contract over JSON-lines with per-call consent

## Remaining — v3 / long horizon

- [ ] Offline "ask your vault" with mandatory note+line citations
- [ ] Contradiction / duplicate review queue, advisory only
- [ ] Multi-vault with isolated state keyed by `(vault_id, path)`
- [ ] Deterministic offline backup bundles with hash manifest
- [ ] Temporal graph reconstructed from snapshots

## Explicitly rejected / out of scope

- [x] Decision recorded: no Sync, Publish, cloud, encryption, telemetry
- [x] Decision recorded: no community plugin marketplace, theme store, mobile app
- [x] Decision recorded: no Web Clipper while "no network" is locked
- [x] Decision recorded: no copying Obsidian code, themes, icons, or CSS

## Process and housekeeping

- [x] Every feature: lock amendment → implement → raw `cargo test` → GUI smoke → code review → fix findings → docs
- [x] Tests live in `nexora-core/tests/{storage,notes,links,retrieval,views}/`
      with explicit `[[test]]` targets; `tempfile` fixtures; GUI smoke overrides
      `USERPROFILE`/`APPDATA`
- [x] No fabricated test results; raw output pasted in chat
- [ ] Index accumulated insights into `om` memory (deferred until after features, per Zuro)
- [ ] Choose a repository license before first public release
- [ ] Initialize git locally if the project is to be published (no remote operations without a separate ask)

## Verification commands

```powershell
cargo build
cargo test
cargo run -p nexora-app
```
