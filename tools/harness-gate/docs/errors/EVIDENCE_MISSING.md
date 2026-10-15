## Why this fails

A required artifact or control file is absent, or the binding operation cannot
inspect a declared artifact path. The current binding producer also maps
metadata-inspection errors, including permission and I/O errors, to this code.
The original error distinguishes absence from an existing but inaccessible file.

## Diagnose and repair

Inspect the named path and original filesystem error. Correct a missing output
at its producer, or repair the access problem when the file exists. Regenerate
the required evidence; do not create an empty placeholder or substitute an old
invocation artifact. Optional absent artifacts are not the same as required
missing evidence.

## Example

If a parent directory prevents metadata inspection, the artifact may exist but
still produce EVIDENCE_MISSING. Repair the intended access boundary and rerun
validation rather than fabricating a replacement file. If the producer wrote
to the wrong path, correct that output declaration and generate fresh evidence.
