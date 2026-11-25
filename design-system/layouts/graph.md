# Graph and canvas

Both are center-panel tabs. Layout is computed in core: fixed circle order and
deterministic force iterations, no RNG. Above 500 notes the view shows the
highest-degree 500 with a truncation notice. Canvas positions live in an
app-data sidecar; Reset rebuilds the grid. Same resolved-link edge semantics.
