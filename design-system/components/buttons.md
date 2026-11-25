# Buttons

Use `ui.button(label)` for enabled actions and `ui.add_enabled(flag, Button::new(...))`
when disabled state is meaningful. Label with a verb ("Save workspace", not
"OK"). Secondary actions are plain labels; only the primary action gets a
filled background. Every button must have a handler — no fake affordances.

Theme with `Raised` on hover, `Accent` focus, `Text` foreground.
