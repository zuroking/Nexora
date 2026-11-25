# Nexora

Published at <https://github.com/zuroking/Nexora> under the MIT license.

A local-first, open-source notes app in Rust. Notes are plain Markdown files on
disk; Nexora adds a fast desktop interface, links, tags, search, and a graph —
without a server, an account, or a cloud.

## Status

v1 and the v1.1 polish line are complete. DESIGN v2 (§85) replaced the original
visual identity: a measured Obsidian-inspired workspace with four presets,
embedded OFL fonts and a functional icon rail. Raw `cargo test --workspace`:
271 passed, 0 failed. See `DESIGN.md`, `DESIGN_V2_SPEC.md` and `ARCHITECTURE.md`
§85 for the current state; §14–§35 remain the historical record.

## Features

- DESIGN v2 shell (§85): 48 px functional icon rail, graphite/lavender palette,
  four presets (Dark / Light / Sepia / HighContrast), embedded Inter and
  JetBrains Mono; the preset persists in `theme.idx` v2.
- Vault auto-created at `%USERPROFILE%\Documents\Nexora\my-first-vault`;
  nested folders supported, no folder picker.
- Zed / VS Code style folder tree in the left panel with expand/collapse and
  tag filtering; duplicate basenames stay distinct by path. With a tree row
  focused, ↑/↓ move between note rows and Enter opens the focused note.
- Context panel shows related notes with visible lexical/tag/graph factors,
  built on the local vector index.
- Deterministic canvas tab: note cards, wikilink edges, drag to reposition,
  click to open, and a rebuildable sidecar in app data. Arrow keys select cards
  and Enter opens the highlighted note.
- Bases-lite: table/card views over frontmatter with tag filter, sorting and
  group-by; view config stored in app data, never in the vault. Table cells for
  `created`, `tags`, `aliases` and a scalar group field can be edited: click a
  cell, type, press Enter or OK; Escape cancels. Edits go through the guarded
  note write (snapshot + conflict detection) and refresh every open tab. Custom
  scalar frontmatter columns can be added with the `column` field and Add, and
  removed with the chip; they persist in the Base sidecar. In table mode,
  click a cell then use arrow keys and Enter to edit it. Sorting supports a
  `field` mode by any scalar frontmatter key and an asc/desc toggle; missing
  values stay last in both directions. A `contains` filter matches title, path
  and projected frontmatter values case-insensitively; `Clear filters` resets
  the tag and text filters while preserving view settings. `Save Base view` /
  `Apply saved Base view` manage named views in app data (rename, delete, apply).
- Tabs, autosave (700 ms idle), 10 snapshots per note with conflict detection
  and Reload / Overwrite banner. Ctrl+Tab / Ctrl+Shift+Tab cycle tabs. Closing
  a tab flushes it first; on conflict or write failure it stays open with the
  Reload / Overwrite banner so the buffer is not lost.
- External-change detection (1 Hz poll): clean tabs follow disk edits, dirty
  tabs raise the conflict banner; external deletions close clean tabs. A GM-4
  conflict pauses autosave retries for 2 s (§84); a deleted dirty note keeps
  its unsaved buffer (Overwrite recreates the file). If the vault was
  unreadable at startup, the index is retried after recovery without a
  restart (§86).
- Guarded writes (§84): renaming a dirty note is refused with a visible status
  until it is saved; notes with interior dots (`v2.0.md`) round-trip; a bad
  app-data sidecar is reported once and never silently overwritten; a modal
  owns the keyboard so global shortcuts cannot change tabs behind it.
- Source-mode list mechanics: Enter continues `-`, `*` and numbered items
  (empty item exits), Tab/Shift+Tab change indent, Ctrl+Enter toggles task boxes.
- Source-mode pipe-table mechanics: Enter adds a row in the same column with the
  same padding (empty row exits), Tab/Shift+Tab move between cells and Tab on the
  last cell appends one; escaped `\|` is not a separator.
- Source-mode blockquote mechanics: Enter continues `>` markers with their exact
  nesting and indentation; an empty marker exits the quote.
- Snapshot timeline with line diff; restore from a previewed version. Restores
  and Properties edits refresh every open tab for the note from the exact bytes
  written; external edits surface the same Reload/Overwrite banner as autosave.
- `[[wikilinks]]` with frontmatter aliases, backlinks, Ctrl+Click navigation,
  and inline `[[` autocomplete at the cursor (Tab/Enter to insert).
- Nested tags (`#school/math`) index both the full path and its parent
  (`school`); no tag text in notes is rewritten.
- Context panel shows unlinked mentions (prose occurrences of a note's title
  or aliases) with line numbers and jumps to them.
- `![[note]]` embeds render the target body in Reading view; frontmatter is
  hidden, recursion is bounded, and the vault is never modified.
- Hovering note rows in Explorer/Context/Base/Quick Open shows a compact
  preview (title + first body lines, frontmatter excluded).
- Bookmarks in app data: toggle from the editor header or palette, listed in
  Explorer, updated on rename, pruned on deletion. `Ctrl+P → Manage bookmarks`
  opens a window to open, reorder (↑/↓) or remove bookmarks.
- Context panel lists outgoing wikilinks with line numbers; clicking a broken
  target creates the note at that path (same command as the palette).
- Wikilinks may target a heading or block: `[[Note#Heading]]` or
  `[[Note#^blockid]]`. Clicking jumps to that line; rename rewrites the note
  part and keeps the anchor. Unknown anchors just open the note.
- Embeds accept the same anchors: `![[Note#Heading]]` shows only that section,
  `![[Note#^blockid]]` only that line. A missing anchor leaves a visible
  `(anchor not found)` marker; nesting still respects the depth-3 limit.
- Local images: `![[pic.png]]` displays a vault file (png/jpg/jpeg/gif/webp/svg)
  through egui's in-memory byte loader — no network, no base64 in notes. Missing
  or unsafe paths leave a visible marker.
- Note Composer: `Ctrl+P → Extract lines to new note` cuts an inclusive line
  range into a new note and inserts `[[new]]` at the removal point;
  `Merge note into another` appends the note to a target and rewrites incoming
  links. Both ask for confirmation first.
- `F2` rename with dry-run preview; exact wikilinks across the vault are
  rewritten, history is preserved, CRLF files stay CRLF.
- Properties modal for `created`, `modified`, `aliases`, `tags`; other
  frontmatter lines and a UTF-8 BOM are preserved byte-for-byte. It refuses a
  save while the note has unsaved tabs, showing the reason in the modal.
- Full-text ranked search, tags, daily notes, Inbox quick capture.
- Health report (broken links, orphans, duplicate basenames); a click opens the
  note at the exact broken-link line. ↑/↓ move between issues and Enter opens
  the selected note; rows show the note id so identical orphans stay distinct.
  Composer extract/merge refuse dirty tabs and refresh every open note they
  rewrite.
- Deterministic graph view, top 500 nodes by degree. Arrow keys select nodes
  and Enter opens the highlighted note; click also opens a node.
- Command palette (`Ctrl+P`): new note, today, capture, search, quick open,
  go-to-heading, graph, health, restore snapshot, create note at broken-link
  target, cycle theme, new note from template, close tab. Rows are ranked by
  the shared fuzzy scorer over labels and shortcuts (`grph` finds Open graph);
  typing resets the highlighted row.
- `Ctrl+O` fuzzy file switcher and `Ctrl+Shift+O` heading outline jump.
- Source / Reading view toggle (`Ctrl+E`). Reading view renders headings,
  lists, task lists, quotes, code, and Obsidian-flavoured links/callouts.
- Three authored visual presets — dark, light, high-contrast. Cycle with
  `Ctrl+P → Cycle theme`; the choice persists in app data, never in the vault.
  Body and muted text keep ≥ 4.5:1 contrast on every panel. No colour picker.
- Templates stored in app data. `Ctrl+P → New note from template` creates a new
  note from a template; only `{{date}}` and `{{title}}` expand, unknown
  `{{...}}` stays literal. The manager can create, edit, rename and delete
  templates; `daily` is protected because `Today` resolves it by name. No
  scripting. `Today` renders the `daily` template into a newly created note; existing
  daily notes are never rewritten. `Ctrl+P → Insert template into current note`
  renders a template at the Source caret with the note title and today's date.
- SM-2 review queue for attestation prep: `review_due` frontmatter drives a
  due-note list; grading a note through `Grade current note` reschedules it
  through the normal snapshot path. A dirty tab must be saved or discarded
  before grading, and the editor frontmatter refreshes immediately after a
  grade. The queue supports ↑/↓ selection and Enter to open a due note.
  No decks, streaks or gamification.
- Saved workspaces: `Ctrl+P → Save workspace` stores the current tabs and
  active note per vault; `Apply workspace` rebuilds them (missing notes are
  skipped). No panel sizes or unsaved buffers are stored in the workspace file.
  Applying first flushes dirty note tabs, so pending edits are written before
  the tabs are replaced; if a tab cannot be saved (conflict or I/O error), the
  apply aborts and the open tabs are kept. The Apply window also renames and
  deletes workspaces with inline confirmation; failures keep it open.
- Offline CLI (`nexora-cli`): `search`, `query`, `rename` (dry-run by default,
  `--apply` to write) and `daily:path`. Thin wrapper over the same core calls
  the GUI uses; deterministic stdout, errors to stderr with exit code 1.
- Health report lists broken links with a `create` button that makes the
  missing target through the same command used by link navigation.
- Status bar: save state, word count, snapshot count, and relative last-saved
  time (`just now`, `12s ago`).
- Search has four modes: Keyword, Semantic, Hybrid and Query. Query accepts
  `AND`/`OR`/`NOT`, quoted phrases, `path:` and `[author:...]` fields; it
  never panics on a malformed query. `Ctrl+P → Save current query` stores a
  named query in app data (source text only, reparsed on every use);
  `Apply saved query` restores it into the Search tab, and the same window can
  rename or delete saved queries. The manager supports ↑/↓ with wrap and Enter
  to apply; result rows support ↑/↓ selection and Enter to open the note.
- Local vector index (`Vectors`): Keyword / Semantic / Hybrid search over
  deterministic lexical embeddings. No model, no network — the UI says so.

## Build and test

Rust 2021 edition; verified with rustc 1.100.0. Use your own toolchain — no
machine-specific path is required.

```powershell
cargo build
cargo test
cargo run -p nexora-app
```

`nexora-core` holds all domain logic and tests. `nexora-app` is the egui/eframe
shell and is verified by manual smoke, not window tests.

## Documentation

- `ARCHITECTURE.md` — locked decisions, module boundaries, deliberate deviations
- `DESIGN.md` — visual system and acceptance checklist
- `CHECKLIST.md` — done vs remaining across all milestones and goals
- `RESEARCH.md` — what is public about Obsidian and why we did not copy its code
- `Полный Обзор Возможностей Obsidian.md` — Zuro's AI-generated feature
  overview (unverified); used only as a checklist for `ROADMAP.md` goals
- `ROADMAP.md` — future features, including opt-in local AI ideas
- Root `../ARCHITECTURE.md` §4 — deviation that allows this greenfield project

## Licensing and scope

No Obsidian source, themes, icons, or assets are included. Obsidian's own
license reserves rights to its app code; this project only follows publicly
documented behaviour and the MIT-licensed `obsidian-api` type definitions.
`egui_commonmark` (MIT OR Apache-2.0) is used strictly as a display adapter.

This repository is MIT licensed — see `LICENSE`. No network, sync, encryption,
telemetry, push, or publishing is part of the application code.
