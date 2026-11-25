# Inputs

`TextEdit` keeps an explicit stable `Id` where state is set programmatically.
Single-line inputs for modal names, multi-line for note bodies. Validation
errors go to the status/error line, never a silent empty result. Placeholders
and hint text are honest; avoid decorative placeholder content.

Source mode and Reading mode are distinct: Reading view is never editable.
