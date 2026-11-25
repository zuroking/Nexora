# Nexora development-environment setup report

Date: 2026-11-22/23. Scope: infrastructure only. No redesign, no business-logic
changes, no new framework.

## 1. Environment overview

- Project: `C:\Users\ZuroK\Desktop\kronos_github\Nexora`, workspace with three
  crates: `nexora-core` (domain logic + all automated tests), `nexora-app`
  (egui/eframe shell), `nexora-cli` (thin offline CLI).
- Rust 1.100.0 (`rustc 0ef1bc386`, 2026-11-09), Cargo 1.100.0, rustup 1.29.3.
- GUI stack: `eframe 0.37`, `egui 0.37`, `egui_commonmark 0.26` with all
  network/image/syntax features disabled. Other deps: `chrono 0.4`,
  `thiserror 2.0`, dev `tempfile 3`.
- No git repository, no push/publish. CPU-only, no network from the app.
- Read in full: `README.md`, `DESIGN.md`, `ARCHITECTURE.md` (§1–§46),
  root and all three crate manifests, `AGENTS.md`, global opencode config.

## 2. Installed tools and versions

| Tool | Status | Version / note |
|---|---|---|
| rustc, cargo, rustup | [ALREADY AVAILABLE] | 1.100.0 / 1.100.0 / 1.29.3 |
| rust-analyzer | [ALREADY AVAILABLE] | `C:\Users\ZuroK\.cargo\bin\rust-analyzer.exe` |
| rustfmt | [INSTALLED] | rustfmt 1.11.0-stable, via `rustup component add rustfmt` |
| clippy | [INSTALLED] | clippy 0.1.100, via `rustup component add clippy` |
| egui-mcp | [INSTALLED] | 0.3.0 from crates.io, `C:\Users\ZuroK\.cargo\bin\egui-mcp.exe` |
| eframe `inspection` feature | [CONFIGURED] | opt-in via `EGUI_INSPECTION` env var |

Verification commands `rustc --version`, `cargo --version`, `cargo fmt --version`,
`cargo clippy --version` all work after setup.

## 3. Installed skills

Project-local, in `.opencode/skills/` (global config was not touched):

- [CONFIGURED] `designing-frontend-interfaces` — visual craft; app/dashboard
  conventions, adapted to egui.
- [CONFIGURED] `designing-user-experience` — flows, state matrix, failure paths.
- [CONFIGURED] `building-accessible-interfaces` — WCAG 2.2 AA mapping.
- [CONFIGURED] `reviewing-interface-quality` — evidence-based UI review rubric.
- [CONFIGURED] `EGUI_ADAPTATION.md` — web→native translation table, copied into
  each skill so the web vocabulary cannot be applied literally.
- [ALREADY AVAILABLE] global skills from `ew project\skills` (api-design, tdd,
  code-review, theme-factory, etc.) remain in effect; no duplicates installed.
- [NOT COMPATIBLE] `nextlevelbuilder/ui-ux-pro-max-skill`: landing-page/web
  focused, requires a global npm CLI install and emits a generated design
  system. It would conflict with `DESIGN.md` and the locked egui palette.
  Investigated via README; intentionally not installed.
- `osmontero/opencode-skills` was inspected via a shallow clone; only the four
  interface skills and one agent were copied, not `install.sh`.

Skills were confirmed discoverable by the harness after installation.

## 4. Configured MCP servers

- [CONFIGURED] `egui-inspection` in `.opencode/opencode.jsonc`, stdio binary
  `egui-mcp.exe`. It attaches to the app over `127.0.0.1:5719` (default,
  loopback only). Existing global servers (`om`, `browse`, `global-memory`,
  `sandbox-registry`) were not modified.
- Verified live: `attach` → `widget_tree` → `click` by widget id → `screenshot`
  all succeeded against a running Nexora window. The click opened the `note`
  tab and the screenshot showed the rendered Reading view.
- [CONFIGURED] Browser automation was investigated and deliberately not
  installed: the target is a native egui window, not a web page. The existing
  global `browse` server stays available for documentation/research only.

## 5. Design system files created

22 files under `design-system/`, all restating the locked identity in
`DESIGN.md` (no new palette, type scale or component language):

- `README.md`, `foundations/{colors,typography,spacing,borders,icons}.md`
- `components/{buttons,inputs,tabs,tooltips,modals,navigation}.md`
- `layouts/{main-window,explorer,editor,context-panel,graph}.md`
- `interactions/{hover-states,transitions,keyboard-navigation}.md`
- `quality/{visual-checklist,accessibility}.md`

## 6. Configuration changes

- `nexora-app/Cargo.toml`: added `inspection` to the existing `eframe` features.
  No new dependency; environment-gated and off unless `EGUI_INSPECTION` is set.
- `.opencode/opencode.jsonc`: new project-local MCP registration (additive).
- `.opencode/agents/`: five new project agents plus the copied
  `interface-reviewer`:
  - `design-director` — read-only, approves/rejects design changes.
  - `ui-designer` — read-only, component-level proposals.
  - `ux-architect` — read-only, flows/states/recoverability.
  - `egui-engineer` — implementation within locked amendments.
  - `visual-reviewer` — read-only, screenshot + widget-tree evidence.
- `AGENTS.md`: added a short "UI development environment" section pointing to
  the skills, adaptation file, agents and inspection workflow.
- `tools/visual-smoke.ps1`: repeatable native-window screenshot + AccessKit
  control dump into `artifacts/visual/<timestamp>/`; optional `-Inspection`.

No application source files and no business logic were changed.

## 7. Verification results

- [CONFIGURED] `cargo check --workspace` — finished, no errors.
- [CONFIGURED] `cargo test --workspace` — 161 tests passed, 0 failed.
- [FAILED] `cargo fmt --all -- --check` — reports diffs, mainly unsorted imports
  and long lines in `nexora-app/src/main.rs`. This drift predates this setup
  task (it accumulated across the feature amendments); the project has no
  `rustfmt.toml` and no formatting gate. Mass reformatting was not performed
  because it would create a large unrelated diff. Fix separately if desired.
- [CONFIGURED] `cargo clippy --workspace --all-targets` — exit 0, no errors;
  ~19 style warnings (elidable lifetimes, complex type, clone on Copy `Id`,
  `let...else` suggestions). Also pre-existing; not changed here.
- [CONFIGURED] `cargo build -p nexora-app` after adding the feature — finished.
- [CONFIGURED] egui inspection: attach, widget tree, click, screenshot verified.

## 8. Compatibility issues

- `rerun-io/kittest_inspector` is archived (Oct 2026); tools moved to
  `rerun-io/egui_crates`. The current crate `egui_mcp 0.3.0` depends on
  `egui 0.37` / `egui_inspection 0.37`, matching this project exactly.
- docs.rs shows only an x86_64-linux build target for those crates, but the
  Windows release binary installed and ran correctly here.
- `egui_inspection` screenshots require a visible, foreground window; tree and
  input injection work in the background.
- The four interface skills are web-origin (DOM/CSS/breakpoints). The
  `EGUI_ADAPTATION.md` file must be read before using them.

## 9. Tools requiring manual action

- [REQUIRES MANUAL ACTION] Set `EGUI_INSPECTION=1` when launching the app to
  expose the inspection port; otherwise the feature compiles in but stays off.
- [REQUIRES MANUAL ACTION] `ui-ux-pro-max` would need a global npm install
  (`npm install -g ui-ux-pro-max-cli`), which is gated by permissions and was
  intentionally not attempted.
- [REQUIRES MANUAL ACTION] If formatting is desired, run `cargo fmt --all`
  as a separate cleanup commit — it is not part of this infrastructure task.
- [REQUIRES MANUAL ACTION] Repository license is still not selected (README).

## 10. Recommended next steps

1. Use `egui-inspection` for UI verification instead of coordinate-based UI
   Automation; keep `tools/visual-smoke.ps1` for repeatable screenshots.
2. Dispatch `design-director` before any major visual change and
   `visual-reviewer` after it, with real screenshots as evidence.
3. Decide separately whether to fix the pre-existing `rustfmt`/`clippy` debt.
4. Heavy AI features still need an explicit decision on dependencies and
   consent-gated model download; no changes were made here.
