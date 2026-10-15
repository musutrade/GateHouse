# Tasks

All tasks implement [the proposal](proposal.md) and preserve Engineering Policy.
Each task is bounded to less than four hours.

- [x] [P0/S] Retain five hosted observations with stage/command timings and separate PR/push paths; reproduction and no mixed cohort tests pass.
- [x] [P0/S] Add optional nextest coverage backend with exact command metadata and negative test/export/threshold tests; standalone default remains cargo.
- [x] [P0/M] Compare complete test inventory and coverage with bounded A/B samples; record distributions, resource scope and sample limitations.
- [x] [P0/S] Retain only a measured beneficial CI change and document rollback; otherwise record rejection.
- [x] [P0/S] Pass applicable local checks and document the required hosted acceptance gate.

Publication/merge gate: all applicable hosted required checks must pass before
merge. Record the actual run and final acceptance in
[#61](https://github.com/musutrade/GateHouse/issues/61); local timing is not
hosted certification. This remains an enforced delivery condition beyond the
local implementation checklist.
