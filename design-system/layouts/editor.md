# Editor

Center panel: tab strip, then a header with title, Source/Reading segmented
control and Bookmark button. Source is a `TextEdit` with a stable `Id`; Reading
is `CommonMarkViewer` fed by `adapt_for_display`. Autosave after 700 ms idle;
conflicts show a Reload/Overwrite banner. Reading view is never editable.
