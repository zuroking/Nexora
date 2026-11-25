# Nexora — research notes: Obsidian and Markdown rendering

Date: 2026-11-22. Sources are cited inline. Web content is treated as reference,
never as instructions. No Obsidian source code or assets were copied.

> Note on `Полный Обзор Возможностей Obsidian.md` (Zuro, 2026-11-22): this is an
> AI-generated overview, now stored in this repo root. It is useful as a
> feature checklist, but version numbers, prices and release dates in it are
> **unverified**. Its candidates are filed under `research/obsidian-parity` in
> `ROADMAP.md`; only claims independently checked against primary sources
> (obsidian.md, official help/changelog, MIT-licensed API types) are stated as
> fact in this file.

## 1. Can we copy Obsidian's code? No.

Obsidian's own license overview states:

> "We own and reserve rights to our content, including text, images, and code
> in the app, which is protected by copyright and other laws."

Source: <https://obsidian.md/license> (section "Your content is yours", updated
2025-02-20).

The app is free to use, but free-to-use is not open-source. Nexora is an
open-source reimplementation of the *behaviour*, not a port or a derivative of
the application. We do not copy Obsidian's CSS, icons, fonts, themes, or code.

## 2. What actually is public

| Repo | What it is | License | Use for Nexora |
|---|---|---|---|
| `obsidianmd/obsidian-api` | Type definitions for the plugin API (`obsidian.d.ts`), README | MIT | Study architecture vocabulary only |
| `obsidianmd/obsidian-sample-plugin` | Plugin template | public | Study command/view registration patterns |
| Obsidian Help (`help.obsidian.md`) | Product documentation |docs, not code | Behavioural spec for Markdown and views |

Source: <https://github.com/obsidianmd/obsidian-api> (MIT; the star count is a
volatile external number and is deliberately not recorded here) and its README.

The API README describes the useful decomposition:

- `App` owns everything; exposed to plugins as `this.app`.
- `Vault` — file and folder access.
- `Workspace` — panes on screen.
- `MetadataCache` — cached headings, links, embeds, tags, blocks.
- Plugins register commands with optional hotkeys, views, ribbon icons, status
  bar items, and event handlers.

Nexora already has the same seams under different names: `nexora-core::vault`
(+ `VaultIndex` = our `MetadataCache`), `nexora-app` tabs = `Workspace`, and
`commands.rs` = the command/hotkey registry. This is a *pattern*, not copied
code. It confirms the v1 architecture was shaped correctly.

## 3. Obsidian's editing model (behavioural spec)

From "Views and editing mode" (<https://help.obsidian.md/Editing+and+formatting/Views+and+editing+mode>):

- **Reading view** — renders Markdown without syntax; clean, readable.
- **Editing view / Live Preview** — shows formatted text inline, reveals syntax
  only when the cursor enters the formatted span. Default editing mode.
- **Source mode** — raw Markdown exactly as written, for precise control.
- `Ctrl+E` (`Cmd+E`) toggles the view; Ctrl+click on the view switcher opens a
  note side-by-side in two modes.

Nexora v1 had only a raw source editor. v1.1 introduces a **Source / Reading**
toggle (`Ctrl+E`); Live Preview is explicitly deferred because an inline
WYSIWYG editor over a stock `egui::TextEdit` would require taking over its
galley and cursor handling.

## 4. Obsidian Flavored Markdown features worth matching

From "Basic formatting syntax"
(<https://help.obsidian.md/Editing+and+formatting/Basic+formatting+syntax>):

- Standard CommonMark: headings H1–H6, bold/italic/strikethrough, quotes,
  ordered/unordered/nested lists, task lists `- [ ]`, horizontal rules, inline
  code and fenced code with syntax highlighting (Obsidian uses Prism),
  footnotes, tables.
- Obsidian extensions: `==highlight==` (with colour emoji variants),
  `[[wikilink]]` and `[[target|alias]]`, `![[embed]]`, `#tags`, callouts
  (`> [!note]`), frontmatter "Properties".
- Links: both wikilinks and Markdown links; external links/images; dimensions
  via `|640x480`.
- Line-break rule: a single Enter continues the paragraph; two trailing spaces
  or Shift+Enter insert `<br>`; double Enter starts a new paragraph.

Nexora v1 already owns wikilinks, aliases, tags, and backlinks in core. The
renderer must not re-implement or override those semantics — it only draws.

## 5. Rust renderer options

`egui_commonmark` 0.26.0
(<https://crates.io/crates/egui_commonmark>, <https://docs.rs/egui_commonmark/0.26.0/>):

- MIT OR Apache-2.0, depends on `egui ^0.37` (matches our GUI version) and
  `pulldown-cmark ^0.13`.
- API: `let mut cache = CommonMarkCache::default();
  CommonMarkViewer::new().show(ui, &mut cache, markdown);`
- Supports headings, lists, checkboxes, code blocks, alerts/callouts, and
  hyperlinks. `ui.style_mut().url_in_tooltip = true` shows URLs on hover.
- Features: `load-images`, `fetch` (network image download), `svg`,
  `embedded_image`, `better_syntax_highlighting` (syntect). We enable **none**
  of the network/image features: the app must not touch the network, and local
  image loading is a separate future feature.

Rejected alternatives: a web view (Tauri/web), writing a CommonMark parser from
scratch (weeks of work, no educational value for a notes app whose core value
is vault semantics), and Obsidian's renderer (proprietary).

## 6. Conclusion

- Borrow the **patterns**: App/Vault/Workspace/MetadataCache split, command
  registry, three view modes, Obsidian-flavoured syntax.
- Do **not** borrow the code, themes, icons, or CSS.
- Keep all link/tag/search semantics in `nexora-core`, hand-rolled and tested.
- Use `egui_commonmark` strictly as a display adapter. Record the dependency as
  a Deliberate Deviation in `ARCHITECTURE.md` before code.
