# R-16 Scheduler and Wait Scenario Baseline (Before/After)

**Status:** Accepted local evidence for the R-16 wait/backoff and indexed-readiness change.

**Samples per scenario:** 5 warm runs per binary.

Scenarios: `fast` (24 quick independent steps), `slow` (four 1s steps), `narrow` (12-step serial chain), `wide` (1 root plus 24 leaves), `deep` (32-step serial chain), `failed` (failing root with 40 skipped dependents), and `cancel` (1s timeout cancels a 30s step and skips 10 dependents).

| Scenario | Before median (s) | After median (s) | Delta |
| --- | ---: | ---: | ---: |
| fast | 0.9732 | 0.8272 | -15.01% |
| slow | 1.3342 | 1.3551 | 1.57% |
| narrow | 1.5463 | 1.2491 | -19.22% |
| wide | 0.8557 | 0.9140 | 6.81% |
| deep | 3.7859 | 3.0708 | -18.89% |
| failed | 0.3264 | 0.2928 | -10.31% |
| cancel | 1.2772 | 1.3103 | 2.59% |

Raw per-sample JSON is in `r16-scenarios.json`.

