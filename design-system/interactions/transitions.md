# Transitions

egui is immediate mode. Prefer instant state changes; avoid fake spinners and
delays. If a background job is added later it must show honest progress and a
cancel control. Keep retained state stable across frames — never rebuild ids
or scroll areas mid-frame.
