# Nexora roadmap — future work

Status: **idea pool, not a commitment.** School and the May 2027 attestation
outrank this project. Items are grouped by when they become worth doing, not by
a promise of delivery. Everything here must keep the source on GitHub and must
not copy proprietary Obsidian code or assets.

Implemented so far: V1.1-0 (Reading view + authored visuals), V1.1-1
(external-change detection), the Zed/VS Code style folder-tree explorer, and
V1.1-2 (F2 rename with exact wikilink rewriting and dry-run preview), and
V1.1-4 (snapshot timeline with hand-rolled line diff), and V1.1-5 (actionable
health report with line jump), V1.1-6 (fuzzy file switcher + go-to-heading),
V1.1-7 (frontmatter / Properties panel), V1.1-3 (inline `[[` autocomplete at
the cursor), and V1.1-8 (last-saved time). **All v1.1 items are now done.**
Each was locked in `ARCHITECTURE.md` §15–§24, implemented, tested, and
code-reviewed before the next one. V2-1 part A (dependency-free local vector
index with Keyword/Semantic/Hybrid search) is also done — §25. Part B, the
opt-in neural `fastembed` backend, stays deferred pending a decision about
heavy deps and model download (DD-N5).

## Goals from `Полный Обзор Возможностей Obsidian.md`

The overview is AI-generated and unverified; it is a checklist, not a spec.
These goals say **what Nexora should become**, not that we will clone Obsidian.
Each goal is achieved only when the feature is locked in `ARCHITECTURE.md`,
tested in `nexora-core`, smoke-tested in the GUI, code-reviewed and documented.

| Goal | Target state | Current status | Constraints |
|---|---|---|---|
| G1 — Local-first parity | Every offline capability named in the overview is either implemented or explicitly rejected with a reason | in progress: core vault/editor/link stack done, search results keyboard-navigable (§64), fuzzy command palette (§67), Explorer keyboard navigation (§68), review queue keyboard navigation (§69), bookmark manager (§71); gaps listed below | no network, no cloud, Markdown stays source of truth |
| G2 — Structured data | Frontmatter-backed views (table, card, list; later kanban) with filters, sorting and group-by | partially done: Bases-lite table/cards with tag and text filters plus reset (§62–§63), sort (frontmatter field and asc/desc, §59–§60), group-by, guarded single-cell edits with keyboard navigation (§80), custom scalar
      columns (§58) and named saved views (§74); kanban/formulas and full spreadsheet editing deferred | config in app data; never rewrite unknown frontmatter keys |
| G3 — Link intelligence | Backlinks, outgoing links, unlinked mentions, page preview, embeds, bookmarks, automatic rename | done except inline Reading-link hover: backlinks, aliases, rename, related, unlinked mentions, bounded + anchored embeds, row-hover previews, bookmarks, outgoing panel | previews are pure projections; embeds are display-only, depth 3 + visited set |
| G4 — Editor expressiveness | Tables, colored highlights, math, Mermaid, image display, smarter lists | partially done: CommonMark + tasks + code + highlights + local images, plain-text list/table/blockquote mechanics (§55/§57/§61) and template insertion (§75); math/Mermaid/visual table editor still deferred | each renderer extension needs its own amendment and dependency decision |
| G5 — Visual workspaces | Graph, canvas, workspaces, theme presets, saved-query management | graph, canvas, four DESIGN v2 presets with icon rail (§85), saved workspaces, saved-query rename/delete (§65), workspace rename/delete (§66), template management (§70), keyboard-navigable Health report (§72), cyclic tab navigation (§73), keyboard-navigable Canvas and Graph (§76/§77), saved-query manager navigation (§82), guarded write parity (§81), sidecar/modal/workspace audit fixes (§83) and production/UX audit fixes (§84) done | layout/query state in app data, deterministic, rebuildable |
| G6 — Automation | Templates, CLI, URI actions, local-only plugin contract | quick capture, palette, templates, in-note template insertion (§75) and offline CLI done; URI/plugin still planned | CLI/URI offline only; plugin calls require per-call consent; no Web Clipper while network is excluded |
| G7 — Trust and recovery | Snapshots, diff, restore, health, external-change guard | done (v1.1); §84 closes dirty-rename, startup-fingerprint and conflict-retry gaps; §86 recovers the index after a startup vault failure | 10 snapshots per note, no silent overwrite |
| G8 — Hard exclusions | Sync, Publish, themes marketplace, mobile apps, community plugin store | rejected | conflicts with local-only / CPU-only / no-network constraints or with school-first time budget |
| G9 — Honest research | Every external claim in docs is either sourced or marked unverified | in progress: RESEARCH.md separates primary sources from the AI overview | no copying proprietary code, themes, icons or assets |

The remaining queue is tracked in `CHECKLIST.md` under "Remaining — v2" and
"Remaining — v3 / long horizon". It is an unordered pool, not a delivery
schedule: pick by interest after the attestation, not by row position.

Rules inherited from `ARCHITECTURE.md` and the global protocol:

- Markdown on disk stays the only source of truth. Indexes, embeddings, canvas
  positions, and plugin state live in app data and are rebuildable.
- Any new runtime dependency is recorded as a Deliberate Deviation *before*
  code.
- AI features are opt-in, local-first, and never write to a note without an
  explicit Insert action and a snapshot.
- No secrets, no keys, no automatic network calls. A user-initiated model
  download is the only exception, with consent shown in the UI.
- CPU-only. No feature may assume a GPU.

## v1.1 — trust and friction (next)

| ID | Feature | Why | Risk / boundary |
|---|---|---|---|
| V1.1-0 | Reading view + authored visuals (`Ctrl+E`) | Makes notes pleasant to read; kills the generated look | Renderer is display-only; Live Preview stays deferred |
| V1.1-1 | External-change watcher with dirty-state guard | Markdown is the source of truth; stale buffers are correctness bugs | Debounce + write-through dedup on Windows |
| V1.1-2 | Rename note with exact wikilink rewriting (`F2`) | Prevents graph rot; dry-run preview before writes | Multi-file partial failure; rely on snapshots |
| V1.1-3 | Inline `[[` autocomplete at the cursor, Tab/Enter to insert | Wikilinks are the graph's edges; highest-leverage keyboard fix | egui cursor/focus fights; keep popup state stable |
| V1.1-4 | Snapshot timeline with line diff | Makes the 10 snapshots visible and trustworthy | Compute diff once per selection, cache it |
| V1.1-5 | Actionable health report (click → file + line) | A report that only informs is dead weight | Recompute on events, not per frame |
| V1.1-6 | Fuzzy file switcher + go-to-heading | Keyboard flow for a vault you know | Keep strictly separate from the command palette |
| V1.1-7 | Frontmatter / Properties panel | Metadata drives links and tags; raw editing corrupts it | Conservative lossless subset; unknown keys untouched |
| V1.1-8 | Word count, last-saved time, snapshot count in status bar | Honest feedback instead of a fake "Saved" label | Cheap, cache on text change |

## v2 — offline intelligence and new views

| ID | Feature | Why | Risk / boundary |
|---|---|---|---|
| V2-1 | **AI:** opt-in local semantic index (multilingual, e5-small 384-dim) | Finds "exam prep" when the note says "attestation"; RU/KZ must work | Model download only on explicit consent; CPU build time; keyword search must still work without it; no FAISS |
| V2-2 | **AI:** related-note suggestions (lexical vectors + tags + graph distance) — **done, v2-2 slice 0** | Serendipity without an LLM | Factors shown; top 5; dismissals still deferred |
| V2-3 | **AI:** explicit-action local summarization | Review aid, never a silent writer | Quantized CPU model, cancel button, Insert/Discard only, no API keys |
| V2-4 | **AI:** local-only plugin contract over JSON-lines stdin/stdout | Future-proofs AI features without vendor coupling | Per-call consent, exact payload preview, zero plugins by default, loopback only |
| V2-5 | Deterministic canvas view — **done** | Graph says what connects; canvas says what belongs together | Positions in app-data sidecar, drag/click/reset, no freehand or zoom |
| V2-6 | Nested tags (`#school/math`) + aliases — **done** | Flat tags stop scaling; student vaults split by subject | Full tag + parent keys; aliases resolved in links/search/autocomplete, real id wins |
| V2-7 | Templates with closed placeholder set (`{{date}}`, `{{title}}`) — **done** | Lowers capture friction for daily notes | No scripting in templates |
| V2-8 | Light and high-contrast presets — **done** | Accessibility, not decoration | Presets only; no colour picker |
| V2-9 | Auto-apply template to a newly created daily note — **done** | Removes the last manual step from the morning workflow | Only on creation; never rewrites an existing daily note |
| V2-10 | Heading- and block-scoped wikilinks (`[[Note#Heading]]`, `^blockid`) — **done** | Link to the exact paragraph instead of the whole note | Touches parser, display, outgoing, rename, health; plain note links stay valid |
| V2-11 | Boolean and field search (`AND`/`OR`/`NOT`, `path:`, `[author:...]`) — **done** | Find notes by subject/property during attestation prep | Core-only; existing keyword and semantic modes keep working |
| V2-12 | Note Composer: merge notes, extract selection into a new note with an auto-inserted link — **done** | Reorganise a big subject note without breaking links | Confirmation modal; rewrite links on merge; no clipboard or external editor |
| V2-13 | Saved queries over the Query parser — **done** | Stop retyping complex boolean searches | Store source text only; reparse on use; no sidebar pinning in v2 |

## v3 — long horizon

| ID | Feature | Why | Risk / boundary |
|---|---|---|---|
| V3-1 | **AI:** offline "ask your vault" with mandatory citations | Natural endpoint of PKM search | Every claim links to note + line; retrieved chunks shown; default off; experimental label |
| V3-2 | **AI:** contradiction / duplicate review queue | Vaults accumulate stale facts (old schedules, superseded decisions) | Advisory only, explicit scan, confidence threshold, never auto-merge |
| V3-3 | Spaced-repetition resurface queue (SM-2) — **done** | Turns a school vault into active recall for the attestation | Note-level only; no streaks or gamification |
| V3-4 | Multi-vault with isolated state | School and personal notes must not share a graph/search index | State keyed by `(vault_id, path)`; assertion tests for isolation |
| V3-5 | Deterministic offline backup bundles | Portability without sync or cloud | Sorted tar+zstd, blake3 manifest, import shows diff, never overwrite blindly |
| V3-6 | Temporal graph from snapshots | "What did I know when?" | Only 10 snapshots/note — label as recent history, not full history |

## Obsidian parity backlog (`research/obsidian-parity`)

Source: Zuro's AI-generated overview `Полный Обзор Возможностей Obsidian.md`,
2026-11-22. Unverified; treat as candidates, not as a promise to reproduce
Obsidian. Nexora already covers: local Markdown vault, tree explorer,
wikilinks/backlinks, graph, canvas, ranked + vector search, tags, daily notes,
Properties, snapshots/diff, external-change detection, rename with link
rewriting, command palette, quick open, outline, autocomplete, and health.

Still missing, ranked by fit with our locked constraints (offline, CPU-only,
no high-level wrappers, Markdown is the source of truth):

1. **Bases-lite** — table/card/list views over frontmatter with filters,
   sorting and group-by. We already parse properties conservatively; this is
   the highest-value local feature with no new dependency. Kanban is a later
   view on the same data.
2. **Embeds `![[note]]`** — inline inclusion of another note; needs recursion
   guards and must not feed back into indexing. Reading view only in v1.
3. **Unlinked mentions** — text occurrences of a note title without
   `[[...]]`, shown in the Context panel next to backlinks.
4. **Page Preview** — hover a link to see the target note in a small popup.
5. **Outgoing Links panel + Bookmarks** — explicit navigation aids over the
   existing index; bookmarks live in app data, not in the vault.
6. **Templates** with a closed placeholder set (`{{date}}`, `{{title}}`) —
   lowers daily-note friction; already in the old v2 list.
7. **Workspaces** — save tab/panel layout per vault in app data.
8. **Table editor / colored highlights / Mermaid / math / images** — renderer
   work; some require new dependencies or pulling features into
   `egui_commonmark`, so each needs its own amendment and decision.
9. **CLI + `nexora://` URI + Web Clipper** — automation and capture. CLI is
   feasible offline; Web Clipper is browser-side and stays out of scope while
   "no network" is locked.
10. **Sync / Publish / themes marketplace / mobile** — explicitly excluded;
    do not silently reintroduce.

## Ideas deliberately not planned

Network sync, encryption, cloud, telemetry, plugin marketplace, themes store,
Markdown WYSIWYG inline editing in v1, mobile app, calendar/statistics/kanban.
These are either out of scope in `ARCHITECTURE.md` §13 or would only be worth
revisiting after a different life stage. Keep the list honest.

## How to pick the next item

Not by row position. Pick by (a) does it break the source-of-truth promise,
(b) does it remove a daily friction, (c) is it interesting enough to finish.
V1.1-0 is the current slice because reading notes is the core act of a notes
app.
