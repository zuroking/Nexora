# Tooltips

`Response::on_hover_ui` with a compact preview: title plus first non-empty
body lines, frontmatter excluded. Used on Explorer / Context / Base / Quick
Open note rows. Outgoing-link rows deliberately have no hover preview (§34).
Inline hover on rendered Reading-view links is unsupported by `egui_commonmark`.
