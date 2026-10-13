# R-17 Configuration Validation Allocation Evidence

**Status:** Accepted local evidence for the borrowed validation-context change.

Valid configs with 100/400/1200 doctor checks exercise the former whole-config `clone()` per check path and the current borrowed-context validation. Wall time and peak child RSS are median values over 5 samples.

| Checks | Before wall (s) | After wall (s) | Before maxrss (KB) | After maxrss (KB) |
| ---: | ---: | ---: | ---: | ---: |
| 100 | 0.0632 | 0.0594 | 9220 | 9288 |
| 400 | 0.1139 | 0.0685 | 10304 | 10192 |
| 1200 | 0.4013 | 0.0931 | 12576 | 12744 |

Raw per-sample JSON is in `r17-config-allocation.json`.

