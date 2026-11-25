# Nexora — Architecture Document

**Status:** LOCKED — grill-me completed 2026-11-22. Implementation may begin.
**As of:** 2026-11-22
**Stack:** Rust 1.100.0, `egui`/`eframe`, CPU-only
**Type:** greenfield desktop application — **not** a portfolio port

---

## 1. Scope and intent

Nexora is an Obsidian-like notes application with a deliberately smaller surface:
no plugin system, no themes, no network. Notes are plain Markdown files in a
local vault. The application is organized as a **testable core** (`nexora-core`,
no GUI dependencies) and a **thin GUI shell** (`nexora-app`) that only renders
state and forwards user actions.

Target feature set for v1 (all selected by Zuro on 2026-11-22):

1. Vault + editor + note list
2. `[[wikilinks]]` + backlinks
3. Full-text search
4. Daily notes
5. Tags
6. Graph view
7. Local version history
8. Ctrl+Click and autocomplete for links
9. Command palette (`Ctrl+P`)
10. Tabs
11. Vault health report
12. Inbox + quick capture

Every feature below has a single owner module in `nexora-core`. The GUI does not
implement business rules; it calls core functions and renders their results.

---

## 2. Inherited constraints (still binding)

- CPU-only, permanent. No GPU assumptions.
- External crates are allowed (root `ARCHITECTURE.md` §4 deviation), but every
  crate must be justified in §11.
- No stubs: no `todo!()`, no `unimplemented!()`, no silently swallowed errors.
- Live sessions end by 00:00; overnight only for unattended builds/tests.
- No push, no publishing, no remote operations without a separate ask.
- Tests must not touch the network.

---

## 3. Locked decisions — triads

Format: **chosen** / rejected / excluded.

### 3.1 Application shape
- **Desktop GUI (`egui`/`eframe`)** / TUI (`ratatui`) / Tauri or Electron.
  Zuro chose an Obsidian-like desktop app; `eframe` keeps everything Rust and
  CPU-only. Tauri would add a web frontend, Electron a Node runtime.

### 3.2 Storage
- **Plain Markdown files, one note per file, nested folders allowed** / SQLite
  (`rusqlite`) / single JSON file. Files stay human-readable and editable
  outside Nexora; no database dependency; no fragile monolithic write.

### 3.3 Note identity
- **Vault-relative path without `.md`, canonical `/` separators** / basename
  / content hash. Zuro's ruling: two notes with the same filename in different
  folders are different notes because their paths differ. `[[Note]]` addresses
  the root-level `Note.md`; `[[dir/Note]]` addresses the nested one. A link is
  resolved by exact path lookup — there is no fuzzy basename search.

### 3.4 Vault location
- **Auto-create `%USERPROFILE%\Documents\Nexora\my-first-vault` on first run**
  / native folder picker (`rfd`) / fixed path secret. Zuro chose the fixed
  default; no picker dependency, no config file, no first-run prompt.

### 3.5 Autosave
- **Debounced, non-atomic write with prior version snapshot** / manual Save
  button / WAL-style journal. Zuro requires autosave. On Windows `rename`
  cannot overwrite an existing file, so the write order is: snapshot old
  content → write `<path>.nexora-tmp` → remove target → rename temp → target.
  A crash between remove and rename loses the current write, but the snapshot
  preserves the previous content and is restorable.

### 3.6 Dependencies
- **`eframe`+`egui`, `chrono`, `thiserror`; dev-only `tempfile`** / std-only
  GUI (hand-rolled winit) / heavyweight index or graph crates (`tantivy`,
  `petgraph`). Search and graph layout are small enough to implement directly;
  they stay in the core and remain unit-testable.

### 3.7 Verification
- **Raw `cargo test` output pasted into chat + manual GUI smoke run** / 1:1
  parity with a Python baseline / README test counts. Nexora is a greenfield
  app with no Python ancestor, so `pytest` parity does not exist.

---

## 4. Workspace and vault layout

```
%USERPROFILE%\Documents\Nexora\
├── my-first-vault\               # the initial vault
│   ├── Inbox.md                  # quick-capture target (created on demand)
│   ├── 2026-11-22.md             # daily notes, vault root
│   ├── Projects\
│   │   └── Nexora.md             # identity = "Projects/Nexora"
│   └── .nexora\                  # application metadata — NEVER indexed
│       └── history\              # per-note snapshots
│           └── <note-path>\      # mirrors the note's relative path
│               └── <timestamp>   # previous content of that note
```

Invariants:

- Every path component beginning with `.` is skipped by the vault scanner
  (`.nexora`, `.git`, …). Hidden files are application metadata, not notes.
- Only `.md` files are notes. Case of the extension is normalized to lowercase.
- `NoteId` is a `String` using `/` separators regardless of host OS. Conversion
  to an OS path happens only at filesystem boundaries.
- The vault root is the only writable note area. The app never writes outside
  `%USERPROFILE%\Documents\Nexora\`.

---

## 5. Module boundaries

Workspace with two crates:

```
Nexora/
├── Cargo.toml                 # workspace root
├── nexora-core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs             # public re-exports, NexoraError
│       ├── model.rs           # NoteId, Note, VaultIndex
│       ├── vault.rs           # discovery, load, reload, path conversion
│       ├── save.rs            # autosave write order, snapshots, restore
│       ├── wikilink.rs        # parse [[target|alias]], resolve, backlinks
│       ├── tags.rs            # #tag extraction with exclusion rules
│       ├── search.rs          # in-memory ranked substring search
│       ├── daily.rs           # local date -> YYYY-MM-DD.md
│       ├── inbox.rs           # quick capture append
│       ├── health.rs          # broken links, orphans, duplicate basenames
│       ├── graph.rs           # nodes/edges + deterministic layout
│       └── commands.rs        # command registry shared by palette/shortcuts
└── nexora-app/
    ├── Cargo.toml
    └── src/
        ├── main.rs            # eframe entry point
        ├── app.rs             # AppState, frame loop, debounce timer
        ├── shortcuts.rs       # keyboard map -> commands
        └── ui/
            ├── mod.rs
            ├── note_list.rs
            ├── editor.rs      # tabs
            ├── sidepanel.rs   # backlinks + tags
            ├── search.rs
            ├── palette.rs
            ├── graph_view.rs
            └── health.rs
```

Dependency rule: `nexora-app` depends on `nexora-core`; `nexora-core` never
depends on `nexora-app` or any GUI crate. All testable logic lives in core.

---

## 6. Data model

```rust
pub struct NoteId(pub String);              // canonical "dir/name", no .md

pub struct Note {
    pub id: NoteId,
    pub title: String,                       // first H1, else stem
    pub content: String,
    pub modified: SystemTime,                // from filesystem mtime
}

pub struct VaultIndex {
    notes: BTreeMap<NoteId, Note>,           // deterministic iteration order
    backlinks: BTreeMap<NoteId, BTreeSet<NoteId>>,
    tags: BTreeMap<String, BTreeSet<NoteId>>,
    links: BTreeMap<NoteId, Vec<Link>>,      // resolved + unresolved
}
```

- `NoteId` derives `Ord` so every index has deterministic iteration order —
  required for reproducible tests and stable graph layout.
- `VaultIndex` is rebuilt on explicit reload, not incrementally patched. Vaults
  are small (hundreds of notes); a full rebuild is simpler and correct.
- No YAML frontmatter in v1. Metadata comes from the filesystem and note text.

---

## 7. Feature semantics

### 7.1 Vault (`vault.rs`)

- Recursively walk the vault root, skipping dot-prefixed components.
- Convert OS path → `NoteId` by stripping the root and extension and joining
  components with `/`.
- Reading a note returns its raw UTF-8 content; invalid UTF-8 is a typed error
  naming the path, never a panic.
- `reload()` rebuilds the whole index from disk.

### 7.2 Save, autosave, history (`save.rs`)

- Debounce interval: **500 ms** after the last keystroke, checked in the frame
  loop (no background thread, no timers crate).
- On each write:
  1. Re-read the target from disk. If its content differs from the snapshot
     taken at load time, abort and report a conflict (see GM-4).
  2. If the new content differs from the old content, copy the old content to
     `.nexora/history/<timestamp>/<note-path>`.
  3. Write `<note-path>.nexora-tmp`, remove target (ignore `NotFound`), rename
     temp → target.
- Retention: keep the last **10** snapshots **per note**, stored under
  `.nexora/history/<note-path>/<timestamp>`. Prune that note's oldest snapshots
  beyond 10. Zuro overruled the proposed global queue (GM-3).
- `restore(note_id, timestamp)` copies a snapshot back over the note using the
  same write order. It is exposed as a command in the palette.
- Every I/O error is returned as `NexoraError::Io { path, source }`.

### 7.3 Wikilinks and backlinks (`wikilink.rs`)

- Syntax: `[[target]]` and `[[target|alias]]`. `target` is a vault-relative
  path without `.md`; whitespace around target/alias is trimmed.
- Parsing is a single left-to-right scan; nested brackets are not supported in
  v1 (excluded). `[[` inside a fenced code block is ignored.
- Resolution: exact `NoteId` lookup. Missing target → `Link::Broken { target }`.
  A broken link is a **validation error**, not a silent degradation (Zuro,
  GM-2): `validate()` returns `NexoraError::BrokenLink { note, target }` for
  every unresolved link (the field is `note`, not `source`, because `thiserror`
  reserves `source` for the error chain). The error is surfaced in the health report; it does
  not prevent the vault from loading, so the user can still fix it.
- Backlinks are derived from resolved links only. Rendering alias uses `alias`,
  navigation uses `target`.
- Ctrl+Click on a resolved link opens the note in the current tab; on a broken
  link the palette offers "Create note at <path>".

### 7.4 Tags (`tags.rs`)

- A tag is `#`, followed by optional spaces, followed by one or more Unicode
  alphanumeric characters, `_` or `-`. Unicode is required because notes are
  written in Russian.
- **No exclusions.** Zuro chose "any `#word` is a tag" (GM-1), overriding the
  proposed heading/code exclusions. Therefore `# Заголовок` and `## Заголовок`
  both produce the tag `Заголовок`, and `#тег` inside a fenced code block is
  still a tag. This is deliberate; do not "fix" it later without a new decision.
- Nested tags (`#work/project`) are not special in v1: `#work` is a tag and
  `/project` is plain text.

### 7.5 Search (`search.rs`)

- Case-insensitive substring over title and body. No stems, no fuzzy matching,
  no external index (excluded: `tantivy`).
- Ranking (locked, GM-6): exact title match → title substring → body
  substring; ties broken by `NoteId` ascending.
- The index is rebuilt with `VaultIndex`; search operates in memory.

### 7.6 Daily notes (`daily.rs`)

- `chrono::Local::now().date_naive()` formatted `%Y-%m-%d`; file at vault root
  with that stem and `.md`.
- Command "Open today's note" creates the file if absent (empty content), then
  opens it in a tab. No separate daily folder, no template in v1.

### 7.7 Inbox / quick capture (`inbox.rs`)

- Target: `Inbox.md` at vault root, created on demand.
- `Ctrl+Shift+N` opens a one-line modal; Enter appends
  `[YYYY-MM-DD HH:MM] <text>\n` to `Inbox.md` (locked, GM-5).
- Append uses the same write order as autosave so a capture is never lost by a
  concurrent save.

### 7.8 Health report (`health.rs`)

Three categories, computed from `VaultIndex`:

- **Broken links** — `Link::Broken` occurrences with source note and target.
  Classified as **errors** (GM-2), not warnings.
- **Orphan notes** — notes with no incoming resolved links, excluding
  `Inbox.md` and daily-note filenames. Warning.
- **Duplicate basenames** — informational only; never renamed, because path
  identity makes duplicates legitimate (GM-7 resolved by default). Warning.

Each row is clickable and opens the relevant note in a tab.

### 7.9 Graph (`graph.rs`)

- Nodes = all notes; edges = resolved wikilinks, deduplicated, directed for
  semantics but treated as undirected pairs by the layout forces.
- Layout (locked, GM-8): initialize nodes on a circle in `NoteId` order,
  run a fixed number of deterministic force iterations with fixed constants —
  no RNG, no wall-clock input. Layout is computed in core and tested for
  determinism; `graph_view.rs` only draws it.
- If a vault exceeds 500 notes, render the 500 highest-degree nodes and show a
  truncation notice. Full graph remains in the index.

### 7.10 Commands (`commands.rs`)

A single registry (`CommandId`, label, optional shortcut, handler) is the source
for both the palette and keyboard shortcuts. v1 commands: new note, open
today's note, quick capture, search, open graph, run health check, restore
snapshot, create note at broken-link target, close tab. Shortcuts: `Ctrl+P`,
`Ctrl+N`, `Ctrl+Shift+N`, `Ctrl+F`, `Ctrl+W`, `Ctrl+Click` (links).

---

## 8. GUI layout and interaction

```
┌──────────────────────────────────────────────────────────────┐
│ toolbar: search · Ctrl+P · today · health · graph            │
├───────────────┬──────────────────────────────┬────────────────┤
│ note list     │ tabs                         │ backlinks      │
│ (all notes,   │ ┌──────────────────────────┐  │ tags           │
│  flat by      │ │ editor (Markdown text)   │  │ (click -> nav) │
│  NoteId)      │ └──────────────────────────┘  │                │
├───────────────┴──────────────────────────────┴────────────────┤
│ status bar: autosave state · conflict banner · snapshot count │
└──────────────────────────────────────────────────────────────┘
```

- Graph view and health report are tabs in the center panel, not separate
  windows.
- One built-in theme (`eframe` default); no theme picker (excluded).
- Markdown is edited as plain text; a rendered preview is deferred to v1.1 and
  would require `pulldown-cmark` (not in v1 dependencies).

---

## 9. Error handling and invariants

- `NexoraError` is a `thiserror` enum with variants: `Io { path, source }`,
  `InvalidUtf8 { path }`, `NotFound { id }`, `InvalidId { id }`,
  `InvalidInput { message }`, `Conflict { path }`, `BrokenLink { note, target }`.
  The broken-link field is `note`, not `source`: `thiserror` reserves `source`
  for the error chain.
- No `unwrap()` on user data or filesystem input; `expect()` only on internal
  invariants with a message. No `panic!` in library code paths.
- Every filesystem handle is closed deterministically; Windows is strict about
  open handles (see `Gotchas.md`: WinError 32).
- The vault scanner never writes; only `save.rs` and `inbox.rs` write.

---

## 10. Testing and definition of done

- All tests live in `nexora-core/tests/{storage,notes,links,retrieval,views}/`
  and use `tempfile` fixtures. No real network, no real user Documents dir.
  Every file is registered as an explicit `[[test]]` target in
  `nexora-core/Cargo.toml` because Cargo only auto-discovers test crates
  directly in `tests/` (§88).
- Coverage per module: discovery/path identity; link parsing/resolution/
  backlinks; tag extraction (no exclusions, GM-1); search ranking; daily naming; inbox append;
  snapshot + restore; health categories; graph determinism; conflict
  detection; command registry.
- `nexora-app` has no automated window tests; it is verified by a manual smoke
  run (launch, create note, type, observe autosave, follow link, open graph).
- **Done** = raw `cargo test` output pasted into chat with matching assertions,
  plus a described manual smoke run. No paraphrase, no "tests pass" claim.

---

## 11. Dependency justification

| Crate | Why | Rejected alternative |
|---|---|---|
| `eframe`, `egui` | immediate-mode desktop GUI, pure Rust | Tauri (web), TUI (not Obsidian-like) |
| `chrono` | local date for daily notes and capture timestamps | manual calendar arithmetic |
| `thiserror` | typed error enum without boilerplate | hand-written `Display` (verbose) |
| `tempfile` (dev) | isolated vault fixtures | fixed shared temp paths (flaky) |

No search-index crate, no graph crate, no DB crate, no folder-picker crate.

---

## 12. Grill-me log

One adversarial pass on this draft, completed 2026-11-22. Every item is closed.
Three answers overruled the proposed default; those are recorded as decisions,
not as temporary exceptions.

| ID | Challenge | Decision (Zuro, 2026-11-22) | Overrode proposal? |
|---|---|---|---|
| GM-1 | `#tag` vs heading vs code? | **Any `#word` is a tag**, including `# Заголовок` and code blocks; nested tags not special | yes |
| GM-2 | broken `[[link]]` behavior? | **Validation error**; surfaced in health, vault still loads | yes |
| GM-3 | history retention? | **10 snapshots per note**, under `.nexora/history/<note-path>/` | yes |
| GM-4 | file edited outside Nexora while open? | Conflict banner; no silent overwrite; offer Reload / Overwrite | no |
| GM-5 | quick-capture format? | `[YYYY-MM-DD HH:MM] text` appended to `Inbox.md` | no |
| GM-6 | search ranking? | title exact → title substring → body, path tiebreak | no |
| GM-7 | duplicate basenames in health? | Informational warning only; never rename | resolved by default |
| GM-8 | graph determinism / large vault? | Fixed circle + fixed iterations, no RNG; top 500 by degree | no |

Closed items are copied into §5 of the root `ARCHITECTURE.md` grill-me log.
Status is **LOCKED** as of 2026-11-22.

---

## 13. Out of scope

- No network, sync, encryption, cloud, or telemetry.
- No plugins, themes, Markdown preview, templates, calendar, statistics,
  pinned notes, kanban boards.
- No writes outside `%USERPROFILE%\Documents\Nexora\`.
- No push, publishing, or remote operations.

---

### 13.1 Deliberate deviations (implementation)

- **DD-N1 — link autocomplete is a suggestion strip, not an inline popup.**
  egui's `TextEdit` does not expose an inline completion layer without taking
  over its galley painting. The editor detects an unclosed `[[target` and shows
  up to eight matching `NoteId`s in a clickable strip directly below the text
  area; clicking one replaces the partial target. Ctrl+Click navigation is
  implemented separately via `Galley::cursor_from_pos` on the editor's own
  galley, so navigation still happens in the editor as locked in §7.10.
  Rationale: keeps the editor a stock `TextEdit` and avoids a custom text widget
  in v1.

## 14. Milestones

| ID | Work | Gate | Status |
|---|---|---|---|
| M0 | this document + grill-me | status LOCKED | done 2026-11-22 |
| M1 | `model`, `vault`, `save` + history + tests | raw `cargo test` | done 2026-11-22 |
| M2 | `wikilink`, `tags`, `search`, `daily`, `inbox`, `health` + tests | raw `cargo test` | done 2026-11-22 |
| M3 | `graph` + deterministic layout + tests | raw `cargo test` | done 2026-11-22 |
| M4 | GUI shell: list, editor tabs, autosave wiring | manual smoke | done 2026-11-22 |
| M5 | link navigation, palette, search, daily/tags, graph, health UI | manual smoke | done 2026-11-22 |
| M6 | final `cargo test` + full smoke | both | done 2026-11-22 |

Implementation stops at each milestone gate. No code before M0 is locked.

---

**Locked 2026-11-22. Grill-me closed. Implementation may begin.**

---

## 15. Amendment — Reading view and authored visuals (v1.1 slice 0)

Added 2026-11-22 after Zuro asked for a more human design and Obsidian-like
rendering. This section is locked and overrides §8 ("preview deferred to v1.1")
and §11 ("no parser crate") only for the v1.1 slice. Everything else in §1–§14
stays binding.

### 15.1 Triad

- **Decision:** each note tab has a view mode, `Source` (raw `TextEdit`, v1
  behaviour) or `Reading` (rendered). `Ctrl+E` toggles. Live Preview stays out
  of scope.
- **Decision:** rendering is display-only. The renderer receives a *display
  adaptation* of the Markdown; the bytes on disk and the core link/tag
  indexes are never changed by it.
- **Decision:** one authored dark visual preset replaces egui defaults. No
  theme picker, no gradients, no network calls.

### 15.2 Deliberate deviations

- **DD-N2 — `egui_commonmark` 0.26.0 as a display adapter.** §11 rejected a
  parser crate for v1; v1.1 needs rendered reading. `egui_commonmark` is
  MIT OR Apache-2.0, depends on `egui ^0.37` (already used) and
  `pulldown-cmark ^0.13`. We enable none of its `fetch`, `load-images`, `svg`,
  `embedded_image`, or `syntect` features, so it cannot touch the network or
  add binary assets. Rationale: CommonMark parsing has no educational value
  for Nexora, whose core value is vault semantics; all link/tag/search logic
  remains hand-rolled and tested in `nexora-core`. The crate is a renderer,
  not an abstraction over our domain.
- **DD-N3 — display adaptation of Obsidian-flavoured syntax.** Before passing
  text to the renderer, `[[target|alias]]` / `[[target]]` become clickable
  Markdown links with a `nexora://` target, `==highlight==` becomes emphasis,
  and `> [!kind]` callouts become a bold-labelled quote. This is a pure
  transform into a local `String`. It never writes to the vault and never
  feeds back into indexing.

### 15.3 Non-goals of this slice

Live Preview, images, math, themes, custom font binaries, and rendered
wikilink embeds (`![[...]]`). They stay in `ROADMAP.md`.

### 15.4 Verification

The same gate as M6: raw `cargo test` output pasted into chat, plus a described
GUI smoke run. Renderer behaviour is exercised through manual smoke because
`nexora-app` has no window tests (§10).

### 15.5 Status

Implemented 2026-11-22. Source/Reading toggle, display adaptation, renderer,
authored palette, and link interception are in `nexora-app` / `nexora-core::
display`. Open cosmetic gaps are recorded in `DESIGN.md` §6 and stay in v1.1
polish; they are not silent omissions. Gate: raw `cargo test` (31 tests) plus a
described GUI smoke with screenshot.

---

## 16. Amendment — external-change detection (v1.1 slice 1)

Added and locked 2026-11-22 before code. Markdown on disk is the source of
truth; without this slice the app can silently serve a stale buffer.

### 16.1 Triad

- **Decision:** poll the vault every 1000 ms instead of installing a filesystem
  watcher crate. Compare a per-note fingerprint (content hash + modified time)
  captured at startup / after our own writes. Polling is deterministic, needs
  no new dependency, and avoids Windows event storms and self-triggered echo
  events.
- **Decision:** a change to a **clean** open tab is loaded silently (the disk
  wins, no data can be lost); a change to a **dirty** open tab raises the
  conflict banner from GM-4 with Reload / Overwrite. No silent overwrite.
- **Decision:** deletions of clean open tabs close the tab; deletions of dirty
  tabs raise the same conflict banner. Creation of new notes only refreshes the
  sidebar index.

### 16.2 Deliberate deviation

- **DD-N4 — polling instead of `notify`.** A watcher would be more responsive,
  but on Windows it produces duplicate/rename events that must be deduplicated
  against our own writes, and it adds a dependency whose behaviour is hard to
  unit-test without a real filesystem. The vault is a local folder of a few
  hundred small files: a 1 Hz scan of metadata + content hash is cheap, fully
  testable with `tempfile`, and cannot miss a change made while the app was
  hidden. Latency of at most one second is acceptable for a notes app.

### 16.3 Data flow

`watch::DiskMonitor::refresh()` → `Vec<Change>` → for each change, app mutates
only the affected tab → `reload_index()` once → `monitor.resync()` after any
Nexora write. The scanner is read-only; only `save.rs` / `inbox.rs` write.

### 16.4 Verification

Raw `cargo test` output, including `tests/watch.rs`, plus GUI smoke: edit a
note in an external editor while Nexora has it open and observe clean reload /
dirty conflict banner.

Status: implemented 2026-11-22. `nexora-core::watch`, `App::logic` polling, and
the existing conflict banner are wired. Verified: 34 tests pass; GUI smoke
opened a note, rewrote the file externally, and the Reading view showed the new
text within one poll. Code review found and fixed an off-by-one in the active
tab index when an externally deleted tab was removed. Open non-blocking limits:
one global conflict slot, and `DefaultHasher` fingerprints are per-run only.

---

## 17. Amendment — folder tree explorer (v1.1 slice 2)

Locked 2026-11-22 before code, after Zuro asked for a Zed / VS Code style
explorer on the left instead of a flat note list.

### 17.1 Triad

- **Decision:** the left panel becomes a hierarchical tree built from
  `NoteId` paths; folders are implicit prefixes, no filesystem metadata is
  stored. Root row is the vault folder name.
- **Decision:** folders sort before notes, then by name (Unicode order).
  Duplicate basenames in different folders remain distinct because the path is
  part of the node identity.
- **Decision:** collapse state lives in app memory as a set of collapsed folder
  paths; default is expanded. No persistence in v1.1 (session-local only).
- **Decision:** the tag chip row stays above the tree and filters it; it does
  not change note data or paths.

### 17.2 Non-goals

Drag-and-drop move, context menus, file creation from the tree, icons theme,
multi-select. `Ctrl+N` still creates notes at the vault root.

### 17.3 Verification

Raw `cargo test` including `tests/tree.rs`, plus GUI screenshot of nested
folders with chevrons and expand/collapse.

Status: implemented 2026-11-22. Left panel is now an explorer tree built from
`nexora_core::tree`; rows use flat `selectable_label` with `push_id(node.id_key())`
so duplicate basenames cannot collide. Verified: 39 tests pass, screenshot of
`org/people`, `perf/brag`, `reference`, `templates`, and UIA toggle confirmed
`v org/` → `> org/`. Code review fixed two test assumptions about sort order
and changed the root collapse key to a control-character sentinel so a real
folder cannot collide with it.

---

## 18. Amendment — rename with wikilink rewriting (v1.1 slice 3)

Locked 2026-11-22 before code, from ROADMAP V1.1-2.

### 18.1 Triad

- **Decision:** `F2` on the active note opens a two-stage modal: enter a new
  vault-relative path, see a dry-run count, then confirm. No fuzzy matching —
  only exact `[[target]]` / `[[target|alias]]` occurrences are rewritten, with
  fenced code blocks skipped (same rule as §7.3).
- **Decision:** `commands.rs` gains `RenameNote` with shortcut `F2`, keeping the
  single registry as the source for palette and keys.
- **Decision:** apply order is: snapshot old note → rewrite every affected
  note through `save::write_note` → `fs::rename` old file to new path → copy
  history files into the new history directory. A failure can leave a partial
  rewrite, but every touched note has a snapshot; this is accepted and shown in
  the preview.

### 18.2 Invariants

- The new path must be safe and must not already exist.
- A rename does not touch aliases, tags, frontmatter, or note titles inside the
  text; only wikilink targets.
- Open tabs keep their view mode but switch identity to the new `NoteId`.

### 18.3 Verification

Raw `cargo test` including `tests/rename.rs`, plus GUI smoke: F2 → preview →
confirm, then check backlinks still resolve.

Status: implemented 2026-11-22. `nexora_core::rename` plans and applies exact
rewrites; `save::snapshot_note` preserves history before the file move.
Verified: 45 tests pass; GUI smoke renamed `b` → `folder/c` through F2 /
Preview / Confirm, old file gone, `a.md` now contains `[[folder/c]]`, `stderr`
empty. Code review found and fixed two issues: the snapshot trick used a
conflicting `loaded_content`, and line rewriting normalised CRLF to LF (now
line endings are preserved and covered by a test). Open limit: a dirty tab
showing an old wikilink is not auto-rewritten and will raise the existing
conflict banner if saved.

---

## 19. Amendment — snapshot timeline with line diff (v1.1 slice 4)

Locked 2026-11-22 before code, from ROADMAP V1.1-4.

### 19.1 Triad

- **Decision:** clicking a snapshot in the right panel opens a modal with a
  line diff between that snapshot (left) and the current note (right). The
  palette command `RestoreSnapshot` opens the newest snapshot in the same
  modal instead of restoring blindly.
- **Decision:** the diff is hand-rolled LCS over lines in `nexora-core::diff` —
  no `similar` crate. Above 1,000,000 DP cells it degrades to common
  prefix/suffix plus one replace block, so a huge note cannot allocate
  unbounded memory.
- **Decision:** Restore is disabled while the note tab is dirty; the modal says
  why. Restoring still goes through `save::restore_snapshot`, so the current
  content is snapshotted first and history is never deleted.

### 19.2 Verification

Raw `cargo test` including `tests/diff.rs`, plus GUI smoke: open a note, edit
it, open a snapshot, observe added/removed rows, restore, verify content.

Status: implemented 2026-11-22. `nexora_core::diff` is a bounded LCS with a
prefix/suffix fallback; `save::read_snapshot` feeds the modal. Verified: 49
tests pass; GUI smoke edited a note, opened its snapshot from History, saw the
`+` rows in the modal, pressed Restore, and the file returned to `v1`. Code
review removed a duplicated inert Restore button and an `unwrap()` on modal
state; both replaced with checked responses / `if let`. Open nit: the diff grid
has no column headers yet.

---

## 20. Amendment — actionable health report (v1.1 slice 5)

Locked 2026-11-22 before code, from ROADMAP V1.1-5.

### 20.1 Triad

- **Decision:** `HealthIssue` gains `line: Option<usize>` (1-based). Broken-link
  issues carry the exact source line; orphan/duplicate issues carry `None`.
- **Decision:** clicking a health row opens the note in Source mode and places
  the editor cursor at that line, scrolling it into view. Rows without a line
  open the note normally.
- **Decision:** the report is recomputed only when the Health tab is drawn from
  the current index, not per keystroke; no caching layer in v1.1.

### 20.2 Verification

Raw `cargo test` including the updated health assertions, plus GUI smoke:
create a broken link on a known line, run Health, click the row, confirm the
note opens with the cursor on that line.

Status: implemented 2026-11-22. `HealthIssue.line` is computed for broken
links; the Health row shows `line N` and opens the note in Source mode with the
cursor at that line via `TextEdit::load_state` + explicit editor `Id`. Verified:
49 tests pass; GUI smoke opened Health, clicked the broken-link row, and the
caret landed before `[[Missing]]` on line 3. Code review fixed a moved-value
compile error, a deprecated `Id::new` call, and an early `?` in `line_of_link`
that could abort scanning on an unclosed bracket. Open nit: the report is
recomputed each frame while the Health tab is visible (fine for current vault
sizes).

---

## 21. Amendment — fuzzy file switcher and go-to-heading (v1.1 slice 6)

Locked 2026-11-22 before code, from ROADMAP V1.1-6.

### 21.1 Triad

- **Decision:** `Ctrl+O` opens a fuzzy file switcher over note titles, aliases
  (unstemmed paths) and paths; `Ctrl+Shift+O` opens the heading outline of the
  active note only. Both are separate from the `Ctrl+P` command palette.
- **Decision:** fuzzy matching is a hand-rolled subsequence score with
  prefix / word-boundary / consecutive bonuses — no fuzzy crate, explainable
  and unit-testable. Ties break by `NoteId`.
- **Decision:** choosing a heading reuses `pending_line` from §20, so it opens
  the note in Source mode with the cursor on that line. The file switcher
  reuses `open_note` and keeps the note's current view mode.

### 21.2 Non-goals

Recency ranking, aliases from frontmatter, multi-vault scope, fuzzy search in
the full-text search panel. Those stay in `ROADMAP.md`.

### 21.3 Verification

Raw `cargo test` including `tests/fuzzy.rs` and `tests/outline.rs`, plus GUI
smoke: open the switcher, type part of a path, Enter; then open the outline and
jump to a heading on a known line.

Status: implemented 2026-11-22. `fuzzy::score` and `outline::extract` are pure
core modules with 3 + 2 tests; `Ctrl+O` / `Ctrl+Shift+O` are in the command
registry (12 commands). Verified: 53 tests pass; GUI smoke opened Quick open
through the palette and typed a query. Keyboard shortcut smoke through SendKeys
was unreliable because other desktop windows stole focus — the commands are
registered and reachable from the palette, but the direct key chord was not
independently confirmed. Code review fixed edition-2021 let-chains, a double
mutable borrow of the window flag, and a fuzzy ranking assumption (consecutive
bonus raised to 8).

---

## 22. Amendment — Properties / frontmatter panel (v1.1 slice 7)

Locked 2026-11-22 before code, from ROADMAP V1.1-7.

### 22.1 Triad

- **Decision:** only four keys are editable: `created`, `modified`, `aliases`,
  `tags`. Every other line inside the frontmatter block — comments, unknown
  keys, unusual formatting — is preserved byte-for-byte.
- **Decision:** `aliases` and `tags` are shown as comma-separated lists and
  written back as inline YAML arrays `[a, b]`. Scalar values are written
  double-quoted when edited.
- **Decision:** editing happens in a modal with explicit Save/Cancel; Save uses
  `save::write_note`, so a snapshot is created and the conflict guard still
  applies. No live rewrite while typing.

### 22.2 Non-goals

Full YAML round-trip, nested keys, block-style list rewrite, schema
validation, date pickers. Unknown keys are never removed or reformatted.

### 22.3 Verification

Raw `cargo test` including `tests/frontmatter.rs`, plus GUI smoke: open a note
with frontmatter, change a scalar and add a tag, Save, verify bytes of unknown
lines are unchanged.

Status: implemented 2026-11-22. `nexora_core::frontmatter` edits four keys and
preserves every other line; the modal saves through `save::write_note`. Verified:
60 tests pass; GUI smoke changed `tags: [a]` → `["school", "algebra"]` while
`# comment`, `created`, and `custom` stayed byte-identical. Code review found a
critical BOM bug: a UTF-8 BOM hid the leading `---`, so the first save created a
second frontmatter block and duplicated the note. Fixed by stripping/preserving
the BOM explicitly; also fixed block-list indentation and empty-key insertion.

---

## 23. Amendment — inline `[[` autocomplete (v1.1 slice 8)

Locked 2026-11-22 before code, from ROADMAP V1.1-3. This **supersedes DD-N1**:
the suggestion strip below the editor is replaced by a popup anchored at the
cursor in Source mode.

### 23.1 Triad

- **Decision:** while typing `[[` in Source mode, scan backwards from the
  cursor to the unmatched opening bracket; rank matching `NoteId`s with the
  existing `fuzzy::score`; show at most 8 in a popup at the cursor.
- **Decision:** Tab / Enter inserts the selected target and closes the popup;
  Escape dismisses it. Arrow keys move the selection. Clicking a row inserts a
  target. No edit is applied to disk until autosave runs.
- **Decision:** `autocomplete::partial_at` is a pure core function with tests;
  the GUI only handles display and insertion.

### 23.2 Verification

Raw `cargo test` including `tests/autocomplete.rs`, plus GUI smoke: type `[[alg`
in Source mode, see the popup, press Enter, confirm `[[school/math/algebra]]`
is inserted.

Status: implemented 2026-11-22. `autocomplete::partial_at` is pure and tested;
the popup is anchored at the galley cursor position and uses `fuzzy::score`.
Verified: 64 tests pass; GUI smoke typed `[[alg`, saw the popup with
`school/math/algebra`, pressed Enter, and the file became `[[school/math/algebra]]`.
Code review found that `TextEdit` consumed Enter before the popup could act —
key flags are now captured before the editor and matching key events are
removed from input while the popup is open. DD-N1 is superseded by §23.

---

## 24. Amendment — last-saved time in status bar (v1.1 slice 9)

Locked 2026-11-22 before code, from ROADMAP V1.1-8. This closes the v1.1 set.

### 24.1 Triad

- **Decision:** the status bar shows a relative age (`just now`, `12s ago`,
  `3m 4s ago`) instead of a wall-clock timestamp, so no new time crate or
  timezone handling is needed.
- **Decision:** the age is computed from an `Instant` captured on every
  successful Nexora write; an external reload clears the marker, so the status
  never claims a save that did not happen.
- **Decision:** `status::saved_label(seconds)` is a pure core function with
  tests; the app only measures elapsed time.

### 24.2 Verification

Raw `cargo test` including `tests/status.rs`, plus GUI smoke: edit a note,
wait for autosave, observe `just now` then `Ns ago` in the status bar.

Status: implemented 2026-11-22. This closes v1.1. `status::saved_label` has a
test; the app records an `Instant` on every successful write and clears it on
external reload. Verified: 65 tests pass; GUI smoke shows
`Saved note · 3 words · saved just now`. Code review found the status bar was
added *after* `CentralPanel`, so egui gave it zero visible height — it is now
added before the central panel with an explicit 56 px size, and the default
window is 1120×680 so the bar fits on a 1536×864 desktop.

---

## 25. Amendment — local vector index, part A (v2 start)

Locked 2026-11-22 before code. Zuro chose to build the pure core part first and
postpone the model backend pending a separate decision about heavy deps and
consent-gated downloads.

### 25.1 Triad

- **Decision:** ship a deterministic, dependency-free embedder based on hashed
  Unicode character 3-grams + word tokens (384 dims, L2-normalised). It is a
  *local lexical vector space*, not a neural model: it cannot infer synonyms.
  The UI must say so honestly.
- **Decision:** the index lives in a sidecar under app data, never in the vault;
  it stores `(NoteId, line, vector)` plus a vault fingerprint. Stale indexes are
  labelled, not silently trusted. Markdown stays the only source of truth.
- **Decision:** search has three modes — `Keyword` (unchanged §7.5),
  `Semantic` (cosine over chunk vectors), `Hybrid` (blended scores with visible
  source per hit). Keyword results must work when the index is missing or stale.

### 25.2 Deliberate deviation

- **DD-N5 — no `fastembed` / ONNX in this slice.** `fastembed` 7.7.1 would give
  `multilingual-e5-small`, but adds ONNX Runtime, hf-hub, tokenizers and a
  build-time runtime download plus a ~100 MB model download at runtime. That is
  a consequential dependency and network decision; Zuro deferred it. The hash
  embedder is pure Rust, CPU-only, testable with `tempfile`, and needs no
  network. A neural backend can replace it behind the same `Embedder` contract
  in a later amendment.

### 25.3 Verification

Raw `cargo test` including `tests/semantic.rs`, plus GUI smoke: build index,
search in all three modes, observe honest "lexical vectors" label.

Status: part A implemented 2026-11-22. `nexora_core::semantic` provides
chunking, a deterministic 384-dim hash embedder, cosine search, sidecar
save/load with vault fingerprint, and keyword/vector blending. GUI adds a
`Vectors` button (builds the sidecar under app data) and Keyword/Semantic/Hybrid
modes in the Search tab. Verified: 71 tests pass; GUI smoke built a 2-chunk
index and Semantic mode returned `math — math · line 1 · 0.39` with the
"lexical vectors · local, no model, no network" label. Code review found the
sidecar save failed when the app-data directory did not exist — now created
with `fs::create_dir_all`. Neural backend (`fastembed`) remains deferred by
DD-N5.

---

## 26. Amendment — related-note suggestions (v2-2 slice 0)

Locked 2026-11-22 before code. Built on V2-1 part A; no new dependency.

### 26.1 Triad

- **Decision:** suggestions are ranked by three *visible* factors: lexical
  similarity from the V2-1 vectors (weight 0.5), tag-set Jaccard (0.25), and
  undirected graph proximity up to depth 3 (0.25). Every row shows the three
  numbers — no black-box score.
- **Decision:** only notes outside the current note are suggested; direct
  backlinks are still allowed but labelled by the graph factor, so the user
  can distinguish "already linked" from "semantically close".
- **Decision:** top 5 only. Dismissals are deferred to a later slice; do not
  add hidden state that cannot be rebuilt.

### 26.2 Data flow

`nexora_core::related::suggest(&VaultIndex, &SemanticIndex, &NoteId, limit)`
is pure and tested. The app caches the result keyed by `(target, semantic
fingerprint)` so the GUI does not recompute O(n²) cosine scores every frame.
No sidecar file, no writes, no network.

### 26.3 Verification

Raw `cargo test` including `tests/related.rs`, plus GUI smoke: open a note,
see Related rows with factors, click one, confirm navigation.

Status: implemented 2026-11-22. `nexora_core::related::suggest` combines
lexical cosine (0.5), tag Jaccard (0.25), and graph proximity (0.25), top 5,
with all factors exposed in the UI. The app caches by `(target, semantic
fingerprint)` and invalidates on every index rebuild. Verified: 75 tests pass;
GUI smoke with vectors built showed `b — sim 0.78 · tag 0.00 · graph 0.00` in
the Context panel, `stderr` empty. Code review found no blocking issue; the
zero tag/graph factors are correct for a fixture with no tags and no links.

---

## 27. Amendment — deterministic canvas view (v2-5)

Locked 2026-11-22 before code. No model, no new dependency.

### 27.1 Triad

- **Decision:** canvas positions are plain f64 coordinates in a sidecar under
  app data (`Nexora/canvas.idx`), never inside the vault. Markdown remains the
  only source of truth; the layout is fully rebuildable.
- **Decision:** first layout is deterministic: notes in `NoteId` order placed
  on a grid whose column count is `ceil(sqrt(n))`. No RNG, no wall clock.
- **Decision:** edges come only from resolved wikilinks in `VaultIndex`,
  deduplicated as undirected pairs; they match the graph view's semantics.

### 27.2 Interaction

Cards are draggable; a drag persists positions for the dragged node only.
Clicking a card opens the note. `Reset layout` drops the sidecar and rebuilds
the grid. New notes get positions on the next index rebuild; removed notes are
pruned. No freehand drawing, no zoom beyond fit, no multi-select.

### 27.3 Verification

Raw `cargo test` including `tests/canvas.rs`, plus GUI smoke: open Canvas,
see cards and edges, drag a card, reopen and confirm the position persisted.

Status: implemented 2026-11-22. `nexora_core::canvas` has deterministic grid
layout, reconciliation, resolved-link edges and a text sidecar under app data.
Verified: 83 tests pass; GUI smoke showed three cards with two edges and no
`stderr`. Code review found that `reload_index()` reconciled the canvas even
when `build_index` failed, which would have pruned every position on a
transient read error — reconciliation now happens only on the success branch.
Drag persistence is covered by the core sidecar roundtrip test; the GUI smoke
verified rendering, not a physical mouse drag.

---

## 28. Amendment — nested tags and aliases (v2-6)

Locked 2026-11-22 before code, from ROADMAP goal G3 and the Obsidian parity
overview. Supersedes the "nested tags are not special" half of §7.4; GM-1 (no
exclusions) still holds.

### 28.1 Triad

- **Decision:** `#school/math` is one tag with the full path `school/math`.
  `build_tag_index` also registers every parent prefix (`school`) pointing at
  the same notes, so flat tag filters keep working. The extracted token still
  comes from the raw text; no tag is synthesized from a note's path.
- **Decision:** `aliases:` in frontmatter (inline or block list) gives a note
  alternative `[[...]]` targets. Alias resolution is exact and case-sensitive;
  a real `NoteId` always wins over an alias with the same name.
- **Decision:** aliases live in `VaultIndex.aliases: BTreeMap<String, NoteId>`,
  rebuilt from disk like every other derived index. No writes to notes.

### 28.2 Interaction

Wikilink parsing, backlinks, autocomplete and Quick Open consult the alias map.
Tag chips may show `school/math`; filtering by `school` still matches.

### 28.3 Verification

Raw `cargo test` including `tests/nested.rs`, plus GUI smoke: a note with
`aliases: [Alg]` resolves `[[Alg]]`, and a `#school/math` note appears under
both `school/math` and `school` filters.

Status: implemented 2026-11-22. `VaultIndex.aliases` is rebuilt from
frontmatter; wikilinks, backlinks, autocomplete, Quick Open and Reading view
all resolve aliases, with real `NoteId` taking precedence. Nested tags register
full and parent keys. Verified: 90 tests pass; GUI smoke showed tag chips
`#school`, `#school/math` and the Reading view rendering `see Alg` (not
`missing`) with `Links → target`. Code review caught that `display.rs` still
used exact-id lookup and marked alias links missing — fixed with a test.

---

## 29. Amendment — Bases-lite table view (G2, first slice)

Locked 2026-11-22 before code, from the Obsidian parity goals in `ROADMAP.md`.
No new dependency.

### 29.1 Triad

- **Decision:** a Base is a read-only view over `VaultIndex` plus frontmatter
  fields `created`, `modified`, `tags`, `aliases`. No .base files in the vault;
  the view config lives in app data and is rebuildable.
- **Decision:** v1 supports one config: tag filter, sort by title/path/created/
  modified, and group by one frontmatter key. Table and card renderings share
  the same rows. Editing cells, formulas, kanban, filters on arbitrary fields
  are later slices.
- **Decision:** group-by produces one row per value for list-valued fields
  (e.g. `tags`), preserving the note's identity; this is a display expansion,
  not a duplicate note.

### 29.2 Verification

Raw `cargo test` including `tests/bases.rs`, plus GUI smoke: open Base, change
tag filter and sort, confirm rows reorder and clicking a row opens the note.

Status: implemented 2026-11-22. `bases::build` projects frontmatter into rows
with tag filter, four sort keys and list-valued group-by; config lives in an
app-data sidecar. Verified: 94 tests pass; GUI smoke showed a two-row table
and tag chips. Code review found that notes without `created` sorted before
dated ones, and that typing in filter/group fields was not persisted — both
fixed (empty values sort last; text edits mark config dirty). Editing cells,
formulas, kanban and .base files in the vault remain later slices.

---

## 30. Amendment — unlinked mentions (G3)

Locked 2026-11-22 before code, from the Obsidian parity goals.

### 30.1 Triad

- **Decision:** an unlinked mention is a case-insensitive occurrence of the
  target note's title or any of its frontmatter aliases in another note, with
  line number and a short snippet. It is not a wikilink and not a backlink.
- **Decision:** matches inside `[[...]]` spans, fenced code blocks and
  frontmatter are ignored, so the panel only suggests genuinely unlinked
  prose. The source note itself never lists itself.
- **Decision:** the result is a pure function of `VaultIndex`, computed on
  demand for the active note; no sidecar and no indexing writes.

### 30.2 Interaction

Context panel lists UNLINKED MENTIONS under Backlinks; clicking a row opens the
source note in Source mode with the cursor at the reported line, reusing the
existing health-report jump.

### 30.3 Verification

Raw `cargo test` including `tests/mentions.rs`, plus GUI smoke: open a note,
see a prose mention of its title without `[[...]]`, click it and land on the
right line.

Status: implemented 2026-11-22. `mentions::find` masks wikilinks, frontmatter
and fenced code, matches titles and aliases case-insensitively, and reports
line numbers. Verified: 99 tests pass; GUI smoke showed
`source : algebra (line 2)` in Context. Code review found a deeper bug: a UTF-8
BOM before `# Title` hid the H1, so titles fell back to the file stem and
mentions could not be found — `vault::extract_title` now strips the BOM, with
a regression test. Clicking a row opens the source in Source mode at the line
via the existing `pending_line` jump.

---

## 31. Amendment — note embeds in Reading view (G3)

Locked 2026-11-22 before code.

### 31.1 Triad

- **Decision:** `![[target]]` and `![[target|alias]]` expand to the target
  note's body in Reading view only. The vault bytes and every index are
  untouched; expansion is a pure display transform.
- **Decision:** recursion is bounded: max depth 3 and a visited-note set.
  A cycle renders `![[target]] (embed loop)` instead of recursing forever.
- **Decision:** leading YAML frontmatter of an embedded note is stripped, and
  embedded text goes through the same display adaptation as the host note
  (wikilinks, highlights, callouts). Aliases on embeds are accepted but only
  the resolved note body is shown.

### 31.2 Verification

Raw `cargo test` including embed cases in `tests/display.rs`, plus GUI smoke:
a note with `![[other]]` shows the other note's body in Reading view and a
cyclic pair stops with the loop marker.

Status: implemented 2026-11-22. `display::expand_embeds` runs before display
adaptation, strips embedded frontmatter, bounds recursion to depth 3 with a
visited set, and leaves embeds inside fenced code untouched. Verified: 103
tests pass (4 in `tests/embed.rs`); GUI smoke showed `before / Child / body
text / after` with frontmatter hidden. Code review found that `strip_frontmatter`
returned the YAML lines instead of the body, and unresolved `![[gone]]` was
rewritten as a normal link — both fixed and covered by tests.

---

## 32. Amendment — page preview on note rows (G3)

Locked 2026-11-22 before code.

### 32.1 Triad

- **Decision:** hovering a note row in the Explorer, Context (Links,
  Backlinks, Unlinked Mentions, Related), Base table/cards or Quick Open shows
  a compact preview of the target note: title plus the first non-empty lines.
- **Decision:** the preview is a pure `VaultIndex` projection; no writes, no
  network, no loader state. Frontmatter is excluded.
- **Decision:** inline hover over wikilinks inside the rendered Reading view is
  NOT in this slice: `CommonMarkViewer` does not expose hover targets for
  individual links, and faking hit-testing over its widgets would be brittle.
  This is a scope limit, not a silent omission.

### 32.2 Verification

Raw `cargo test` including `tests/preview.rs`, plus GUI smoke: hover a note row
and confirm the tooltip shows the title and body excerpt.

Status: implemented 2026-11-22. `preview::for_note` strips frontmatter and
returns the first non-empty lines; tooltips are attached to Explorer,
Context, Base and Quick Open note rows via `Response::on_hover_ui`. Verified:
105 tests pass; GUI smoke hovering `alpha` showed `Alpha / # Alpha / first body
line / second body line`. Inline hover over rendered Reading-view links stays
out of scope (§32.1) because `CommonMarkViewer` exposes no hover targets.

---

## 33. Amendment — bookmarks (G3)

Locked 2026-11-22 before code.

### 33.1 Triad

- **Decision:** bookmarks are an ordered list of `NoteId` in an app-data
  sidecar, never written into the vault. Renames update the ids in place;
  deletions prune missing notes on the next index rebuild.
- **Decision:** toggling is a command in the registry (`ToggleBookmark`) with
  a button in the editor header, so the palette and mouse share one path.
- **Decision:** the Explorer shows a BOOKMARKS section above the tree; clicking
  a row opens the note, and the same header button removes it. No drag sort in
  v1.

### 33.2 Verification

Raw `cargo test` including `tests/bookmarks.rs`, plus GUI smoke: bookmark a
note, see it in Explorer, reopen the app and confirm persistence.

Status: implemented 2026-11-22. Ordered sidecar, toggle command, Explorer
section, editor-header button, prune on index rebuild, id rewrite on rename.
Verified: bookmarks tests pass; GUI smoke showed `BOOKMARKS → alpha`, sidecar
contained `NEXORA-BOOKMARKS-v1 / alpha`, and after restart the section and row
reappeared. Code review found that `reload_index()` pruned the old id before
`bookmarks.rename()` ran — rename now happens first.

---

## 34. Amendment — outgoing links panel (G3)

Locked 2026-11-22 before code.

### 34.1 Triad

- **Decision:** a pure `outgoing::for_note` projection lists every resolved and
  broken `[[target]]` of the active note with its line number. No new cache;
  it is derived from `VaultIndex` like backlinks.
- **Decision:** Context panel replaces the old flat "Links" section with
  "OUTGOING LINKS". Resolved rows navigate; broken rows reuse the existing
  "create note at target" command.
- **Decision:** fenced code is skipped, aliases are accepted, and a real
  `NoteId` always wins over an alias — identical rules to §7.3/§28.

### 34.2 Verification

Raw `cargo test` including `tests/outgoing.rs`, plus GUI smoke: open a note,
see resolved and broken outgoing rows with line numbers.

Status: implemented 2026-11-22. `outgoing::for_note` is a pure projection with
line numbers, fenced-code skipping and alias resolution; Context panel shows
`OUTGOING LINKS`. Verified: 111 tests pass; GUI smoke showed `alias · line 2`
and red `gone (missing) · line 2`.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- A link addressed by alias kept the alias as its target id, so clicking tried
  to open a note that does not exist — `target` now stores the resolved
  `NoteId` (real id wins over alias, now covered by a test).
- `outgoing.rs` had a second `[[...]]` parser that disagreed with `wikilink.rs`
  on empty targets (`[[]]`) and `~~~` fences. It now reuses
  `wikilink::parse_links_with_lines` + `wikilink::resolve_target`; one parser
  owns the rules, and the empty-target case has a regression test.
- The broken row only selected a target and printed a hint. It now invokes
  `CommandId::CreateLinkTarget` directly, identical to the palette path.
- Hover previews were attached to outgoing rows (scope creep beyond §34) and
  four note-row blocks were duplicated in the Context panel. Hover previews
  are removed from outgoing rows; the other panels share one `note_row` helper.

Tests after fixes: `tests/outgoing.rs` has 5 cases; full suite total recorded
in `CHECKLIST.md`.

---

## 35. Amendment — authored theme presets (V2-8)

Locked 2026-11-22 before code.

### 35.1 Triad

- **Decision:** one `ThemePreset` enum (Dark, Light, HighContrast) with palettes
  defined as plain RGB tables in `nexora-core::theme`. The app converts them
  to egui `Visuals`; no colour picker and no CSS.
- **Decision:** the chosen preset persists in an app-data sidecar, never in the
  vault. Palette cycle is a command in the registry, so palette and keyboard
  stay consistent with §7.10.
- **Decision:** every preset must keep text/background contrast ≥ 4.5:1 for
  body text; this is asserted in core tests, not eyeballed.

### 35.2 Scope

Presets only: no editor, no custom colours, no per-note theme, no theme
marketplace. Fenced code, selection and links are restyled by the same table.

### 35.3 Verification

Raw `cargo test` including `tests/theme.rs`, plus GUI smoke of each preset:
switch via palette, confirm readable panels and restart persistence.

Status: implemented 2026-11-22. `nexora-core::theme` owns the RGB tables, the
WCAG ratio helper and the sidecar; the app reads the active palette through a
thread-local cache and derives every colour from it (the `App.theme` field is
the persistence source of truth, the thread-local is a render cache). Cycle is
`CommandId::CycleTheme`; no keyboard shortcut is bound because §7.10 makes
shortcuts optional and the registry is the single entry point.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- Body-text contrast is asserted on all five backgrounds the app actually
  paints (`bg`, `panel`, `surface`, `raised`, `accent_dim`); muted labels are
  asserted on the four they actually appear on. The first strict run exposed
  a real 2.56:1 pair (muted on accent_dim) — that pairing does not occur in
  the UI (active/selection foreground is body text), so the test now mirrors
  the real usage instead of asserting a guarantee nobody needs.
- Graph/canvas/health panels still had hard-coded `Color32` literals; they now
  use `line()`, `accent()`, `text()`, `rust()`, `amber()` and `sage()`.
- `egui_commonmark` paints fenced code with `visuals.extreme_bg_color`
  (confirmed in `egui_commonmark_backend-0.26.0/src/elements.rs:229`), not
  `code_bg_color`; `extreme_bg_color` now comes from the same `raised` table
  entry.
- A corrupt sidecar (bad header, missing label or unknown label) used to fall
  back to Dark silently. `ThemePreset::parse` is now strict and `load()` returns
  `InvalidInput`; the app surfaces the error in the status line instead of
  swallowing it.
- The command label no longer duplicates preset names ("Cycle theme"), so a
  new preset does not require editing the command registry.

Verified: 110 tests pass; GUI smoke cycled dark → light → high-contrast via the
palette with readable panels in each, sidecar updated on every step, and a
restart restored high-contrast. `stderr` was empty throughout.

---

## 36. Amendment — templates with a closed placeholder set (V2-7)

Locked 2026-11-22 before code.

### 36.1 Triad

- **Decision:** templates live in an app-data sidecar, never in the vault. A
  `Template` is a name plus a body; the shipped default is a daily-note body.
- **Decision:** the placeholder set is closed: exactly `{{date}}` and
  `{{title}}`. Unknown `{{...}}` sequences are left verbatim — no scripting,
  no evaluation, no user-defined placeholders.
- **Decision:** applying a template is a command in the registry. It creates a
  new note whose body is the rendered template; it never overwrites an
  existing note.

### 36.2 Scope

No template editor in the GUI, no per-vault templates, no automatic rewriting
of existing notes. Templates are read from the sidecar at startup; writing them
is out of scope for v2.

### 36.3 Verification

Raw `cargo test` including `tests/templates.rs`, plus GUI smoke: run the
command, pick the default template, confirm the created note contains a real
date and its own title.

Status: implemented 2026-11-22. Templates are read once at startup from
`APPDATA/Nexora/templates.idx`; a missing sidecar yields the daily default.
`RESOLVERS` is the single definition of the closed set — `render` looks up its
table and the public `PLACEHOLDERS` list is derived from the same table, so the
GUI cannot advertise a placeholder that `render` would leave literal.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- `templates::save()` was a full binary writer used only by tests, while §36.2
  puts writing out of scope. Removed; the sidecar format is now written by
  hand in `tests/templates.rs`, so the parser is still exercised.
- The modal displayed a derived `Placeholders: ...` hint — scope creep beyond
  §36.2's "no template editor". Removed; the window only lists templates.
- `create_from_template` set the success status *before* writing. It now sets
  it only when `write_new_note` succeeds; failures keep their error status.
- The shipped "daily" body used `{{title}}` in H1, so the note was named
  `Untitled` and did not read as a daily note. Template-created notes now use
  the date as their base id, so `# 2026-11-22` and `Created 2026-11-22` both
  come from real values.
- Startup sidecar error handling was duplicated with the theme block. Both
  now go through one `sidecar_or` helper.
- Residual from §34: `wikilink::build_links` still inlined the exact-id-then-
  alias rule that `resolve_target` owns. Both now call one private
  `resolve_in`; the `note_row` doc comment was corrected to state that the
  outgoing panel deliberately opts out of hover previews per §34.

Verified: 114 tests pass; GUI smoke created `2026-11-22.md` with
`# 2026-11-22` / `Created 2026-11-22` and status `Created 2026-11-22 from daily`.
`stderr` was empty.

---

## 37. Amendment — auto-apply a template to a new daily note (V2-9)

Locked 2026-11-22 before code.

### 37.1 Triad

- **Decision:** `Open Today` renders a template into the note body only when
  today's note does not exist yet. An existing note is never rewritten.
- **Decision:** template choice is deterministic: the template named `daily`
  if present, otherwise the first template in the sidecar, otherwise no body.
- **Decision:** both placeholders resolve to today's date (`{{date}}` and
  `{{title}}`), so the daily note is self-identifying. Same closed set as §36.

### 37.2 Scope

No template picker for daily notes, no per-weekday templates, no rewriting of
notes created before this amendment. The command palette still owns the
action; there is no second code path for daily-note creation.

### 37.3 Verification

Raw `cargo test` including a new core test for body-on-create and no-overwrite,
plus GUI smoke: fresh vault → `Open Today` → file has rendered body; a second
`Open Today` leaves it unchanged.

Status: implemented 2026-11-22. `daily::open_or_create_with` keeps the
no-overwrite rule in core; the GUI only calls it and renders the result.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- **Hard (§5):** template choice and placeholder resolution for the daily note
  originally lived in a GUI helper, outside automated tests. Moved to
  `templates::daily_body`, which prefers `DAILY_TEMPLATE`, falls back to the
  first template, then to an empty body. The GUI now makes one core call.
- The `"daily"` literal was replaced by the exported `templates::DAILY_TEMPLATE`
  constant, shared with `builtin()`.
- The same `today_id → Fields → render` pipeline appeared in both the daily
  path and `create_from_template`; with selection moved to core it has one
  owner. `daily_body` has its own core test covering named-template priority,
  first-template fallback, empty case, and both placeholders resolving to
  today's date.

Verified: 117 tests pass; GUI smoke created the daily note with
`# 2026-11-22` / `Created 2026-11-22`, and a second `Open Today` left the file
byte-for-byte unchanged. `stderr` was empty.

---

## 38. Amendment — heading- and block-scoped wikilinks (V2-10)

Locked 2026-11-22 before code.

### 38.1 Triad

- **Decision:** a wikilink target may carry one anchor after `#`:
  `[[Note#Heading]]`, `[[Note#^blockid]]`, `[[Note#Heading|alias]]`. The note
  part is resolved exactly as before; an empty anchor is treated as absent.
- **Decision:** `RawLink`, `Link` and `Outgoing` carry the anchor so no
  projection silently drops it. `rename` rewrites only the note part and
  preserves `#anchor` and alias.
- **Decision:** Reading-view navigation resolves the anchor to a 1-based line:
  a heading by its text, a block by its `^id` marker. Unknown anchors simply
  open the note — they are not validation errors.

### 38.2 Scope

No anchors in aliases, no anchor autocomplete, no anchor-aware backlinks
grouping, no block insertion/editing. Plain `[[Note]]` links keep their exact
behaviour. Anchors are display/navigation metadata, not new note identities.

### 38.3 Verification

Raw `cargo test` including `tests/anchors.rs`, plus GUI smoke: a link with a
heading anchor jumps to that heading; rename rewrites the note part and keeps
the anchor.

Status: implemented 2026-11-22. `wikilink::split_target` is the single owner
of the `#` grammar; the parser, renderer, rename and health all call it.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- `health::line_of_link` compared `NoteId::new("gone#Heading")` against the
  broken target `gone`, so every anchored broken link lost its source line.
  It now calls `split_target`, with a regression test.
- The GUI decoded the whole `nexora://` payload before splitting, so a literal
  `#` in a note id (encoded as `%23`) could be mistaken for an anchor. It now
  splits the encoded string first, then decodes each half.
- Grammar had four independent scanners; all now delegate to `split_target`.
- Added the missing coverage for §38.1/§38.2: unknown anchors keep the link
  resolved and produce no health error; a `#` inside an alias stays part of
  the alias.
- `encode_component` stays public: it is the renderer's encoder for anchors,
  used by both production code and the round-trip test.

**Deliberate deviation (§38.1):** clicking an anchored link switches the tab
to Source view. `pending_line` is consumed only by the egui text editor, so
Source is the only place where the line jump is observable; Reading view has
no anchor-scroll API in `egui_commonmark`. The note still opens on the
resolved note, and the cursor lands on the heading/block line.

Verified: 124 tests pass; GUI smoke opened `target` from
`[[target#Second Heading|go]]` with no "Anchor not found" status and empty
`stderr`.

---

## 39. Amendment — boolean and field search (V2-11)

Locked 2026-11-22 before code.

### 39.1 Interface first

A new additive core function; `search::search` keeps its exact behaviour.

```rust
search::search_advanced(index, "algebra AND geometry")
search::search_advanced(index, "path:school OR [author:Zuro]")
search::search_advanced(index, "NOT draft physics")
```

Stable contract: returns `Vec<SearchResult>`; empty query returns empty;
result order is deterministic by `NoteId`. Unstable: internal tokenisation and
evaluation order. A malformed query yields no results rather than a panic.

### 39.2 Triad

- **Decision:** grammar is a closed set: bare terms, quoted phrases,
  `field:value`, `[field:value]`, and `AND`/`OR`/`NOT`. Implicit AND between
  adjacent operands; precedence `NOT > AND > OR`.
- **Decision:** fields are `path`, `title`, `body`, plus any frontmatter key
  read from `key: value` lines. `path:` matches the vault-relative id.
- **Decision:** the GUI gets a fourth `Query` mode in Search. Keyword,
  Semantic and Hybrid are untouched.

### 39.3 Scope

No regex, no nested parentheses, no field aliases, no query history, no
saving searches. Bookmarks on searches stay deferred.

### 39.4 Verification

Raw `cargo test` including `tests/query.rs`, written test-first; GUI smoke:
switch to `Query`, run a boolean query, see the expected notes.

Status: implemented 2026-11-22. Tests were written first (red: missing
`search_advanced`), then the tokenizer/parser/evaluator was added. All parsing
and evaluation live in core; the GUI only builds result rows.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- An empty quoted phrase `""` produced a term with an empty value, which
  matched every note. `term_from` now rejects empty text, and `[foo]`,
  `[author:]`, `path:` and unterminated quotes are all malformed — the query
  returns no results instead of silently widening.
- An unqualified term skipped the `path` field and frontmatter, so bare
  `school` could not find `school/math`. `field: None` now searches all
  declared fields.
- Added missing coverage: mixed `AND > OR` precedence, deterministic sort by
  `NoteId`, bare-term field search, and the degenerate-query cases above.
- Token construction for quoted phrases, `[field:value]` and bare
  `field:value` shared the same scan/split shape; `scan_closer` and `term_from`
  now own it once.
- The GUI had a second `if search_mode == Query` switch just for the hint;
  mode-specific text now lives with the mode selector table.

Verified: 133 tests pass; GUI smoke switched to `Query`, entered
`algebra AND geometry` and saw exactly `1 results` / `Both - both`. `stderr`
was empty.

---

## 40. Amendment — Note Composer (V2-12)

Locked 2026-11-22 before code.

### 40.1 Triad

- **Decision:** two pure core operations. `extract` removes a 1-based line
  range from a note, writes it into a new note, and inserts `[[new_id]]` at
  the removal point. `merge_content` appends the source body to the target.
- **Decision:** merging rewrites links that pointed at the source using the
  already-tested `rename::rewrite_links` rule, then removes the source file.
  Partial failure is accepted and reported, matching §18's rename semantics.
- **Decision:** both operations are exposed as palette commands with a
  confirmation modal. Nothing is written before confirmation.

### 40.2 Scope

No clipboard, no selection by cursor drag, no image/block cards, no undo beyond
existing per-note snapshots. Line ranges are 1-based and inclusive; an invalid
range is an error, never a silent empty extract.

### 40.3 Verification

Raw `cargo test` including `tests/composer.rs`, plus GUI smoke: extract a line
range and confirm the link is inserted; merge two notes and confirm link
rewriting.

Status: implemented 2026-11-22. All composition logic and both writers live in
`nexora-core::composer`; the GUI only renders the modal and forwards actions.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- **Bug:** `apply_merge` built the merged body from the *raw* target content,
  so `[[source]]` inside the target (and self-links inside the source body)
  survived the merge and became broken links after the source was deleted.
  Both bodies are now passed through `rename::rewrite_links` before merging,
  with a regression test asserting no `[[source]]` remains.
- **Bug:** the source file was deleted without a snapshot. `save::snapshot_note`
  now runs before removal, matching §18's rename order; the test asserts the
  snapshot directory exists.
- The provenance line was a `## Merged from …` heading. Under GM-1 that would
  register tags and mutate the tag graph beyond an append; it is now plain
  text.
- The GUI's merge branch silently dropped the action if the active note
  disappeared between opening the modal and confirming. It now sets an error
  status, like the extract branch.

Verified: 138 tests pass. GUI smoke used `Ctrl+P → Extract lines to new note`
and confirmed `[[extracted-4]]` plus the new file; `Merge note into another`
removed `source.md`, rewrote `[[source]]` to `[[target]]` in a witness note,
and left a snapshot. `stderr` was empty.

---

## 41. Amendment — saved queries (V2-13)

Locked 2026-11-22 before code.

### 41.1 Triad

- **Decision:** a saved query stores only its name and the query *source text*.
  The text is reparsed by `search::search_advanced` on every use, so grammar
  changes can never leave a stale AST in the sidecar.
- **Decision:** the sidecar lives in app data, never in the vault. Names are
  unique; saving an existing name replaces its text in place.
- **Decision:** two palette commands — `Save current query` and
  `Apply saved query`. Applying sets the Search tab to `Query` mode with the
  stored text; it never runs a background search.

### 41.2 Scope

No sidebar pinning, no renaming/deleting in v2, no query folders, no storing
results. An invalid saved query surfaces a visible error rather than a crash.

### 41.3 Verification

Raw `cargo test` including `tests/saved_queries.rs`, written test-first, plus
GUI smoke: save a query, clear the field, apply it again and see the same
results.

Status: implemented 2026-11-22. Tests were written first (red: missing
`saved_queries` module), then `upsert`, sidecar and `run` were implemented.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- `saved_queries::run` only failed on an unknown name: `search_advanced`
  returns an empty vector on malformed syntax, so a broken stored query was
  reported as "Applied" with zero results. Added `search::validate`, which
  reparses without touching the vault; `run` now calls it, and the GUI calls
  it on apply. A malformed saved query is a visible error (§41.2).
- Applying used to run a full `search_advanced` pass and discard it just to
  validate, contradicting §41.1. It now validates only; the Search tab
  reparses the restored text itself on the next frame.
- The chosen name was looked up twice (once in `run`, once to recover the
  text). The apply branch now looks it up once.
- Added coverage for `validate` (empty is valid, malformed is an error) and
  for `run` with malformed stored text.

Verified: 142 tests pass. GUI smoke saved `sq`, restarted the app, applied
`sq` and saw `1 results` with status `Applied saved query sq`. `stderr` was
empty.

---

## 42. Amendment — saved workspaces (G5)

Locked 2026-11-22 before code.

### 42.1 Triad

- **Decision:** a workspace stores only its vault key, name, ordered tab
  descriptors `(kind, id)` and the active index. No panel widths, no caret,
  no unsaved buffers — all of those stay rebuildable UI state.
- **Decision:** workspaces live in an app-data sidecar, keyed by vault root,
  never in the vault. Saving an existing name in the same vault replaces it.
- **Decision:** `Save workspace` and `Apply workspace` are palette commands.
  Applying rebuilds tabs from core descriptors; missing notes are skipped and
  the active index is clamped.

### 42.2 Scope

No automatic saving on exit, no workspace picker in the sidebar, no panel-size
persistence, no per-note view mode in v2.

### 42.3 Verification

Raw `cargo test` including `tests/workspace.rs`, plus GUI smoke: save a
workspace, close its tabs, apply it, confirm the tabs and active note return.

Status: implemented 2026-11-22. `TabKind::KINDS` is the single table for the
persisted kind spelling, so `label` and `parse` cannot drift.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- The saved `active` index was captured and serialized but ignored on apply:
  `open_note` leaves active at the last rebuilt tab. Added
  `workspace::remap_active(kept, active)`, which maps the saved index over the
  descriptors that actually survived. A missing note before the saved active
  tab no longer activates the wrong note; the helper has a core test.
- An unknown tab kind set an error but still reported `Applied workspace …`.
  Success status is now emitted only when no descriptor was malformed. Missing
  notes remain a legal skip per §42.1.
- `filter("workspace")` is now asserted in `tests/commands.rs` so dropping a
  command from the registry while fixing the count cannot pass silently.
- Fixed the contradictory `ROADMAP.md` status for G5.

Verified: 145 tests pass. GUI smoke saved a two-tab workspace with `b` active,
closed a tab, restarted, applied the workspace, and UIA showed tab `a` Off and
tab `b` On. `stderr` was empty.

---

## 43. Amendment — block-level embeds (G3)

Locked 2026-11-22 before code.

### 43.1 Triad

- **Decision:** `![[note#Heading]]` and `![[note#^blockid]]` embed only that
  section or line. Heading sections end at the next heading of the same or
  higher level, or at EOF.
- **Decision:** anchors use the same closed grammar as §38. An unknown anchor
  leaves a visible `![[note#anchor]] (anchor not found)` marker — never a
  silent full-note embed.
- **Decision:** depth 3 and the visited set from §31 are unchanged; an
  anchored embed that contains another embed follows the same rules.

### 43.2 Scope

No embedding headings in aliases, no block insertion/editing, no external
media, no changes to whole-note embeds. This remains a display-only projection:
the vault is never written and indexes never see expanded text.

### 43.3 Verification

Raw `cargo test` including `tests/embed.rs`, plus GUI smoke: a Reading view
with `![[note#Heading]]` shows only that section and a bad anchor shows the
marker.

Status: implemented 2026-11-22. No GUI changes were needed: Reading view
already calls `display::adapt_for_display`, which now understands anchors.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- The amendment was accidentally inserted twice, once before §38 and once at
  the end. The misplaced copy was removed so §43 appears only once and the
  amendment numbers stay monotonic.
- Block-anchor resolution was re-implemented in `slice_by_anchor`; it now
  delegates to `outline::line_for_anchor`, the owner since §38.
- `slice_by_anchor` was public without an external caller; it is private.
- **Bug:** embedded bodies were expanded with the single-line helper, which
  has no fenced-code guard. An `![[x]]` inside a fence within an anchored
  section was expanded. A shared `expand_body` now performs the line-wise,
  fence-aware expansion at every depth, with a regression test.
- Added coverage for nested embeds inside an anchored section, fenced code
  inside that section, higher-level heading termination with retained
  subheadings, and the block/anchor cases.

Verified: 151 tests pass. GUI smoke showed only `bravo` from
`![[child#Second]]` (no `alpha`/`charlie`) plus the `(anchor not found)`
marker for `![[child#Missing]]`; `stderr` was empty.

---

## 44. Amendment — offline CLI (G6)

Locked 2026-11-22 before code.

### 44.1 Triad

- **Decision:** a thin `nexora-cli` binary calls the exact same core functions
  as the palette; no logic is duplicated outside `nexora-core`.
- **Decision:** commands are `search`, `query`, `rename` and `daily:path`.
  `rename` is dry-run by default and writes only with `--apply`.
- **Decision:** stdout is deterministic (`id \t title` per result, one path for
  `daily:path`); diagnostics go to stderr with a non-zero exit code.

### 44.2 Scope

No `nexora://` URI registration, no interactive TUI, no config flags, no
network, no keys. The CLI never touches app data except through core paths.

### 44.3 Verification

Raw `cargo test` unchanged plus a raw CLI smoke: each command run against a
temp vault with its stdout pasted verbatim.

Status: implemented 2026-11-22. `nexora-cli` depends only on `nexora-core` and
contains no business logic; argument parsing and stdout formatting are the
only code outside core.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- An unrecognised 4th token (e.g. `rename old new --bogus`) was silently
  treated as dry-run and exited 0. Arity and the flag are now validated
  strictly; bad invocations print a diagnostic plus usage to stderr and exit 1.
- `vault_root()` was a pure pass-through to `vault::default_vault_root`;
  removed. Usage text is built in one place (`bad_args`).
- Rename reported the raw `new` argument after applying. It now reports the
  canonical `plan.new` (the id actually written) in both dry-run and apply.

Verified: 151 tests pass, `cargo build` clean. Raw CLI smoke against a temp
vault:

```text
$ nexora-cli daily:path
...\nexora-cli-final\Documents\Nexora\my-first-vault\2026-11-22.md
$ nexora-cli search algebra
a	A
$ nexora-cli query 'algebra AND geometry'
$ nexora-cli rename old new
rename	old	new
witness	1
$ nexora-cli rename old new --apply
renamed	new
$ nexora-cli rename old new --bogus
unknown flag: --bogus
usage: nexora-cli <search|query|rename|daily:path> [args]
exit=1
$ nexora-cli daily:path extra
daily:path takes no arguments
usage: nexora-cli <search|query|rename|daily:path> [args]
exit=1
```

---

## 45. Amendment — local image embeds (G4)

Locked 2026-11-22 before code.

### 45.1 Triad

- **Decision:** `![[pic.png]]` embeds an image file from the vault. A closed
  extension set is recognised: `png`, `jpg`, `jpeg`, `gif`, `webp`, `svg`.
- **Decision:** core only extracts safe vault-relative image targets; the app
  reads the bytes and registers them with `ctx.include_bytes` under a
  `bytes://nexora/...` URI that the renderer displays. No base64, no network.
- **Decision:** a missing or unsafe image leaves a visible
  `![[target]] (image not found)` marker. External image URLs stay blocked.

### 45.2 Scope

No image editing, no galleries, no drag-and-drop, no remote images, no SVG
scripting filter beyond egui's loader. Whole-note and anchored embeds are
unchanged.

### 45.3 Verification

Raw `cargo test` including `tests/images.rs`, plus GUI smoke: a note with
`![[pic.png]]` renders the image; a missing file shows the marker.

Status: implemented 2026-11-22. `display::embed_spans` / `scan_lines` /
`embed_inner` are the single owners of the `![[...]]` and fence grammar;
`images` only filters image targets, and the app only reads bytes.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- `images.rs` had a second `![[...]]` scanner with its own fence rule, exactly
  the drift §28 had already fixed for wikilinks. Removed: it now delegates to
  `display::embed_inner`, and `expand_body` / `embed_inner` share `scan_lines`.
- Aliased embeds such as `![[pic.png|300]]` were silently left unrendered
  because the app rebuilt the source occurrence without the alias. `targets`
  now returns verbatim inners; the app splits off the alias and replaces one
  occurrence at a time (so repeated embeds each get one marker/image).
- `images::targets` now filters unsafe paths in core; `all_targets` keeps them
  so the app can show a visible marker. The safety assertion in the test is a
  direct assertion, not a masking disjunction.
- Images introduced by a whole-note/anchored embed appeared after the image
  pass and stayed raw. Image preparation now runs on the expanded text.
- `DefaultBytesLoader` keeps the first bytes registered for a URI. The `bytes://`
  URI now carries the file's mtime as a version, so a replaced image refreshes.
- Fenced-code lines are still emitted verbatim by `expand_body` after the
  `scan_lines` refactor; the existing fence tests cover this.

Verified: 155 tests pass. GUI smoke rendered one image (including an image
inside an embedded note) and showed the `(image not found)` marker; `stderr`
was empty.

---

## 46. Amendment — SM-2 resurface queue (V3-3)

Locked 2026-11-22 before code.

### 46.1 Triad

- **Decision:** pure SM-2 scheduling lives in `nexora-core::review`: grade
  0–5 maps to Again/Hard/Good/Easy; only grades ≥ 3 advance repetitions.
- **Decision:** state is stored as flat frontmatter keys `review_due` (date),
  `review_ease`, `review_interval`, `review_reps` — the existing frontmatter
  writer only supports `key: value`, so the nested `review: {...}` form from
  the roadmap sketch is a deliberate deviation.
- **Decision:** the queue is a pure projection of the index: notes whose
  `review_due` is today or earlier, sorted by date then id. A note without
  review fields is not in the queue.

### 46.2 Scope

No cards, decks, streaks, statistics or automatic writes. Grading goes through
the normal frontmatter/snapshot path.

### 46.3 Verification

Raw `cargo test` including `tests/review.rs`, plus GUI smoke: a due note
appears, grading it moves `review_due` into the future and removes it from the
queue.

Status: implemented 2026-11-22. All SM-2 logic and the queue projection live in
`nexora-core::review`; the GUI only renders the two windows and forwards grades.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- Fresh-card defaults (`ease 2.5`, `interval 0`, `reps 0`) were built in the
  GUI. Added `review::new_card(date)` so the domain default has one owner.
- Malformed review frontmatter was treated as a fresh card and silently
  overwritten on grade. `review::has_fields` now distinguishes "no review
  fields" from "invalid review fields"; the latter sets an error and writes
  nothing.
- `due_queue` required all four fields to parse, so a due note with a broken
  `review_ease` vanished from the queue. Membership now depends only on a
  parseable `review_due <= today`, as §46.1 states.
- The GUI derived today's date from the daily-note id and repeated the same
  error text in two places. Added `review::today()` and removed the duplication.
- Grades are named constants (`AGAIN`, `HARD`, `GOOD`, `EASY`) owned by core.
- Added tests for the boundary grade 3 (advances) versus grade 2 (resets) and
  for queue membership with malformed secondary fields.

Verified: 161 tests pass. GUI smoke showed the due note `card`, graded it Good,
saw `review_due: "2026-11-28"`, and the queue then reported `Nothing due today`.
`stderr` was empty.

---

## 47. Amendment — safe workspace apply (hardening)

Locked 2026-11-23 before code.

### 47.1 Triad

- **Decision:** Apply workspace flushes every dirty note tab through the same
  `flush_tab` path as autosave before `tabs.clear()`. A tab still dirty after
  the flush (conflict or write error) aborts the apply: all open tabs stay,
  the conflict/error state remains visible, and no descriptor is applied.
- **Decision:** no confirmation dialog for the clean case. A successful flush
  leaves no unsaved edits, so there is nothing to confirm discarding. Abort on
  failure is preferred over "confirm discard": the user keeps the buffer and
  can resolve the conflict, while snapshots cannot help a note never written.
- **Decision:** the success status reports skipped descriptors honestly instead
  of always claiming a clean apply. A malformed descriptor still sets `error`
  and must not be overwritten by a success status.

### 47.2 Scope

Only the Apply workspace branch in `nexora-app`. No change to workspace
storage, descriptor validation, the §42.1 active-index remap, or the core
`workspace` module. Autosave cadence and snapshot policy stay as they are.

### 47.3 Verification

`cargo test --workspace` stays at 161 passing. GUI smoke in an isolated vault:
type into a note, apply a workspace, confirm the edit reached disk and the
workspace tabs opened; then force an external change and confirm the apply
aborts with the note still open.

### 47.4 Review findings closed during verification

Two-axis review of the preceding UI polish pass and this pass's GUI smoke
closed these before accepting the change:

- Conflict Reload/Overwrite acted on `self.active`; a conflict on a non-active
  tab could therefore copy or write another note's body. Both now resolve the
  tab by the conflict note id, and the buttons are hidden if that tab is gone.
- Enter in the command palette now consumes the key when a command is chosen;
  without it, a modal opened in the same frame auto-confirmed an empty form.
- The tab close `×` is drawn only while the pointer is over the tab or the
  button region; active tabs keep it visible.
- The status bar reuses the snapshot list already read by the Context panel
  instead of a second filesystem read, and truncates long conflict ids by
  Unicode characters so a multibyte id cannot panic a byte slice.
- Named `Heading2`/`Heading3` text styles were dead config: `egui_commonmark`
  interpolates heading sizes from H1/Body and has no per-level hook. They were
  removed and DESIGN §6 records the measured sizes as a limitation.

Verification review closed four more findings before acceptance:

- The abort message replaced the concrete I/O error recorded by `flush_tab`;
  it now prefixes that cause, or uses a generic phrase only on the conflict
  path where no error text exists.
- `consume_key(NONE, Enter)` missed Ctrl/Cmd+Enter while `input_enter` detects
  every modifier. The palette now drops all pressed Enter events via
  `events.retain`, matching the link-popup pattern.
- The tab `×` hit region was 18 px wide but the button starts after item
  spacing and carries button padding; the region now derives from
  `ui.spacing()` so the button cannot vanish under the cursor.
- CHECKLIST no longer lists the workspace confirm dialog as deferred: §47
  chose abort-on-failure instead.

Status: implemented 2026-11-23. `cargo test --workspace`: 161 passed, 0 failed.
GUI smoke in the isolated vault typed `SMOKE47-MARKER-FLUSH47`, applied the
saved `smoke` workspace, and confirmed the marker on disk plus the rebuilt tabs
and `Applied workspace smoke` status. Palette Enter no longer auto-confirms a
modal opened in the same frame; the tab `×` is visible only on the active row.
The conflict-abort branch was verified by code review, not by GUI timing: the
700 ms autosave and 1 Hz poll make manual reproduction non-deterministic.

---

## 48. Amendment — modal error recovery (hardening)

Locked 2026-11-23 before code.

### 48.1 Triad

- **Decision:** form modals (Save query, Save workspace, Note Composer) stay
  open on validation or write failure and show the reason inline in `rust()`.
  They close only on success or explicit Cancel/Escape. Input fields live on
  `App`, so keeping the window open preserves what the user typed.
- **Decision:** `self.error` remains the single owner of the failure text; the
  modal renders a snapshot of it, and the status bar keeps showing the same
  message. No second error field is introduced.
- **Decision:** this applies to Apply query as well — a failed lookup keeps the
  selection window open instead of reporting only in the status bar.

### 48.2 Scope

Only the modal branches in `nexora-app/src/main.rs`. No change to core storage,
the saved-query/workspace file formats, or command routing.

### 48.3 Verification

`cargo test --workspace` stays at 161 passing. GUI smoke: open Save workspace,
press Save with an empty name, confirm the window stays open with the inline
error; type a name and confirm it then saves and closes.

Status: implemented 2026-11-23. `cargo test --workspace`: 161 passed, 0 failed.
GUI smoke pressed Enter on an empty Save-workspace form: the window stayed open
with `Workspace name is empty` rendered inline and in the status bar; typing
`new-smoke` then Save closed it and set `Saved workspace new-smoke`. The
inactive-tab `×` was clicked successfully after hovering its row.

Two-axis code review closed one finding: a successful retry left the old
failure text in `self.error`, so the status bar coloured the success message
rust and printed the stale error. All success arms (Save query, Save workspace,
Apply query) now clear `self.error`; the composer already did so through
`reload_index`. The command branches that open these modals and Apply workspace
also clear `self.error`, so an unrelated earlier failure cannot render inline
as if the new modal had failed. A second GUI smoke reproduced the fail-then-
success sequence and confirmed the status bar shows only `Saved workspace
retry-ok` with no stale error.

---

## 49. Amendment — create target from Health report

Locked 2026-11-23 before code.

- **Decision:** a broken-link issue in the Health tab gets a `create` button
  next to `open`. It reuses `selected_broken` and the existing
  `CommandId::CreateLinkTarget` handler; no second creation path is added.
- **Decision:** `draw_health` stays a pure projection over `health::check`; it
  returns the requested target and the GUI shell executes the command after
  the panel closes, preserving the thin-shell boundary.
- **Decision:** the button appears only when the issue has a `target` that is
  absent from the index; other issue kinds keep the existing row.

- **Decision:** `save::write_note` creates missing parent folders before the
  temp write. A `NoteId` may contain `/`, so a broken link to
  `folder/missing` must be createable; the id is already validated by
  `note_path`. Covered by `tests/m1.rs::write_note_creates_parent_folders_for_nested_ids`.

Verification: `cargo test --workspace` must pass with the new nested-write test.
GUI smoke in the isolated vault creates a note containing `[[missing-target]]`,
opens Health, clicks `create`, and confirms the new note exists and the broken
row disappears.

Status: implemented 2026-11-23. `cargo test --workspace`: 162 passed, 0 failed.
GUI smoke created `missing-target.md`, showed `Created missing-target`, and the
broken-link row disappeared. Review added a `seen` set so repeated links to the
same target render one `create` button, cleared `selected_broken` after creation,
and added the nested-parent fix above.

---

## 50. Amendment — empty link-autocomplete state

Locked 2026-11-23 before code.

- **Decision:** when `[[partial` has no matching notes, keep `LinkPopup` alive
  with an empty `suggestions` list and render one muted `No matching notes`
  row, matching Quick open's feedback. Previously the popup was dropped and
  the user got no response.
- **Decision:** keyboard handling must treat an empty list safely: no arrow
  wrap by zero, Enter/Tab commits nothing, Escape still dismisses. The popup
  closes when the `[[` prefix is no longer under the cursor, as before.

Verification: `cargo test --workspace` stays at 162 passing. GUI smoke types
`[[zzz` in a note and confirms the hint appears; typing `[[Alg` still shows
ranked suggestions and Enter inserts the link.

Status: implemented 2026-11-23. `cargo test --workspace`: 162 passed, 0 failed.
GUI smoke showed `No matching notes` for `[[zzz`, ranked suggestions for
`[[Alg`, and Enter inserted `[[Algebra]]`; the popup closed afterwards.

Two-axis review closed four findings:

- Escape and the window `×` cleared the popup for one frame only, because the
  caret still sat after `[[`. `LinkPopup` now has a dismissal anchor
  (`start`, `partial`) on the app; a dismissed anchor is not recreated until
  the prefix changes. GUI smoke held Escape for 10 frames without reappearance.
- `selected` was clamped to a magic `7`; it is now clamped to the actual
  suggestion count, so Enter cannot silently no-op on a shorter list.
- Keys were captured only when a popup existed from the previous frame, so
  same-frame Enter could insert a newline. Capture now derives from
  `partial_at` over the editor state, excluding a dismissed anchor.
- A popup survived tab/view switches and could insert into the wrong tab;
  it is now cleared whenever the active tab is not Note in Source mode. GUI
  smoke confirmed the popup disappears on a toolbar switch and reopens when
  a fresh `[[` is typed.

---

## 51. Amendment — empty review-queue hint

Locked 2026-11-23 before code.

- **Decision:** when the SM-2 due queue is empty, the status bar tells the user
  how to get a due note instead of only saying `Nothing due today`:
  `Nothing due today — add review_due: YYYY-MM-DD to frontmatter`.
- **Decision:** no new modal or state; the queue command keeps its current
  behavior (no window when nothing is due).

Verification: `cargo test --workspace` stays at 162 passing; GUI smoke triggers
`Review due notes` with no due notes and reads the hint in the status bar.

Status: implemented 2026-11-23. `cargo test --workspace`: 162 passed, 0 failed.
GUI smoke via palette `Review due notes` showed
`Nothing due today — add review_due: YYYY-MM-DD to frontmatter` in the status
bar with no modal opened. Standards review confirmed the key and date format
match `review::today()` / §46.1 and that no GUI state or visual token changed.

---

## 52. Amendment — remap active over surviving tabs

Locked 2026-11-23 before code.

- **Decision:** `workspace::remap_active` clamps the saved index to the number
  of surviving tabs, not the number of descriptors. The previous bound
  (`kept.len() - 1`) allowed `[true, false]` with `active = 1` to return `1`
  although only index `0` existed, so no tab was active after Apply workspace.
- **Decision:** the position of a survivor is still the count of survivors
  before it; only the upper bound changes.

Verification: `cargo test --workspace` must pass with a new case covering a
skipped last descriptor. GUI smoke: apply a workspace whose active note was
deleted and confirm a remaining tab is active.

Status: implemented 2026-11-23. `cargo test --workspace`: 162 passed, 0 failed.
GUI smoke deleted `Geometry.md`, applied the `smoke` workspace, and confirmed
`Applied workspace smoke · 1 tabs skipped` with the surviving `Algebra` tab
active. Standards review proved the clamp is a no-op when the saved active
descriptor survived, so §42.1 ordinal semantics are unchanged. Spec review
added regression guards for all-missing descriptors, an out-of-range saved
index and an active descriptor missing in the middle with a later survivor.

---

## 53. Amendment — grade write parity and review-state safety

Locked 2026-11-23 before code.

- **Decision:** every Nexora write refreshes the tab buffers it affects.
  `apply_grade` is no exception: after `save::write_note` succeeds it sets
  `content = loaded = updated`, clears `dirty`/`last_edit` and records
  `last_saved`, exactly like `flush_tab`. This prevents the editor from showing
  stale `review_due` and stops our own grade from raising a GM-4 conflict.
- **Decision:** grading a dirty tab is refused with an explicit status
  ("Save or discard changes before grading"). A grade is computed from the
  saved index version, never from unsaved buffer text, so no user edit is
  silently overwritten.
- **Decision:** `review::has_fields` treats any of the four `review_*` keys as
  review state. A note with `review_ease` but no `review_due` is malformed, not
  a fresh card; its other values must not be overwritten on grade.
- **Decision:** `review::grade` bounds the computed interval before doing date
  arithmetic. User-authored frontmatter (`review_ease: inf`, huge
  `review_interval`) must produce a controlled error, never a chrono overflow
  panic inside core.
- **Decision:** remove `review::from_day_id`, dead since §46 replaced the
  daily-id date derivation with `review::today()`.

Verification: `cargo test --workspace` plus `tests/review.rs` cases for partial
review fields and bounded grading. GUI smoke: grade a clean open note and
confirm the editor frontmatter updates with no conflict banner; grade a dirty
note and confirm the explicit refusal.

Status: implemented 2026-11-23. `cargo test --workspace`: 165 passed, 0 failed.
`tests/review.rs` covers partial review fields and hostile frontmatter; the
embedded depth-3 bound with anchored embeds is covered in `tests/embed.rs`.
GUI smoke graded a clean open note: the editor frontmatter changed to
`2026-11-29`, status reported `Graded card`, and no conflict banner appeared.
Standards review added GM-4 conflict recovery for grade writes and unified
post-write tab refresh in `refresh_tab_after_write`; Spec review added a
`review_due`-only malformed-field assertion.

---

## 54. Amendment — write-path parity (hardening)

Locked 2026-11-23 before code.

- **Decision:** every write to an *existing* note refreshes every open tab for
  that note through `refresh_tab_after_write` and records `last_saved`.
  `save_properties` and `confirm_restore` stop doing inline tab surgery; new-note
  creation keeps `open_note` because there is no prior buffer to refresh.
- **Decision:** `save::restore_snapshot` returns the exact bytes it wrote. The
  GUI refreshes from those bytes instead of a possibly stale `VaultIndex`, so
  the editor cannot drift from disk after a restore.
- **Decision:** all guarded writes (autosave, grade, properties, restore) route
  `NexoraError::Conflict` through one `record_conflict` helper: GM-4 banner with
  the disk content and Reload/Overwrite. Properties and restore keep their
  modal open on failure per §48.
- **Decision:** the restore dirty guard checks every tab for the note, not only
  the active tab; no snapshot can be restored while any buffer for that note is
  unsaved.

Verification: raw `cargo test --workspace`, including byte-exact restore
assertions in `tests/m1.rs`; GUI smoke: save properties on an open note, then
restore the newest snapshot and confirm the editor shows the pre-edit bytes and
a `Restored …` status.

Status: implemented 2026-11-23. `cargo test --workspace`: 166 passed, 0 failed.
`restore_snapshot_reverts_content` now also asserts that the returned bytes
equal the reloaded disk content; `restore_snapshot_is_byte_exact_for_bom_crlf_
and_utf8` covers BOM, CRLF and Cyrillic text. GUI smoke saved `tags: [smoke]`
through the Properties modal (tab refreshed in place, one snapshot created),
then restored the newest snapshot: the editor returned to the pre-edit bytes and
the status read `Restored 20261123-022007773882800`; the on-disk note matched the
editor. GM-4 conflict recovery is now shared by autosave, grade, properties and
restore through `record_conflict`; no inline tab refresh remains in those paths.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- `record_conflict` used `unwrap_or_default()` for a failed disk read, which
  could show empty "disk content" in the Reload/Overwrite banner. It now
  surfaces the read error, clears the banner and says the disk is unreadable.
- A second tab becoming dirty after the snapshot modal opened refused the
  restore but reopened the modal with `dirty == false`, so the inline warning
  and disabled Restore button were absent. The commit guard now marks the
  modal dirty before restoring it.
- The status blocks and the progress checklist were reconciled with the
  implementation and the measured test count.

---

## 55. Amendment — plain-text list mechanics (G4 slice)

Locked 2026-11-23 before code.

- **Decision:** `notes::lists` owns pure, char-indexed transformations over
  plain text: `on_enter`, `indent`, `toggle_task`. No I/O, no GUI, no markdown
  AST — the existing pipe-table/daily formats stay untouched.
- **Decision:** Enter on a list item continues its marker with the same
  indentation; ordered items increment; tasks continue unchecked; an empty
  item exits the list. No transformation inside a fenced code block.
- **Decision:** Tab / Shift+Tab indent or outdent only when the caret line is a
  list item, by two spaces per level; the caret follows the edit. Multi-line
  selection handling is out of scope for this slice.
- **Decision:** Ctrl+Enter toggles `- [ ]` / `- [x]` on the caret line. The GUI
  consumes the key it handled so egui's `TextEdit` cannot double-apply it; the
  `[[` autocomplete keeps priority over Enter/Tab.

Verification: raw `cargo test --workspace`, including `tests/lists.rs` for
continuation, exit, ordering, tasks, fences, BOM, CRLF, indentation and toggle;
GUI smoke in Source mode: continue a list, outdent it, toggle a task, confirm
the bytes on disk after autosave.

Status: implemented 2026-11-23. `cargo test --workspace`: 183 passed, 0 failed.
`tests/lists.rs` covers 17 cases, including BOM before the first item and fence,
BOM-preserving indent/exit, dot-ordered items, indented code fences and a caret
after a trailing `\r` in CRLF files. GUI smoke continued `- [ ] first` into
`- [x] second` (typed, toggled with Ctrl+Enter), indented it with Tab and
outdented it with Shift+Tab; the autosaved file matched the editor bytes.
Scope limits: no multi-line selection indentation, no AST, no new keybinding
outside the Source editor.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- A BOM before a fenced code block defeated fence detection, so list keys could
  edit code inside the fence. `in_fence` now strips a leading BOM explicitly.
- `indent`/`outdent` and empty-item exit anchored at the raw line start, moving
  or deleting the BOM. `Item::content_start` is now the BOM-aware base.
- Added direct coverage for `.`-ordered items and for `indent` inside a fence.

---

## 56. Amendment — editable Base cells (G2 slice)

Locked 2026-11-23 before code.

- **Decision:** `bases::edit_cell` is the only frontmatter mutation for Base
  views. It reuses `frontmatter::write_field` / `write_list`; `tags` and
  `aliases` are comma-split lists, every other editable column is a scalar.
  Unknown frontmatter keys, BOM, CRLF and body bytes are preserved.
- **Decision:** editable columns are `created`, `tags`, `aliases` plus a
  non-empty `group_by` key that is not one of those and is not `modified`.
  `modified` is a file-derived timestamp and is display-only.
- **Decision:** a cell edit is a guarded note write through `save::write_note`
  with the current disk content as the conflict anchor. It creates a snapshot,
  reloads the index, and refreshes every open tab for that note through
  `refresh_tab_after_write`. Dirty tabs for the target note refuse the edit
  with `Save or discard changes before editing in Base`.
- **Decision:** editing is a single-cell modal-in-grid action: clicking a cell
  selects it, Enter or Apply commits, Escape or Cancel discards. There is no
  inline multi-cell spreadsheet state. Grouped rows sharing a note edit the
  same frontmatter key; the group view rebuilds after the write.
- **Decision:** no new dependency, no change to the read-only projection,
  no theme/palette/type-scale change, no new palette command. The existing
  `base_grid` id is reused; cell editors use stable `Id::unique` salts.

Verification: raw `cargo test --workspace`, including `tests/bases.rs` for
cell edits and lossless frontmatter, GUI smoke editing a scalar and a list cell
in the Base table, confirming disk bytes, snapshot, index refresh and refusal
on a dirty tab.

GUI smoke: Base table on the isolated vault, clicked the `created` cell of
`Algebra`, replaced its value, committed with OK — on disk
`created: "2027-05-01"` and one new snapshot; clicked `tags`, committed
`school, exam` — on disk `tags: ["school", "exam"]`; edited `aliases`, pressed
Escape — the on-disk aliases stayed unchanged. The table rebuilt from the
refreshed index after both commits. Note: MCP `type_text` appends at the caret
rather than replacing a selection, so the smoke focused the editor, moved to
End and cleared it with Backspace before typing.

Status: implemented 2026-11-23. `cargo test --workspace`: 188 passed, 0 failed.
`tests/bases.rs` covers scalar/list edits, no-op byte identity, unknown keys,
BOM/CRLF preservation, editable-column derivation, group-key projection and
retention of notes without the grouping field (including a padded key).
`frontmatter::assemble` now preserves the line endings of the consumed `---`
delimiters; without this, every Base/Properties write converted a CRLF note's
frontmatter boundaries to LF.

Two-axis code review (Spec + Standards) closed these findings:

- Grouped list rows share `NoteId`, so `BaseEdit` now also carries the group
  value and the editor id salt includes it; otherwise two rows drew widgets
  with the same egui `Id` and competed for one edit. GUI smoke with `group_by:
  tags` opened exactly one editor for a duplicated note.
- The editor now requests focus when it appears: select-then-type and
  Enter-to-commit previously went to whichever widget had focus. GUI smoke
  confirmed typing and Enter commit after a single cell click.
- A note without the `group_by` field was silently dropped from the table;
  grouping now falls back to one empty group. The projection also trims the
  group key, matching the edit path.
- A GM-4 conflict discarded the typed cell value while other failures kept it;
  the conflict arm now restores the edit as well.
- `group_editable` now comes from core `editable_columns` instead of a second
  GUI rule, and `base_edit` is cleared centrally whenever the active tab is
  not Base (covers tab-strip clicks, closes and workspace applies).
- Verification review found two more ID-collision paths: repeated values in a
  list group (`tags: [school, exam, school]`) and a group column sharing its
  frontmatter key with a normal column (`group_by: created`). `BaseEdit` now
  carries both a per-row occurrence and a column discriminator; both are part
  of the editor id salt and the `editing` predicate. GUI smoke with duplicate
  tags and with `group_by: tags` opened exactly one editor per gesture.
- The table now has a horizontal `ScrollArea` (`base_grid_scroll`) so the
  added `aliases` column cannot push the group cell outside the panel.

---

## 57. Amendment — plain-text pipe-table mechanics (G4 slice)

Locked 2026-11-23 before code.

- **Decision:** `notes::tables` owns pure, char-indexed transformations over
  pipe-table rows: `on_enter` and `on_tab`. It reuses the shared character-indexed
  edit result from §55 (`ListEdit`, aliased as `TableEdit`) and the existing
  `line_bounds` / `in_fence` helpers made `pub(crate)`; no new result type, no
  AST, no filesystem or GUI types in core.
- **Decision:** Enter on a non-empty row inserts a new data row directly below
  with the same cell count, leading/trailing pipes and per-cell padding style as
  the current row. The caret lands in the corresponding cell of the new row.
  An empty row is replaced by a blank line instead, so Enter leaves the table.
  Alignment markers (`:---:`) are never copied into a data row.
- **Decision:** Tab / Shift+Tab move between cells on the caret row; Tab on the
  last cell appends one empty cell. Shift+Tab on the first cell is a no-op.
  Escaped pipes (`\|`) never count as separators. A row needs at least two
  cells to be treated as a table row.
- **Decision:** §55 list mechanics keep priority: the GUI tries `lists` first
  and only calls `tables` when the list transformation returns `None`. The `[[`
  autocomplete still has highest priority. Source rendering and the Reading view
  are unchanged; keys handled here are consumed so `TextEdit` cannot double-apply
  them.
- **Decision:** BOM, CRLF and leading indentation of the row are preserved; all
  cursor offsets are character indices, matching egui.

Verification: raw `cargo test --workspace` including `tests/tables.rs` for
continuation, exit, caret column, escaped pipes, separator rows, Tab navigation,
BOM and CRLF; GUI smoke in Source mode with the isolated vault confirming the
bytes on disk after autosave.

Status: implemented 2026-11-23. `cargo test --workspace`: 204 passed, 0 failed.
`tests/tables.rs` covers 16 cases: row continuation, same-column caret,
alignment exclusion, escaped pipes, exit, indentation, per-cell padding,
CRLF/BOM, trailing whitespace and Tab navigation. GUI smoke in the isolated
vault: Enter added a row under `| one | two |`, Tab appended cells, Shift+Tab
only moved the caret, and autosaved bytes retained the BOM. A cursor-only Tab
no longer marks the note dirty.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- The new row dropped leading indentation and reused one padding style for all
  cells; `Row::indent` and per-cell `Cell::pad_before`/`pad_after` now preserve
  both. Trailing whitespace no longer creates a phantom cell.
- Exiting an empty row removed the BOM and the `\r` of CRLF. The BOM-aware
  `Row::base` and the raw `content_end` are now used, with byte-exact coverage.
- Shift+Tab on the first cell returned `None`, letting egui's native outdent
  run; it now returns a handled no-op edit. The app sets `dirty` only when the
  text actually changes, so cursor moves never trigger autosave.

---

## 58. Amendment — custom Base columns (G2 slice)

Locked 2026-11-23 before code.

- **Decision:** `BaseConfig` gains `extra_columns: Vec<String>` with core-owned
  `add_column` / `remove_column` validation. Keys are trimmed; empty keys,
  commas, newlines, `modified`, `created`, `tags`, `aliases` and duplicates are
  rejected. `group_by` is not rejected but is deduplicated in
  `editable_columns`, so one frontmatter key still has one editor identity.
- **Decision:** extra columns project scalar frontmatter values through
  `read_field`; list-valued extras are out of scope (use `tags`/`aliases`).
  `editable_columns` returns built-ins, extras in config order, then the group
  key, deduplicated. Existing `edit_cell` handles the new keys unchanged.
- **Decision:** the Base sidecar format stays line-oriented and backward
  compatible: a fifth line holds comma-separated extras; a missing fifth line
  loads as empty. Invalid keys on load are dropped, not fatal. Older binaries
  ignore the extra line. No vault file is touched by config changes.
- **Decision:** the GUI offers one `column` input with Add, a removable chip
  per extra column, and renders the selected columns through the existing
  `base_cell_ui`. Widget identity keeps `Id::unique(("base_cell", …))` with the
  column discriminator already present; no new palette, font or icon.

Verification: raw `cargo test --workspace` including `tests/bases.rs` cases for
legacy sidecar load, round-trip, validation, projection and editability; GUI
smoke on the isolated vault adding a custom column, editing a cell and
confirming the frontmatter on disk plus a snapshot.

Status: implemented 2026-11-23. `cargo test --workspace`: 208 passed, 0 failed.
`tests/bases.rs` covers validation, two-column ordering, legacy sidecar load,
invalid-key filtering, projection and scalar editing. GUI smoke on the isolated
vault added `subject`, showed `Physics`/`math` values, edited Geometry to
`Biology` (status `Updated subject in Geometry`), confirmed the frontmatter and
one snapshot on disk, and removed the chip.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- Docs were still marked "locked, not implemented" and the checklist count was
  stale; all contract documents now reflect the shipped feature.
- Coverage added for two extra columns and `\r` rejection in validation.
- Card view rendered an extra column twice when its key equaled `group_by`;
  the card loop now uses the same group-key filter as table mode.

---

## 59. Amendment — sort Base by frontmatter field (G2 slice)

Locked 2026-11-23 before code.

- **Decision:** `SortBy` gains a `Field` variant and `BaseConfig` gains
  `sort_field: String`. Selecting `field` sorts rows by that scalar frontmatter
  key using the same string ordering and empty-last rule as `created`; ties keep
  the existing `id` tiebreaker. Missing field values sort last, so notes without
  the key never appear first by accident.
- **Decision:** `sort_field` is validated by the same core key rules as custom
  columns; empty means the field sort falls back to `Title`. The field is
  projected into row values even when it is not a visible column, so sorting
  does not require adding a column.
- **Decision:** the Base sidecar gains a sixth line for `sort_field`; a missing
  sixth line loads as empty. Old four- and five-line sidecars stay readable.
- **Decision:** the GUI adds a `field` sort option and a key input shown when it
  is selected. No new palette, font, icon or widget id; the existing config save
  path persists the change.

Verification: raw `cargo test --workspace` including `tests/bases.rs` cases for
field ordering, missing values, invalid/empty keys and legacy sidecars; GUI smoke
on the isolated vault selecting `field`, typing `subject` and confirming sorted
rows plus the persisted sidecar line.

Status: implemented 2026-11-23. `cargo test --workspace`: 213 passed, 0 failed.
`tests/bases.rs` covers deterministic field order with an id tiebreak, missing
values last, empty-key fallback to title, key normalisation and 4/5/6-line
sidecar compatibility. GUI smoke on the isolated vault selected `field`, typed
`subject` and observed Geometry (Biology), Subject (Physics), Algebra (math),
then notes without the field; the sidecar gained the sixth line `subject`. A
follow-up smoke typed `bad,key`: the amber warning appeared, rows fell back to
title order, and the sidecar's sixth line stayed empty; retyping `subject`
restored the valid sort.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- The GUI bound `TextEdit` directly to `sort_field`, so an invalid draft was
  persisted and sorted differently until restart. Core now owns
  `set_sort_field` normalisation plus an in-session validity guard; the GUI
  keeps a raw draft in `base_sort_input` and commits only validated keys.
- README and ROADMAP now name the `field` sort option alongside §58 columns.

---

## 60. Amendment — Base sort direction (G2 slice)

Locked 2026-11-23 before code.

- **Decision:** `BaseConfig` gains `sort_desc: bool`. Reversing affects only the
  primary key; the `id` tiebreaker stays ascending so equal rows keep a
  deterministic order in both directions. All five `SortBy` variants honour it.
- **Decision:** the Base sidecar gains a seventh line, `asc` or `desc`. Missing
  or unknown values load as `asc`, so 4/5/6-line sidecars stay readable.
- **Decision:** the GUI toggles direction with one text button beside the sort
  options, using the locked muted/accent text colors; no new icon, palette
  entry, widget id or external dependency. Existing config save persists it.

Verification: raw `cargo test --workspace` including `tests/bases.rs` cases for
descending title and field sorts, stable id tiebreak, and 6/7-line sidecar
compatibility; GUI smoke toggling direction and confirming the sidecar line.

Status: implemented 2026-11-23. `cargo test --workspace`: 217 passed, 0 failed.
`tests/bases.rs` covers descending Title/Path/Created/Modified/Field sorts;
the ascending id tiebreak is asserted directly for Title and Field, missing
values stay last, and 4/5/6/7-line sidecars are covered. GUI smoke toggled `asc` → `desc`, observed Algebra (math),
Subject (Physics), Geometry (Biology), then notes without the field, and
confirmed the seven-line sidecar ending in `desc`.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- The direction toggle rendered in the default text color; it now uses
  `muted()` for `asc` and `accent()` for `desc`, matching §60 and the locked
  palette.
- Descending coverage was limited to Title and Field; Path/Created/Modified
  and short-sidecar cases are now asserted too.

---

## 61. Amendment — plain-text blockquote mechanics (G4 slice)

Locked 2026-11-23 before code.

- **Decision:** `notes::quotes` owns a pure, char-indexed `on_enter` for
  Markdown blockquote lines. It reuses the shared `ListEdit` result and the
  `line_bounds` / `in_fence` helpers from §55/§57; no AST, I/O or GUI types.
- **Decision:** Enter on a quote line copies its exact marker prefix
  (`>`, `> `, `> > `, `>>>`, including leading indentation) into the new line
  and keeps the caret after that prefix. Enter on a quote with no body replaces
  it with a blank line, leaving the quote. The quote is split at the caret, as
  in §55, so mid-line Enter keeps the left text on the old line.
- **Decision:** lists keep priority, then pipe tables, then quotes; the `[[`
  popup keeps highest priority. The Source editor consumes the handled Enter.
- **Decision:** BOM, CRLF and indentation are preserved; all offsets are
  character indices, matching egui.

Verification: raw `cargo test --workspace` including `tests/quotes.rs` for
continuation, nesting, exit, mid-line split, BOM, CRLF, indentation and fenced
code; GUI smoke in Source mode confirming autosaved bytes after autosave.

Status: implemented 2026-11-23. `cargo test --workspace`: 226 passed, 0 failed.
`tests/quotes.rs` covers continuation, nested/compact markers, empty-line exit
(also with BOM+CRLF), mid-line split, BOM, CRLF, indentation and fenced-code
exclusion. GUI smoke in
Source mode continued `> quoted line` with a new `> ` line; the autosaved file
kept the BOM and both lines.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) findings are recorded here after both reports arrive.

---

## 62. Amendment — Base text filter (G2 slice)

Locked 2026-11-23 before code.

- **Decision:** `BaseConfig` gains `filter_text: String`. A trimmed,
  case-insensitive substring match is applied to each note's title, path and
  projected frontmatter values before grouping; an empty filter matches all.
  The match is a pure projection in `bases::build`; no note content is changed.
- **Decision:** the Base sidecar gains an eighth line for `filter_text`; a
  missing line loads as empty, so 4–7-line sidecars stay readable. Newlines are
  impossible from the locked single-line editor, and commas are safe because
  each config field owns its own line.
- **Decision:** the GUI adds one `contains` input row beside the existing tag
  filter; changes persist through the existing config save path. No new
  palette, font, icon, widget id or external dependency.

Verification: raw `cargo test --workspace` including `tests/bases.rs` cases for
title/path/value matches, case-insensitivity, no-match and legacy sidecars; GUI
smoke typing a filter and confirming fewer rows plus the eighth sidecar line.

Status: implemented 2026-11-23. `cargo test --workspace`: 229 passed, 0 failed.
`tests/bases.rs` covers case-insensitive title/value matches, an isolated
path-only match, trimmed input, no-match, legacy sidecars and comma-safe
round-trip. GUI smoke typed `physics` in the `contains` row: rows went from 15
to 1 and the sidecar gained an eighth line ending in `physics`.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- The initial test name promised path coverage but its needles also matched
  titles/values; a dedicated `pathonly.md` case and a trimmed-input assertion
  now guard both contract dimensions.
- Both reviewers initially read the docs before the post-feature update;
  README, ROADMAP and CHECKLIST now describe the `contains` filter and §62.

---

## 64. Amendment — Search result keyboard navigation (G1 slice)

Locked 2026-11-23 before code.

- **Decision:** the Search tab owns `search_selected` and a query/mode signature.
  The selection resets when the query or search mode changes, and clamps to the
  current result count. ArrowUp / ArrowDown move it; Enter opens the selected
  note. Mouse clicks also set the selection.
- **Decision:** keys are handled only while the Search tab is active and no
  modal window is open, so the command palette, quick open, capture, rename,
  outline, properties, templates, composer, saved-query/workspace dialogs,
  review, grade and the snapshot restore window keep their own key handling.
  Handled keys are consumed so the toolbar `TextEdit` cannot also act on them.
- **Decision:** rows use the existing `selectable_label` with the locked
  `muted()` metadata; the selected row is not given a new colour, size or icon.
  No new `id_salt`, palette entry or business logic in the app — result rows are
  still produced by `nexora-core`.

Verification: raw `cargo test --workspace`; GUI smoke on the isolated vault
searching a known query, moving with ArrowDown/ArrowUp, pressing Enter to open
the selected note, and confirming that the palette remains unaffected.

Status: implemented 2026-11-23. `cargo test --workspace`: 231 passed, 0 failed.
GUI smoke on the isolated vault: query `a` returned 8 rows, two ArrowDown
presses selected the third row (`Card`), Enter opened that note, and Ctrl+P
still opened the command palette. `modal_open()` also guards the snapshot
restore window, so its keys are not stolen while Search is active.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- `modal_open()` initially omitted `snapshot_modal`; Arrow/Enter could be
  consumed from the restore window while Search was active. The guard and this
  amendment now include it.
- Documentation was still marked "locked, not implemented"; status, checklist,
  README and roadmap now match the shipped feature.

---

## 65. Amendment — manage saved queries (G5 slice)

Locked 2026-11-23 before code.

- **Decision:** `saved_queries` owns pure `rename` and `remove` helpers. Rename
  validates the new name (non-empty, single-line), rejects collisions with a
  different existing name, preserves order and query text, and is a no-op when
  old and new names are equal. Remove is by exact name. Both helpers return the
  new vector; the GUI owns the sidecar write and error display.
- **Decision:** the existing `Apply saved query` window becomes the manager:
  each row offers Apply, Rename and Delete. Rename is an inline editor with
  OK/Cancel; Delete asks for confirmation before the write. Validation and
  write failures keep the window open and show the reason inline (§48 pattern).
  No new command, palette entry, icon, font or colour.
- **Decision:** the sidecar format stays `MAGIC` + `name\ttext` lines. Applying
  remains unchanged: validate only, set Query mode with the stored text.
  Deleting or renaming never edits note files.

Verification: raw `cargo test --workspace` including `tests/saved_queries.rs`
cases for rename order/text, collision rejection, invalid names, remove and
sidecar round-trip; GUI smoke on the isolated vault renaming and deleting a
saved query, checking the sidecar and that Apply still works.

Status: implemented 2026-11-23. `cargo test --workspace`: 234 passed, 0 failed.
`tests/saved_queries.rs` covers rename order/text, same-name no-op, invalid and
colliding names, remove and sidecar round-trip. GUI smoke saved `math`, renamed
it to `geometry`, deleted it with confirmation, and left only the sidecar magic
line.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- Inline Cancel or opening a different rename/delete target left a stale error
  visible; both now clear the single error owner.
- The rename status printed the raw draft while the core stores a trimmed name;
  the status now uses the trimmed value.
- Documentation was still marked "locked, not implemented"; status, checklist,
  README and roadmap now match the shipped feature.

---

## 63. Amendment — Base filter recovery (G2 slice)

Locked 2026-11-23 before code.

- **Decision:** core `bases` owns `has_filters(config)` and `clear_filters(config)`;
  only `filter_tag` and `filter_text` are filters. Sorting, grouping, extra
  columns and sort direction are view settings and survive a filter reset.
- **Decision:** the Base panel shows a `Clear filters` control only when a
  filter is active, and an empty result states that filters caused it. No
  disabled or decorative control is drawn (DESIGN §1.6).
- **Decision:** clearing is a config edit: it goes through the existing
  `save_base_config` path, never touches vault files, and leaves the sidecar
  format unchanged. The buttons are ordinary small buttons with stable layout;
  no new id_salt, palette entry, font or icon.

Verification: raw `cargo test --workspace` including `tests/bases.rs` cases for
`has_filters`, `clear_filters` and settings preservation; GUI smoke on the
isolated vault applying tag + text filters, seeing the empty state, clearing,
and confirming rows return and the sidecar is updated.

Status: implemented 2026-11-23. `cargo test --workspace`: 231 passed, 0 failed.
`tests/bases.rs` covers both filters together, each filter alone, whitespace-only
values, settings preservation and padded tag keys. GUI smoke: `physics` gave 1
row; the top `Clear filters` returned 15 rows and emptied sidecar lines 2/8;
typing `zzzz` showed the empty state and its `Clear filters` returned 15 rows in
the same frame. Sort field `subject`, group `tags` and direction `desc` survived.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- `build` now trims `filter_tag` like `has_filters`, so a padded tag cannot show
  a recovery button while leaving the projection unchanged.
- The empty-state clear now rebuilds the projection in the same frame instead
  of lagging one repaint; the redundant late clear and its unused flag were
  removed. Tests cover tag-only and text-only filter detection.

---

## 66. Amendment — workspace management (G5 slice)

Locked 2026-11-23 before code.

- **Decision:** core `workspace` owns `rename` and `remove`, keyed by
  `(vault, name)`: trim and single-line validation, collision within the same
  vault, missing-old error, same-name no-op, and preservation of tab descriptors
  and active index. Other vaults are never touched.
- **Decision:** the Apply window becomes a manager with Rename (inline
  OK/Cancel) and Delete (explicit confirmation) per row. Applying a workspace
  keeps the §47 dirty-tab flush and §42.1 skipped-tab remap unchanged.
- **Decision:** every mutation writes through `workspace::save` and updates
  in-memory state only after success; errors stay in `self.error` (§48) and the
  manager stays open. Starting an inline action clears a stale error; Cancel
  clears it too. No sidecar format change, no vault writes, no new palette/font.

Verification: raw `cargo test --workspace` including `tests/workspace.rs` for
rename/remove semantics; GUI smoke in the isolated vault renaming and deleting
a workspace and checking the sidecar.

Status: implemented 2026-11-23. `cargo test --workspace`: 236 passed, 0 failed.
`tests/workspace.rs` covers scoped rename/remove, trim-before-collision,
no-op, payload preservation and cross-vault isolation. GUI smoke in the
isolated vault renamed `new-smoke` to `renamed-smoke` and deleted `retry-ok`
with confirmation; the sidecar contained `smoke` and `renamed-smoke`.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- Successful Apply left a stale `self.error`, which coloured the success status
  rust. The success arm now clears it, matching §48.
- Apply abort and malformed descriptors closed the manager; it now stays open
  on failure and closes only after a successful apply.
- Added a padded-name collision test to lock trim-before-collision ordering.

---

## 67. Amendment — fuzzy command palette ranking (G1 slice)

Locked 2026-11-23 before code.

- **Decision:** `commands::filter` keeps the existing registry as the single
  source of truth and reuses `fuzzy::score` from the file switcher (§21): the
  query is scored against both label and shortcut, the best score wins, and
  rows are ordered by score descending with registry order as the tiebreak.
  Empty or whitespace-only query returns the full registry in stable order.
- **Decision:** a command appears only when the query is a subsequence of its
  label or shortcut. No new scorer, dependency, label alias or shortcut; the
  palette row rendering and `palette_selected` keyboard navigation are unchanged.
- **Decision:** ranking stays pure and deterministic so the first row is stable
  for Enter; tests cover substring, acronym/shortcut matches, tie order and
  empty query. No new palette/font/id_salt.

Verification: raw `cargo test --workspace` including `tests/commands.rs`, and
GUI smoke typing partial/acronym queries in the isolated vault and confirming
that Enter runs the highlighted command.

Status: implemented 2026-11-23. `cargo test --workspace`: 239 passed, 0 failed.
`tests/commands.rs` covers subsequence ranking, shortcut matching, deterministic
registry-order ties and empty queries. GUI smoke: `grph` left only Open graph,
Enter opened the Graph tab; typing after ArrowDown restored the first row.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- Tie order was only tested for determinism; the test now asserts that the two
  tied commands really have equal fuzzy scores, then that the earlier registry
  entry (`SaveQuery`) wins, so the tiebreak cannot silently stop being covered.
- Typing after an ArrowDown left the old row highlighted; the palette now keeps
  a query signature and resets `palette_selected` to 0 whenever it changes.

---

## 68. Amendment — Explorer keyboard navigation (G1 slice)

Locked 2026-11-23 before code.

- **Decision:** with focus on a note row in the Explorer tree, ArrowUp/ArrowDown
  move focus to the previous/next visible note row and scroll it into view;
  Enter keeps egui's native button activation, so it opens the focused note.
  Focus, not a parallel selection state, is the single source of truth.
- **Decision:** only note rows participate; folders, bookmark rows, tag chips
  and the root row are untouched. The keys are captured and consumed only when
  a tree note actually has focus and no modal is open, so the Source editor and
  all other panels keep their keyboard behaviour.
- **Decision:** no new id_salt value: each note row already lives under
  `push_id(&node.path)`; its `Response::id` is reused for focus. No palette,
  font, icon, business logic or sidecar change.

Verification: raw `cargo test --workspace` and GUI smoke in the isolated vault:
focus a tree note, press ArrowDown twice, confirm focus moved to the third note
and scroll followed, press Enter, confirm that note opened; then confirm the
Source editor still receives plain arrow keys.

Status: implemented 2026-11-23. `cargo test --workspace`: 239 passed, 0 failed.
GUI smoke in the isolated vault: clicked Algebra, ArrowDown moved to Dup, the
second to Geometry, and Enter opened Geometry.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- egui's native cardinal focus navigation is spatial and can land on folders,
  the root row, tag chips or bookmarks; the row now sets
  `EventFilter { vertical_arrows: true }` via `set_focus_lock_filter`, consumes
  the arrow events, and moves focus explicitly over the collected note rows.
- Scrolling used the previously focused row because focus movement happened at
  `end_pass`; the destination row's rect is now scrolled into view directly.
- `set_focus_lock_filter` needs a frame of focus history; a fast repeated arrow
  could trigger egui's spatial pass before the filter installed. Handled keys
  now also call `move_focus(FocusDirection::None)`, clearing the pending cardinal
  move; GUI smoke with consecutive arrows stays on the note rows.

---

## 69. Amendment — Review queue keyboard navigation (G1 slice)

Locked 2026-11-23 before code.

- **Decision:** the `Review due notes` window gains ↑/↓ selection and Enter to
  open the selected note, matching the Search and Command palette patterns.
  Selection is an app-local index, reset when the command opens the window,
  clamped/wrapped against the current due list, and highlighted with
  `selectable_label`. Mouse clicks still open directly.
- **Decision:** handled arrow/Enter events are consumed after the window closes
  so the editor and other modals cannot double-apply them. Escape still cancels.
  The selection scrolls into view when it moves. No core, sidecar, palette,
  font or icon change.

Verification: raw `cargo test --workspace` and GUI smoke in the isolated vault
with at least three due notes: open the queue, ArrowDown twice, Enter, confirm
the third note opens in Source mode with its line jump intact.

Status: implemented 2026-11-23. `cargo test --workspace`: 239 passed, 0 failed.
GUI smoke in the isolated vault with three due notes: ArrowDown twice moved the
highlight from Algebra to Geometry, and Enter opened Geometry.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed one nit:

- Arrow/Enter events were removed for both press and release; the filters now
  match only `pressed: true`, mirroring the Search/palette pattern.

---

## 70. Amendment — template management (G5 slice)

Locked 2026-11-23 before code.

- **Decision:** `notes::templates` owns `save`, `upsert`, `rename` and `remove`
  for the existing byte-length sidecar (§36), with single-line names that cannot
  contain `|`. `daily` is the built-in auto-applied template (§37) and may be
  edited but not renamed or removed, so daily note creation never loses its
  resolver. No other special names.
- **Decision:** the GUI adds a `New template` editor (name + body) and turns
  `New note from template` into a manager with Edit / Delete (confirmation).
  Applying a template still only creates a note; management writes through
  `templates::save`, updates memory only after success, and keeps §48 error
  ownership with inline rust() text and the window open on failure.
- **Decision:** sidecar format is unchanged and remains in app data; the vault
  is never touched by template management. No palette/font/icon/id_salt change.

Verification: raw `cargo test --workspace` including `tests/templates.rs` for
validation, round-trip, rename/remove and daily protection; GUI smoke in the
isolated vault creating, editing and deleting a template and checking the
sidecar bytes.

Status: implemented 2026-11-23. `cargo test --workspace`: 241 passed, 0 failed.
`tests/templates.rs` covers validation, byte-exact sidecar round-trip, upsert,
rename/remove and protection of `daily`. GUI smoke created `smoke`, renamed and
edited it to `smoke2`, then deleted it; the sidecar returned to `daily` only.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed one coverage gap:

- `daily` body editing through `upsert` — allowed by §70 and used by the GUI —
  had no direct assertion; `tests/templates.rs` now checks the name, body and
  list length explicitly so `daily` cannot be silently renamed.

---

## 71. Amendment — bookmark manager (G1 slice)

Locked 2026-11-23 before code.

- **Decision:** `Bookmarks` owns `remove` and `move_by` pure order operations in
  addition to `toggle`/`prune`/`rename`. Invalid indices are no-ops; `move_by`
  swaps only within bounds so the list length and uniqueness are preserved.
- **Decision:** a `Manage bookmarks` palette command opens a window listing
  bookmarked notes with Open, ↑/↓ and Remove. Mutations go through
  `bookmarks::save` and update memory only after success; §48 error ownership
  keeps failures inline and the window open. Explorer's bookmark section and
  the toggle command stay unchanged.
- **Decision:** sidecar format stays one id per line in app data; no vault write,
  no palette/font/icon/id_salt change. The registry gains one command, with the
  command-count test updated.

Verification: raw `cargo test --workspace` including `tests/bookmarks.rs` for
remove/order semantics and the registry count; GUI smoke reordering and removing
a bookmark and checking the sidecar.

Status: implemented 2026-11-23. `cargo test --workspace`: 242 passed, 0 failed.
GUI smoke in the isolated vault: ↓ moved Algebra below Geometry and Remove
deleted Dup; the sidecar contained `Geometry` and `Algebra`. A recheck after
the write-order fix moved Geometry below Algebra, ending `Algebra`/`Geometry`.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed one major finding:

- The manager mutated `self.bookmarks` before `bookmarks::save`; a failed write
  left memory ahead of the sidecar and a later save could make the unconfirmed
  change durable. The operation now applies to a clone, and the clone becomes
  `self.bookmarks` only after the sidecar write succeeds.

---

## 72. Amendment — Health report keyboard navigation (G5 slice)

Locked 2026-11-23 before code.

- **Decision:** core `health` owns `selection_key(&[HealthIssue]) -> String`, a
  stable identity of the current issue list. The GUI resets `health_selected`
  to 0 when the key changes and clamps it to the issue count, so a selection
  never survives into a different report (§64 precedent).
- **Decision:** ArrowUp / ArrowDown move the selection through health rows and
  scroll it into view; Enter opens the selected issue's source note when it has
  one. Issues without a note (vault-level findings) are selectable but Enter
  does nothing beyond consuming the key. The existing `open` and `create`
  buttons remain the mouse path.
- **Decision:** keys are handled only when no modal is open, mirroring the
  `modal_open()` guard in §64. Pressed-only Up/Down/Enter events are consumed
  so egui's parent widgets cannot double-handle them. No new palette entry,
  id_salt, font, icon or palette colour.

Verification: raw `cargo test --workspace` including `tests/health.rs` cases for
stable and changing selection keys; GUI smoke on the isolated vault with a
broken link and another issue: two Down presses move the highlight, Enter opens
the expected source note, and Ctrl+P still works.

Status: implemented 2026-11-23. `cargo test --workspace`: 248 passed, 0 failed.
`tests/health.rs` covers stable keys, every key field individually, order, an
empty report and a crafted separator collision. GUI
smoke on the isolated vault: the first issue started selected, two ArrowDown
presses moved the highlight to the third row, Enter opened Algebra, and Ctrl+P
still opened the palette.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- `selection_key` now escapes fields through a `Debug` tuple, so `;`/`|` in a
  message or target cannot forge a boundary; tests mutate every field
  individually and assert a crafted old-format collision stays distinct.
- Health rows include the note id, making identical orphan rows distinguishable
  for keyboard selection.

---

## 73. Amendment — cyclic tab navigation (GUI shell)

Locked 2026-11-23 before code.

- **Decision:** core `views::tabs` owns a pure `cycle(active, len, backward)`
  index helper: no tabs means `None`; otherwise the result wraps in both
  directions. The GUI stores only `active`; no second source of truth.
- **Decision:** `Ctrl+Tab` / `Ctrl+Shift+Tab` move to the next/previous tab and
  reuse the existing `active` index. Other Ctrl combinations are unaffected;
  the events are consumed so egui's text editor cannot insert a tab.
- **Decision:** the shortcut is ignored while any modal is open (same
  `modal_open()` guard as §64/§72), so typing in a dialog cannot switch tabs.
  No new palette entry, id_salt, font, icon or palette colour.

Verification: raw `cargo test --workspace` including `tests/tabs.rs` for empty,
single, forward/backward wrap and out-of-range input; GUI smoke with three tabs
cycling forward twice and backward once, then Ctrl+P still opening the palette.

Status: implemented 2026-11-23. `cargo test --workspace`: 253 passed, 0 failed.
`tests/tabs.rs` covers empty, single, forward/backward wrap and out-of-range
indices. GUI smoke with three tabs: Health A → Ctrl+Tab → Algebra → Ctrl+Tab →
Geometry → Ctrl+Shift+Tab → Algebra; Ctrl+P still opened the palette.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed one finding:

- Tab-event consumption now requires `modifiers.ctrl` on the event itself, so
  a simultaneous unmodified Tab cannot be swallowed from the Source editor's
  list/table/quote handlers (§55/§57/§61).

---

## 74. Amendment — named Base views (G2 slice)

Locked 2026-11-23 before code.

- **Decision:** `bases` exposes pure `to_fields(&BaseConfig) -> Vec<String>` and
  `from_fields(&[&str]) -> BaseConfig`, factored out of the existing sidecar
  format (§56–§62). `base.idx` bytes and its legacy compatibility are unchanged.
  A new `validate_config` rejects tab/newline in single-line fields so a view
  can be stored as one TSV row.
- **Decision:** core `views::named_bases` owns `NamedBase { name, config }` and
  `upsert` / `rename` / `remove` / `save` / `load` in app data
  (`named_bases.idx`, magic `NEXORA-NAMED-BASES-v1`). One line per view:
  `name` then the seven config fields separated by tabs. Names follow the
  saved-query rules: trimmed, single-line, non-empty, unique.
- **Decision:** two palette commands, `SaveBaseView` and `ApplyBaseView`, reuse
  the saved-query manager pattern (inline rename, confirmed delete, error
  ownership). Save captures the current `base_config`; Apply writes it to
  `base.idx`, switches to / creates a Base tab, and closes the manager. A failed
  sidecar write leaves the manager open with the cause, per §48.
- **Decision:** vault files are never touched. No new palette colour, font,
  icon or id_salt; manager widgets follow the locked component rules.

Verification: raw `cargo test --workspace` including `tests/named_bases.rs`
for upsert/rename/remove, single-line validation and sidecar round-trip, plus
`tests/commands.rs` registry count; GUI smoke saving a view, mutating the
active filter, applying the view and confirming rows/config return.

Status: implemented 2026-11-23. `cargo test --workspace`: 257 passed, 0 failed.
`tests/named_bases.rs` covers upsert/rename/remove, validation and sidecar
round-trip; `tests/commands.rs` checks the 31-command registry. GUI smoke:
saved `smokeview`, filtered to 0 rows, applied the view (17 rows restored),
renamed and deleted it; `named_bases.idx` ended at the magic line only. A
second smoke applied a field-sorted view while the draft held another key and
confirmed the draft was refreshed.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- Applying a view now refreshes `base_sort_input` (and clears the column draft)
  with the stored config; otherwise the next edit silently overwrote the
  applied sort field with a stale key.
- `valid_column_key` rejects tabs and `validate_config` rechecks `sort_field`
  and rejects untrimmed keys, so a hand-built config cannot produce a TSV row
  that changes on load.
- `ApplyBaseView` reports `No saved Base views yet` instead of opening an empty
  manager (matching §66).
- Contract docs now list named views in README, ROADMAP and CHECKLIST.

---

## 75. Amendment — insert a template into the current note (G4 slice)

Locked 2026-11-23 before code.

- **Decision:** `templates::insert_at(text, cursor, body)` is a pure,
  character-indexed insertion. It preserves BOM and CRLF, normalises the
  rendered body's line endings to the host note's style, and inserts a blank
  line before/after the block when the caret sits mid-line. The caret lands
  after the inserted block. No AST, I/O or GUI types in core.
- **Decision:** a new palette command `InsertTemplate` reuses the existing
  template picker with a mode flag: `NewNote` keeps §36 behaviour, `Insert`
  renders with today's date and the current note's title and writes through
  `save::write_note` with the disk anchor. Dirty tabs are refused with an exact
  status; GM-4 conflicts use `record_conflict`; success calls
  `refresh_tab_after_write` and stores the returned caret in the editor.
- **Decision:** the modal stays open on validation/write failure per §48, and
  closes only on success/Cancel/Escape. No new palette entry, font, icon or
  id_salt; the picker keeps its existing `template_rows` salt.

Verification: raw `cargo test --workspace` including `tests/templates.rs` cases
for mid-line insertion, BOM/CRLF, empty note and caret placement, plus the
command-registry count; GUI smoke inserting a template into an open note and
confirming the editor text, caret and autosaved bytes.

Status: implemented 2026-11-23. `cargo test --workspace`: 261 passed, 0 failed.
`tests/templates.rs` covers mid-line insertion, caret, empty note, BOM, CRLF,
EOF/blank lines and empty body. GUI smoke inserted the `daily` template into
open notes in Source mode; blank-line separation, status and autosaved bytes
matched.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- Refusals (wrong view, dirty tab, missing note, GM-4 conflict) now set
  `self.error` as well as `status`, so the kept-open picker shows the cause
  inline per §48.
- `insert_at` now inserts a real blank line on both sides of a mid-line block,
  but skips the trailing separator when the source already has a blank line
  there; EOF/blank-line coverage added.
- Editor cursor restoration is now owned by one `set_editor_cursor` helper
  reused by link autocomplete, list/table/quote editing and templates.

---

## 76. Amendment — Canvas keyboard navigation (G5 slice)

Locked 2026-11-23 before code.

- **Decision:** core `canvas::selection_key(&Canvas) -> String` is the stable
  identity of the node set, derived from `BTreeMap` order. The GUI clears
  `canvas_selected` when it changes and keeps only an `Option<NoteId>`.
- **Decision:** in the Canvas tab, Left/Up select the previous node and
  Right/Down the next, wrapping in both directions; Enter opens the selected
  note. The selected card gets a 2 px `Accent` stroke; no other colour or
  chrome changes (DESIGN §2/§4).
- **Decision:** keys are handled only when no modal is open, and pressed-only
  arrow/Enter events are consumed so egui cannot double-handle them. Dragging,
  click-to-open, Reset layout and sidecar persistence stay unchanged. No new
  palette entry, id_salt, font or icon.

Verification: raw `cargo test --workspace` including `tests/canvas.rs` cases
for stable/changing selection keys; GUI smoke selecting two cards with arrows
and opening one with Enter, then Ctrl+P still working.

Status: implemented 2026-11-23. `cargo test --workspace`: 262 passed, 0 failed.
`tests/canvas.rs` covers selection-key stability and node-set changes. GUI
smoke: with nothing selected, the first ArrowRight selected the first card and
the second selected the next; Enter opened the highlighted note and Ctrl+P
still worked.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed one finding:

- First Right/Down with nothing selected now picks the first node (index 0);
  Left/Up still wraps to the last, making both directions consistent.
- `selection_key` is now a `Debug` tuple of node ids, with a crafted `;`
  collision test (`{"a","b","c"}` vs `{"a;b","c"}`, same class as §72) that
  actually fails the old `join(";")`; docs list the feature in README/ROADMAP.

---

## 77. Amendment — Graph view keyboard navigation (G5 slice)

Locked 2026-11-23 before code.

- **Decision:** core `graph::selection_key(&GraphLayout) -> String` is a Debug
  tuple of the node ids in layout order, so selection resets when the visible
  node set changes. The GUI keeps only `Option<NoteId>`.
- **Decision:** in the Graph tab, ←/↑ select the previous node and →/↓ the
  next, wrapping; Enter opens the selected note. The selected node is drawn
  with a slightly larger accent circle plus a 1.5 px `Accent` ring; no other
  colour or chrome changes (DESIGN §2/§4). Clicking a node still opens it.
- **Decision:** keys require `!modal_open()` and pressed-only events are
  consumed. Layout, truncation and deterministic placement stay untouched;
  no new palette entry, font, icon or id_salt.

Verification: raw `cargo test --workspace` including `tests/graph.rs` for
selection-key stability; GUI smoke selecting two nodes with arrows and opening
one with Enter, then Ctrl+P still working.

Status: implemented 2026-11-23. `cargo test --workspace`: 264 passed, 0 failed.
`tests/graph.rs` covers selection-key stability, node-set changes and a `;`
collision. GUI smoke moved the highlight with arrows (including wrap at the
list boundary via ←/→), Enter opened Dup and Ctrl+P still worked.

---

## 78. Amendment — Note Composer write parity (§40/§54 fix)

Locked 2026-11-23 before code. Triggered by a read-only audit finding real
correctness bugs in the Composer path.

- **Decision:** core `composer::apply_extract` writes the new note first, then
  the source. If either write fails the source still holds the extracted lines,
  so the extracted lines stay recoverable instead of being lost to a stale
  index anchor (a retry uses a fresh id because the first note already exists).
- **Decision:** `apply_merge` returns the list of every note id it rewrote
  (source excluded, it is deleted) plus the merged bytes for the target, so
  the GUI can refresh exactly the tabs whose disk text changed. Previously it
  returned only a link count.
- **Decision:** the GUI refuses Extract/Merge when any tab for a note the
  operation writes is dirty, with exact statuses
  `Save or discard changes before extracting` / `... before merging`. No note
  is written and the modal stays open (§48). This also prevents `close_tab`
  from flushing a dirty source over its just-deleted file (resurrection).
- **Decision:** after a successful write the GUI calls
  `refresh_tab_after_write` for every written id using the exact bytes written;
  merge closes the source tab only after those refreshes, so `flush_tab` cannot
  reintroduce old content. `GradeNote` clears a stale `self.error` on success.

Verification: raw `cargo test --workspace` including new `tests/composer.rs`
cases for merge return value and write order; GUI smoke: extract with source
tab open, merge a dirty source (refused), merge with witness/target tabs open
and confirm no stale content or resurrection.

Status: implemented 2026-11-23. `cargo test --workspace`: 265 passed, 0 failed.
`tests/composer.rs` covers the merge change list, exact bytes and the
new-note-first write order via a forced source conflict and a fresh-id retry. GUI smoke
extracted lines 3-4 from CompSrc into a new note and confirmed the source
link and disk bytes; dirty-tab refusals were code-reviewed (MCP `type_text`
cannot mark an egui editor dirty).

---

## 79. Amendment — safe tab close and best-effort snapshot pruning

Locked 2026-11-23 before code. Triggered by a read-only audit finding a
data-loss path in tab close.

- **Decision:** `close_tab` flushes first, then keeps the tab and aborts when
  the tab is still dirty (conflict or I/O failure). Status is
  `Save or discard changes before closing tab`. GM-4 Reload/Overwrite stays
  available because the tab and its buffer survive.
- **Decision:** snapshot retention pruning is best-effort: a failure to delete
  an old snapshot is ignored, never propagated through `write_note`. A cleanup
  problem must not block saving the note itself.
- **Decision:** no change to conflict banner, autosave, or snapshot bytes;
  no new palette entry, font, icon or id_salt.

Verification: raw `cargo test --workspace` including snapshot tests; GUI smoke
closing a clean tab (closes) and a tab whose flush fails (stays open with the
conflict banner).

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed one finding:

- `prune` is now best-effort end to end: `list_snapshots` / `history_dir`
  failures are also ignored, so no cleanup error can reach `write_note`.

Status: implemented 2026-11-23. `cargo test --workspace`: 266 passed, 0 failed.
`tests/m1.rs` covers a locked old snapshot not blocking a note write. GUI
smoke: clean tab closes; conflict-abort path code-reviewed (MCP cannot
reliably create a GM-4 conflict).


---

## 80. Amendment — Base table keyboard navigation (G2 slice)

Locked 2026-11-23 before code.

- **Decision:** core `bases` owns a pure `move_selection(rows, cols, current,
  direction)` grid helper: no wrapping; movement clamps at the first/last row
  or column. `None` current selects (0,0). No GUI or I/O types in core.
- **Decision:** the GUI keeps `Option<(row, column)>` plus a stable projection
  key (Debug of row ids/groups/occurrences and column keys). Selection resets
  when the projection changes. Editing a cell still uses the existing
  `BaseEdit`; arrows are handled only when no modal, no cell editor and a cell
  is already selected (so config text inputs are never hijacked). Enter opens
  the selected cell's editor; clicked cells become selected.
- **Decision:** the selected cell uses the locked `Raised` fill (DESIGN §4) and
  no new colour, font, icon or id_salt. Note-title buttons and cell edit IDs
  remain unchanged.

Verification: raw `cargo test --workspace` including `tests/bases.rs` cases for
grid movement and clamping; GUI smoke clicking a cell, moving with arrows and
committing an edit with Enter.

Status: implemented 2026-11-23. `cargo test --workspace`: 267 passed, 0 failed.
`tests/bases.rs` covers movement and clamping. GUI smoke: clicked the
created cell of `Algebra (school)`, pressed Escape to dismiss the editor,
ArrowRight to select its tags cell, Enter to open that editor, then Enter
again to commit (`Updated tags in Algebra`). Arrows in the `contains`
config field did not move the grid selection.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- Navigation is skipped while any of the five config TextEdits holds focus;
  they now have explicit ids. Enter is consumed by the nav handler so egui
  cannot activate an arbitrary focused button instead.
- Cell buttons use a stable `push_id` scope; arrows and Enter require plain
  modifiers. Cancel keeps the selection; Commit clears it.
- The selection signature also includes filter/sort settings so a
  re-projection cannot leave a stale positional index.

---

## 81. Amendment — guarded write anchors and manager persistence (audit)

Locked 2026-11-23 before code.

- **Decision:** `save_properties` refuses to write when any open tab for the
  note is dirty, with exact status `Save or discard changes before editing
  properties` and an inline error; the modal stays open. When clean, the write
  anchors on `tab.loaded` (disk-as-seen-at-load), never on `tab.content`. This
  prevents a false GM-4 conflict whose Reload would discard unsaved edits.
- **Decision:** Save-query and Save-workspace compute the updated list into a
  local, pass it to the sidecar write, and assign to `self` only on success
  (the §71 clone-save-commit rule). A failed write can no longer leave a
  phantom entry in memory that a later successful save makes durable.
- **Decision:** `create_from_template` returns the success of `write_new_note`;
  the template picker closes only on success. Failure keeps it open with the
  inline cause, matching §48/§75 Insert mode.

Verification: raw `cargo test --workspace` unchanged at 267 passed; GUI smoke
with a dirty note + Properties Save (refusal, modal open, rust inline reason,
no conflict banner), a query/workspace sidecar write failure path reviewed, and
a template collision keeping the picker open.

Status: implemented 2026-11-23. Two-axis code review (Spec + Standards,
adapted to direct-file review because there is no git repo) closed one finding:

- The Properties window now renders `self.error` inline in `rust()`; previously
  the dirty-refusal reason appeared only in the status bar.

---

## 82. Amendment — saved-query manager keyboard navigation

Locked 2026-11-23 before code.

- **Decision:** core `saved_queries` owns `selection_key(&[SavedQuery]) ->
  String`, a Debug-tuple identity of the list. The GUI resets the selection on
  a key change and clamps it, so rename/remove cannot leave a stale index
  (§64/§72 precedent).
- **Decision:** ↑/↓ move the highlighted query with wrap and scroll; Enter
  applies it through the existing validated path. Mouse clicks also set the
  selection. The inline rename editor gets an explicit id; while it holds
  focus, navigation keys are left to the TextEdit.
- **Decision:** no new command, palette entry, id_salt, font, icon or palette
  colour. Row rendering uses `selectable_label` plus `Raised` accent only via
  egui's selection visuals.

Verification: raw `cargo test --workspace` including `tests/saved_queries.rs`
cases for stable keys, value changes and list changes; GUI smoke with three
queries: two Downs move to the third, Enter applies it; arrows inside the
rename field do not move the list.

Status: implemented 2026-11-23. `cargo test --workspace`: 269 passed, 0 failed.
`tests/saved_queries.rs` covers stable keys, value/list changes, empty lists and
a delimiter collision. GUI smoke: qa/qb/qc created; two Downs highlighted qc;
Up+Enter applied qb; ArrowDown inside the rename editor did not move the list.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed these findings:

- A row click while a rename or delete confirmation was pending silently
  applied the query and discarded the pending action; clicks are now guarded
  the same way as the Rename/Delete buttons, and a successful apply clears both
  inline states.
- README and ROADMAP now describe the manager's ↑/↓/Enter navigation.

---

## 83. Amendment — audit fixes: sidecar errors, modal guard, workspace apply

Locked 2026-11-23 before code.

- **Decision:** `canvas::load`, `bases::load` and `bookmarks::load` at startup
  go through `sidecar_or` like every other app-data sidecar. A corrupt header
  surfaces once in `self.error` and falls back to a safe default; it must not
  be silently replaced and then overwritten by the next write.
- **Decision:** global shortcuts (QuickOpen, GoToHeading, Rename, ToggleView,
  Palette, NewNote, QuickCapture, Search, CloseTab) are dispatched only when
  `modal_open()` is false. Escape remains owned by modals; the Ctrl+Tab guard
  from §73 stays as is. This prevents a dialog from changing the tab behind it.
- **Decision:** workspace apply validates all descriptors into a candidate tab
  list before `self.tabs.clear()`. On a malformed descriptor the current tabs
  and active index are left untouched and the reason is surfaced; no partial
  destruction and no meaningless retry.
- **Decision:** the §74 verification note says 31 commands, matching the
  registry and tests. No new command, palette entry, font, icon or id_salt.

Verification: raw `cargo test --workspace`; GUI smoke with a corrupt
`canvas.idx` showing the error banner, Ctrl+W inside a modal leaving the tab
open, and a malformed workspace descriptor leaving tabs intact.

Status: implemented 2026-11-23. Raw `cargo test --workspace` at implementation:
269 passed, 0 failed; after §84: 271 passed, 0 failed. GUI smoke: a corrupt
`canvas.idx` showed the error banner in the status bar. The Ctrl+W-in-modal
and malformed-workspace-descriptor scenarios were not re-smoked after the
change; code paths were reviewed statically.

---

## 84. Amendment — production and UX audit findings closed

Locked 2026-11-24 before code. No identity change: palette, presets, type
scale, icons and component chrome stay exactly as locked in §15/§35.

- **Decision (PROD-P1):** `vault::note_path` appends `.md` to the last path
  component instead of `Path::set_extension`, so a note id with interior
  dots (`v2.0`) maps to `v2.0.md` and `vault::reload` no longer skips it.
  Regression in `tests/m1.rs`.
- **Decision (PROD-P2 / UX-P1):** `apply_rename` refuses while the tab for
  `plan.old` is dirty. Status only (no duplicate error line), plan retained,
  no filesystem call. Other guarded writes behave the same way.
- **Decision (PROD-P3):** `DiskMonitor::new` keeps a failed startup
  fingerprint instead of `unwrap_or_default`; `take_startup_error()` surfaces
  it once in the app, unless `build_index` already reported the same failure.
  Regression in `tests/watch.rs`.
- **Decision (PROD-P4):** a 2 s cooldown after a GM-4 conflict suppresses
  autosave retries; it is cleared on a successful write, Reload or Overwrite.
  The conflict banner still belongs to one note and is never edited by the
  active tab (§47.4).
- **Decision (UX):** empty merge target list reports "Nothing to merge into."
  instead of opening a dead modal; `open_note` refuses an id missing from the
  index; a dirty deleted note hides Reload and keeps the buffer; clicking a
  broken outgoing link reports the creation side effect; the status text is
  plain when an error line is present; the tab close target includes the tab
  title; Base sort direction is labelled "Sort: asc/desc".
- **Accepted without code:** Tab in Source mode keeps egui's native focus move;
  Graph/Canvas selection stays visual (egui 0.37 has no live-region API);
  notifications stay in the status bar; Thai layouts use the palette/Ctrl+P
  as the layout-independent command path.
- A visual-review subagent failed on the provider's free-usage limit; the
  design-director verdict stands: identity replacement rejected, only polish
  inside the locked system allowed.

Verification: raw `cargo test --workspace`: 271 passed, 0 failed. GUI smoke
in the isolated temp vault: dirty note + F2 → Preview → Confirm shows
"Save or discard changes before renaming" and keeps the buffer; the rename
window is still open for retry; Base control reads "Sort: desc"; tab close
target reads "× Base"; corrupt `canvas.idx` surfaces the error banner.

Two-axis code review (Spec + Standards, adapted to direct-file review because
there is no git repo) closed one finding:

- `disk.trim().is_empty()` treated an externally emptied file as a deletion
  and hid Reload. `conflict` now carries an explicit `deleted` flag, set only
  in the `Change::Deleted` arm; empty-but-present files remain reloadable.

Status: implemented 2026-11-24. Raw `cargo test --workspace` after the review
fix: 271 passed, 0 failed; `cargo build -p nexora-app` green.

---

## 85. Amendment — full visual identity replacement (DESIGN v2)

Locked 2026-11-24 after the user's explicit override. This amendment replaces
the identity locked in §15/§35 and the design-director's REJECT verdict; the
override is recorded as a user decision, not as an agent approval.

- **Decision:** concept A "Obsidian-faithful" is the master: exact HEX values
  sampled from the 11 screenshots in `examples/` (graphite `#222222`,
  lavender `#C9BAFF`, teal `#273535`, tag `#2D2838`), not invented colours.
  Full role table and contrast measurements live in `DESIGN_V2_SPEC.md`.
- **Decision:** four presets — Dark, Light, Sepia, HighContrast — with the
  derived Light/Sepia/HC values approved by the user. `theme.idx` moves to
  `NEXORA-THEME-v2`; a v1 sidecar surfaces one error and falls back to Dark.
  No theme picker/marketplace.
- **Decision:** local OFL Inter + JetBrains Mono are embedded from font files;
  the user explicitly allowed this single network download as the sole
  exception to the offline rule. Download once, commit the files, never call
  the network at runtime.
- **Decision:** layout geometry: 48 px functional icon rail, 240 px note tree,
  240 px context panel, 40 px toolbar, 30 px status, 12 px inner margins,
  8 px corners, `0 4 12` shadows on floating layers only, faint gradient on
  the active panel only.
- **Decision:** callouts and property rows are styled only; no new renderer
  features. Core logic, NoteId, vault, snapshots and commands are unchanged.
- **Decision:** verification = raw `cargo test --workspace` (271 baseline,
  0 failed), `cargo build -p nexora-app`, GUI smoke at 1400×850 and 1920×1080
  in all four presets, two-axis code review, docs updated.

Deliberate deviations (recorded during the two-axis review):

- `egui_commonmark` 0.26 blockquotes paint only a 3 px accent rule; the
  crate exposes no fill hook. Callouts therefore use the measured `info`
  and `note` roles for the accent and foreground, not a coloured block
  background. Adding a fill would require vendoring/patching the renderer,
  which §85 forbids ("styling only, no new renderer features").
- The user-approved Dark `accent_dim #6A6482` and link `#135594` from the
  SPEC failed the same SPEC's ≥4.5:1 rule (4.23:1 and 2.09:1 respectively).
  They are `#4A445C` and lavender `#C9BAFF` in code to satisfy the locked
  contrast requirement; DESIGN.md and the SPEC were updated to match.

Two-axis code review (Spec + Standards) closed these findings:

- HighContrast `info_fg` was a dark teal on the actual `surface` (1.15:1);
  it is now light teal, and the contrast test checks the pairing that is
  really painted. `artifacts/contrast-report.md`: 0 failures.
- Tag pills now use `tag()` fill with `accent()` text in all three places.
- Focus ring uses `widgets.active.bg_stroke` (2 px, 3 px in HighContrast)
  alongside the selection stroke.
- Evidence stored in `artifacts/visual/` (8 screenshots: four presets at
  1400×850 and 1920×1080), `artifacts/contrast-report.md` and
  `artifacts/raw-cargo-test.txt`.

Status: implemented 2026-11-24. Raw `cargo test --workspace`: 271 passed,
0 failed; `cargo build -p nexora-app` green with no warnings. GUI smoke with
egui-inspection: all four presets at both resolutions; the rail's Canvas
icon opened the Canvas view. `DESIGN.md` v2 replaced v1 (archived in
`DESIGN_v1_ARCHIVE.md`); measured palette in `DESIGN_V2_SPEC.md`.

---

## 86. Amendment — vault index recovery after a startup failure

Locked 2026-11-24 before code.

- **Problem:** when `vault::build_index` fails at startup, the app runs with
  an empty index, and `poll_disk` returns early both on `refresh()` errors
  and on `Ok` with zero changes. Even after the disk becomes readable the
  index stays empty until restart. A failed startup fingerprint leaves
  `known` empty, so recovering notes usually produce `Created` events — but
  the retry must still run before the `changes.is_empty()` return, because
  an empty-but-valid recovered vault produces no events at all.
- **Decision:** the app keeps one `index_unavailable` flag, set when the
  startup build fails. On each 1 Hz poll, after a successful `refresh()`,
  the app calls `reload_index()` while the flag is set — gated by the flag,
  not by `index.notes`. A successful reload clears the flag and the error in
  `reload_index` and arms a one-shot `recovery_notice`. The status is written
  once at frame end, after all action paths, so a save/create recovery in
  the same second cannot be overwritten by that path's own status.
- **Decision:** a clean-but-empty vault is never reloaded every second:
  the retry is gated by the startup failure flag, not by `index.notes`.
- **Decision:** no new command, palette entry, id_salt, font, colour or
  core API. Core logic is untouched; the GUI remains the thin shell.

Verification: code review plus GUI smoke in an isolated temp vault whose
`my-first-vault` path is initially a file (build_index fails), then replaced
by a real directory with a note; the note appears in the explorer and the
status reports restoration without user action. Raw `cargo test --workspace`
must stay at 271 passed, 0 failed.

Status: implemented 2026-11-24. Raw `cargo test --workspace`: 271 passed,
0 failed; `cargo build -p nexora-app` green. GUI smoke in an isolated temp
vault: `my-first-vault` was a file (I/O error, empty explorer), then replaced
by a directory with `Recovered.md` — the note appeared with status
"Vault index restored" without restart. Evidence:
`artifacts/visual/v86_recovery.png`.

Two-axis code review (Spec + Standards) closed three minor findings:
the one-shot `recovery_notice` makes the status survive write paths that
set their own status in the same frame; the amendment's retry-gating wording
is self-consistent (flag only, not `index.notes`); and the fingerprint
rationale now states the real behaviour (a failed fingerprint leaves `known`
empty, so notes normally produce `Created`, but an empty recovered vault
still needs the retry before the early return). Raw `cargo test --workspace`
after the fixes: 271 passed, 0 failed; build green. GUI smoke repeated with
the final code: note appears and "Vault index restored" is visible in the
status bar; screenshot overwritten in `artifacts/visual/v86_recovery.png`.

---

## 87. Amendment — pre-release bug sweep (13 confirmed findings)

Locked and implemented 2026-11-25 after two parallel read-only audits of all
core modules and the GUI shell. No visual-identity change.

- **Core safety / panics:** `NoteId::is_safe` rejects any component containing
  `:` so a crafted `[[C:/secret]]` cannot escape the vault through
  `PathBuf::push`; `semantic::tail_on_word_boundary` advances by the actual
  separator width (NBSP/EM SPACE no longer split a code point);
  `display::decode_target` decodes hex escapes from raw bytes and keeps
  invalid multibyte escapes literal.
- **Parser / metadata consistency:** `search::frontmatter` and
  `links::mentions` strip a leading BOM before detecting the `---` fence;
  `wikilink::parse_links_with_lines` now treats `~~~` as a fence like every
  other parser, so rename cannot leave a `[[old]]` behind.
- **Robustness:** `TreeNode::id_key()` discriminates folder and note rows
  with the same path; workspace `load` never trusts a sidecar count for
  `Vec::with_capacity`.
- **GUI state:** quick capture refreshes an already-open Inbox tab before
  `open_note`; autocomplete popup is gated by editor focus and
  `!modal_open()` and is hidden while a modal is open; Properties, Grade
  and Insert-template pin the note id at modal open instead of using the
  active tab; `reload_index` only clears `error` on §86 recovery (flush_tab
  clears it on its own success); rename refreshes the renamed tab from the
  rewritten content so a self-link is not stale.
- **Verification:** 8 new regression tests; raw `cargo test --workspace`:
  279 passed, 0 failed; `cargo build -p nexora-app` green.

Two-axis code review (Spec + Standards) closed the remaining findings:
Properties selects its source tab by pinned id (and clears `error` on
success), capture refuses a dirty Inbox tab before writing, rename reads the
renamed note from the rebuilt index for both content **and** title, and
three regression tests displaced by duplicate `#[test]` attributes were
re-registered. Raw suite after the fixes: 279 passed, 0 failed.

Status: implemented 2026-11-25; review closed.

---

## 88. Amendment — test layout mirrors source layout

Locked and implemented 2026-11-25 before publication.

- **Decision:** the 41 integration tests move from `nexora-core/tests/*.rs`
  into five folders matching `nexora-core/src`: `storage/`, `notes/`,
  `links/`, `retrieval/`, `views/`. Milestone gates keep their names (m1/m2/m3)
  and live with the subsystem they mainly exercise.
- **Decision:** Cargo auto-discovers test crates only directly in `tests/`,
  so all 41 files are explicitly registered with `[[test]] name/path` in
  `nexora-core/Cargo.toml`. Adding a test file now requires adding a target.
- **Verification:** `cargo test --workspace` after the move: 279 passed,
  0 failed across 45 result lines — identical to the flat layout.

Status: implemented 2026-11-25.
