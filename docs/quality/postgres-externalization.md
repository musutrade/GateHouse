# Postgres externalization candidate (GateHouse #24)

Dated 2026-10-14. GateHouse tracks delivery across both repositories. GateProbes
owns the MIT-licensed `postgres-service` plugin, Postgres URL parsing, test-database
isolation, external health checks, container recipes, ecosystem templates and
legacy migration. Core retains generic process execution, timeouts, requiredness,
service lifecycle, leases/cleanup, result validation and final blocking decisions.
Engineering Policy thresholds, baseline/debt/ratchet, measurement series,
required gates and release authority are unchanged.

## Contract and trust

`service_checks` maps a service ID to a nonempty list of required checks. Each
pins an absolute installed executable, SHA-256, plugin/version identity,
`gate-service-check/v1`, timeout and explicit `pass_env`. Core executes the
artifact in the project root with inherited environment cleared, supplies the
protocol/mode/value variables, and bounds time and output. The value stays out
of argv and published plugin stdout/stderr. Digest verification before and after
execution and an exact JSON result bind the selected program to its evidence.
Failure, missing executable, digest mismatch, incompatible/malformed/missing
result, crash and timeout fail closed before dependent steps. Doctor invokes
the same service path; migration makes its service check required. This digest
binding assumes the user installed a trusted artifact; it is not an OS sandbox
or a new signing ceremony.

Managed services retain explicit Docker/Podman health recipes and Core-owned
cleanup. External Postgres checks require `pg_isready` on the explicitly forwarded
PATH. URL isolation rejects production database identity and non-test names;
remote use requires explicit `HARNESS_GATE_ALLOW_REMOTE_TEST_DATABASE` opt-in.
The plugin cannot waive a required Core check.

## Consumer inventory

| Consumer | Candidate disposition |
| --- | --- |
| `src/recipes/postgres.rs` | Removed; implementation/license/tests now in GateProbes |
| `config/model.rs`, flow schema | Dedicated external policy enum removed; generic `ServiceCheck` added |
| `service/mod.rs`, doctor service checks | Same generic service-check preparation; no URL or database-name rules |
| `config/loader.rs` | Reject old `external_value_policy` with actionable migration help |
| `config/migration.rs` | Reject v1 recipe migration; GateProbes owns conversion |
| Config/preset/doctor/service/CI adoption tests | Explicit required external check fixtures or expected legacy rejection |
| `fixtures/preset-compat`, v1 fixture, ADRs, dogfood/evidence | Frozen historical bytes retained; no product fallback |
| Executable docs consistency | Asserts legacy rejection/help/no publication and unchanged migration source |
| Current EN/ZH configuration and ownership docs | Describe installed binding and compatible-Core migration |
| OpenSpec 4.2 original completion | Retained with appended dated correction |

No Postgres URL/naming/health implementation remains in the Core generic model
or active runtime. The secrets auditor's separate `postgres-url` secret format
is a historical security-audit contract, not a service policy or health recipe;
this issue does not remove its detection assurance.

## Upgrade, exit and rollback

Install and verify the plugin, run `gateprobes-postgres migrate --input <PATH>
--output <NEW_PATH> --project <NAME>`, then review and run `harness-gate config
check` with a compatible Core. The command refuses overwrite, retains the old
input, preserves step/policy configuration and binds its own installed executable
and digest. Existing quality configuration remains separate. Old v1 and v2
policies, including `none`, are rejected; deleting a policy field alone is not
an assurance-preserving migration. Ecosystem preset upgrades must preserve all
existing execution and quality gates while binding the installed check.

Compatibility exits when all active configs use explicit required checks and
an independently verified compatible released Core/plugin pair. v0.4.9 is
immutable and incompatible; GateProbes keeps that pin until a real compatible
release is verified. Release and package acceptance must fail in the meantime.
Plugin upgrades require explicit rebind; Core bytes do not change. Rollback
restores retained configuration with its compatible immutable artifact pair.
Do not mix a migrated flow with an old Core or remove checks to obtain success.

## Evidence and remaining certification

Developer evidence is frozen outside the checkout and delivered to the task
coordinator. Installed candidate acceptance exercises doctor, service steps and
verify plus failures using a fixed plugin artifact; Core is not rebuilt by
acceptance. It uses a deterministic health fixture because this host has no
`pg_isready`; it does not certify a live database daemon. Compatible released
Core, hosted required quality/critical-path certification, independent signed
package publication and actual daemon acceptance remain separate gates.

The production inventory retains the original coverage thresholds and boundaries.
Deleted recipe measurement is supported as an absent source on head;
`process/capture.rs` is a new measured production input with no continuity claim.
The coordinator must collect exact base/head inputs under the existing measurement
series. Critical-path source bindings are refreshed using the same function,
probe text, applicability and assertions; that refresh is not execution evidence.
