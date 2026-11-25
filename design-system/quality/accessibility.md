# Accessibility (native egui)

Target WCAG 2.2 AA where it maps to a native desktop app. eframe is built with
`accesskit`; the widget tree is inspectable through `egui-inspection`. Contrast
is asserted in core tests, not eyeballed.

- Everything operable by keyboard through the command registry.
- Visible focus; no color-only status (use text like "(missing)").
- Tooltips carry context; modals have explicit Save/Cancel.
- Screenshots must include the state under review, not just the happy path.
