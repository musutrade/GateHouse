## Why this fails

A configuration field names an entry that has not been declared. The diagnostic
path identifies the reference to inspect; references must match the correct
kind of entry, such as a parser, service or profile.

## Diagnose and repair

Compare the referenced identifier with the declaration, including spelling and
case. Correct the reference or add the missing declaration with its intended
settings. Run `harness-gate config check` before verification.

## Example

If a step refers to parser `unit-results` but the configured parser is named
`unit-result`, choose the intended name and make the reference and declaration
agree. Do not delete parsing or a required service to silence the diagnostic.
A valid reference still needs to satisfy the target entry's own constraints.
