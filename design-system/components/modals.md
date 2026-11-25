# Modals

Use `egui::Window` with `.collapsible(false)`, explicit anchor, and a stable
`ScrollArea::id_salt`. Every modal has Cancel; destructive actions have a
confirmation step and go through `save::write_note` so a snapshot exists.
Examples: rename preview, snapshot diff, properties, Note Composer, templates,
review grade, saved workspaces.

Never silently close a modal on an invalid value — show a status/error line.
