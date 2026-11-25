# AGENTS.md — Nexora

## Read this first

- `Nexora/ARCHITECTURE.md` is the source of truth. Root `../CLAUDE.md` and
  `../ARCHITECTURE.md` still bind. Read amendments §15–§35 before code; each was
  locked before implementation and lists deliberate deviations.
- There is **no git repository** here (no `.git` in `Nexora` or its parent).
  Do not assume history, branches, or remotes. No push/publish without a direct ask.
- Talk to Zuro in Russian, terse. Code, docs, and architecture text in English.

## Commands

- `cargo build` · `cargo test` · `cargo run -p nexora-app`
- One suite: `cargo test -p nexora-core --test rename` (also `m1 m2 m3 commands
  display diff tree watch`)
- Rust toolchain is per-user: `C:\Users\ZuroK\.cargo\bin\`. No CI, no lint/task
  runner, no `rustfmt.toml` — do not invent formatting gates.
- Windows locks the exe: before rebuilding after a GUI smoke run,
  `Get-Process nexora-app -ErrorAction SilentlyContinue | Stop-Process -Force`.
- Set `[Console]::OutputEncoding=[System.Text.Encoding]::UTF8` before printing
  Cyrillic paths.

## Layout

- `nexora-core/` — all domain logic and **all automated tests**; no GUI deps.
  `src/` is grouped into five folders, re-exported at the crate root so every
  `crate::foo` / `nexora_core::foo` path stays unchanged:
  - `storage/` — `vault`, `save`, `frontmatter`, `watch`, `diff` (disk I/O)
  - `notes/` — `model`, `daily`, `inbox`, `rename`, `status`, `templates` (note lifecycle)
  - `links/` — `wikilink`, `outgoing`, `mentions`, `preview`, `related`,
    `display`, `graph`, `images` (link graph and projections)
  - `retrieval/` — `search`, `semantic`, `fuzzy`, `tags`, `health` (indexing)
  - `views/` — `commands`, `tree`, `outline`, `autocomplete`, `bases`, `canvas`,
    `bookmarks`, `theme`, `saved_queries` (UI-facing config and projections)
  Integration tests in `nexora-core/tests/` mirror those five folders
  (`storage/ notes/ links/ retrieval/ views/`). Cargo only auto-discovers
  test crates directly in `tests/`, so every target is registered with an
  explicit `[[test]]` entry in `nexora-core/Cargo.toml` — add one when adding
  a test file.
- `nexora-app/` — egui/eframe shell, no window tests. Verified only by manual
  smoke; describe the smoke explicitly instead of claiming "tests pass".
- `nexora-cli/` — thin offline binary over the same core functions; no logic
  outside `nexora-core`, deterministic stdout, verified by raw CLI smoke.

## UI development environment (setup 2026-11-22)

- Project-local interface skills live in `.opencode/skills/`. They came from
  `osmontero/opencode-skills` (MIT) and are web-origin: always read
  `.opencode/skills/EGUI_ADAPTATION.md` before applying them. They are advisory
  and can never override `DESIGN.md` or `ARCHITECTURE.md` §15/§35.
- Project-local agents: `design-director`, `ui-designer`, `ux-architect`,
  `egui-engineer`, `visual-reviewer` in `.opencode/agents/`. Reviewers are
  read-only; `egui-engineer` implements.
- Live UI inspection: `.opencode/opencode.jsonc` registers the `egui-inspection`
  MCP server. It attaches to a running app over `127.0.0.1:5719`; the app binds
  that port only when built with eframe's `inspection` feature and launched
  with `EGUI_INSPECTION=1`. Never bind a non-loopback address — the protocol is
  unauthenticated. See `tools/visual-smoke.ps1`.

## Locked invariants

- Markdown files on disk are the only source of truth. Indexes, collapse state,
  and UI state are rebuildable.
- `NoteId` = vault-relative path without `.md`, `/` separators, traversal
  rejected. Duplicate basenames are legitimate; keep them distinct by path.
- Any `#word` is a tag, including headings and fenced code (GM-1). Broken
  `[[link]]` is a validation error, not silent text (GM-2).
- 10 snapshots **per note** under `.nexora/history/<note-path>/`. Writes go
  snapshot → temp → remove → rename. Outside edits use a 1 Hz poll; dirty tabs
  get Reload/Overwrite, never a silent overwrite (GM-4).
- No network, sync, encryption, keys, or telemetry. `egui_commonmark` has none
  of its image/fetch features; all `OpenUrl` commands are intercepted and
  external URLs blocked.
- No stubs, no `unwrap()` on user/filesystem data, no silent `except`-style
  swallowing. Paste raw `cargo test` output in chat; never paraphrase results.

## egui 0.37 gotchas (learned the hard way)

- `eframe::App` implements `logic()` (input, autosave, polling) and `ui()`;
  there is no `update()`.
- Panels are `egui::Panel::top/left/right/bottom(...).show(ui, ...)` — they take
  a `Ui`, not a `Context`. `CentralPanel::default().show(ui, ...)`.
- `ScrollArea::id_salt(...)` (not `id_source`), `Frame::new()` (not
  `Frame::none()`), `Id::unique(...)` (not deprecated `Id::new`).
- Themes are explicit: `set_theme(ThemePreference::Dark)`, `style_of(Theme::Dark)`,
  `set_style_of(Theme::Dark, style)`.
- `TextEdit::show` returns `TextEditOutput`; give the editor an explicit
  `Id::unique(("nexora_editor", note_id))` when setting the cursor from Health.
- Duplicate `ScrollArea` IDs draw red "First/Second use" overlays — always set
  `id_salt`.

## Verification workflow

- Keep every test in `nexora-core/tests/` with `tempfile` fixtures. Never touch
  the real `Documents\Nexora` vault in tests.
- GUI smoke overrides `USERPROFILE` to a temp dir so the vault is isolated.
  Drive it with PowerShell UI Automation: `TogglePattern` for explorer/tree
  rows, `InvokePattern` for buttons. `ValuePattern.SetValue` on an egui editor
  does **not** produce edit events — use focus + `SendKeys` instead.
- After each feature: lock an amendment in `ARCHITECTURE.md`, implement,
  `cargo test`, GUI smoke, run the code-review checklist, fix findings, update
  `ARCHITECTURE.md` status, `ROADMAP.md`, and `README.md`.
- If a Rust/egui mechanism is unclear, check `docs.rs` or the crate source under
  `C:\Users\ZuroK\.cargo\registry\src\` and compile to confirm; do not guess an
  API and leave broken code behind.
