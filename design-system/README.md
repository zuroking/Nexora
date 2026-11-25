# Nexora design system

Index of the locked visual system. These files restate `DESIGN.md` and
`ARCHITECTURE.md` §15/§35 in component-oriented form; they do not define a new
identity. If a conflict appears, `DESIGN.md` wins.

Values are native egui values: colors are `nexora-core::theme::Palette` roles,
sizes are `FontId` points, spacing is `style.spacing`. No CSS, DOM, React,
Next.js, Tauri or Electron. See `.opencode/skills/EGUI_ADAPTATION.md`.

- foundations/ — colors, typography, spacing, borders, icons
- components/ — buttons, inputs, tabs, tooltips, modals, navigation
- layouts/ — main-window, explorer, editor, context-panel, graph
- interactions/ — hover-states, transitions, keyboard-navigation
- quality/ — visual-checklist, accessibility
