# Colors

Source: `DESIGN.md` §2 and `nexora-core/src/views/theme.rs`.

| Role | Hex | Usage |
|---|---|---|
| Ink | #191817 | window background |
| Panel | #201F1D | side panels, toolbar, status bar |
| Surface | #272523 | editor / reading canvas |
| Raised | #302D29 | hovered controls, active tab |
| Line | #3A3733 | separators, panel borders |
| Text | #E8E3DA | primary text |
| Muted | #A9A196 | secondary text, paths, timestamps |
| Accent | #C98B6B | active tab underline, links, focus |
| Accent dim | #7A5543 | quiet accent backgrounds |
| Sage | #8FA98A | saved / ok |
| Amber | #D8A657 | warnings, snapshots |
| Rust | #D06B5B | errors, broken links |

Never hard-code a `Color32` in `nexora-app`; use the helpers backed by the
active `Palette`. Body-text contrast >= 4.5:1 is asserted in `tests/theme.rs`.
