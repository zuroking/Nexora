# Main window

Toolbar 44 px, status bar 30 px, left panel 232 px, right panel 236 px, 12 px
inner margins. Default window 1120×680. Panels are `egui::Panel::top/left/right/
bottom(...).show(ui, ...)` and `CentralPanel::default().show(ui, ...)` — they
take a `Ui`, not a `Context`. Status bar is added before the central panel.
