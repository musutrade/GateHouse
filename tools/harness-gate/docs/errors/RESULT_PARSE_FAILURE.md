## Why this fails

The configured parser could not consume the test result. This is different
from a valid report with zero tests or fewer tests than required. A successful
producer exit does not prove its output uses the configured report format.

## Diagnose and repair

Check the step's parser reference, declared parser kind and result path. Inspect
the producing command's exit status and logs. Verify that the report exists and
contains the complete expected format before retrying verification.

## Example

If a step declares a JUnit parser but its command writes console text to the
report path, configure the producer's JUnit output and point the parser at that
XML file. Keep console logs separate. Do not rename plain text to `.xml` or
replace a malformed report with an empty passing report. Preserve any required
minimum result count.
