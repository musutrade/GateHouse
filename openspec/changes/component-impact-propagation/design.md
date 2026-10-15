# Component impact version 1

1. Path rules seed directly changed components. Git uses --no-renames, so old
   and new paths both seed the graph; deletions need no existing source file.
2. A declaration `api = ["shared"]` means api consumes shared. Propagation follows
   the reverse adjacency shared → api. Compute the least transitive fixed point
   with an iterative queue and ordered sets. A consumer change does not select
   its dependencies. Diamonds never enumerate all possible paths.
3. Component selection precedes ordinary profile filtering. Step depends_on
   retains prerequisite closure across profiles/components; it adds no component
   impact edge or implicit execution ordering. All mandatory gates stay present.
4. Explicit --components remains an exact override, followed by step dependency
   closure. --all remains all. unmatched=fail/all/ignore keep their established
   behavior, including no selection on a clean tree.
5. Graph validation uses Kahn traversal (no recursion), rejects duplicate edges
   and requires the exact workflow component inventory. Budgets: 1024 components,
   8192 edges, 64 pinned text inputs, 4 MiB per input. Ordered maps add logarithmic
   lookup overhead; traversal visits each reachable vertex/edge once.
6. Pins are lowercase SHA-256 of exact UTF-8 input bytes, with confined repository
   paths. Validate against the selected invocation input, after choosing staged
   versus working content. Read-only staged previews hydrate only declared graph
   inputs using read-only Git; reject checkout filter/encoding/ident transforms.
   Pin mismatch blocks even --all: review/regenerate graph and pins, or remove the
   impact section and deliberately use --all. A failed graph never becomes PASS.
7. verify --plan retains verification-plan/v1 and adds COMPONENT_IMPACT plus
   impact_chain, impact_sources and depends_on_components per component. A
   deterministic shortest witness runs from a directly changed dependency to a
   consumer. Sources identify exact paths and rule indices. The full declared
   adjacency remains available to explain alternative roots/diamond paths without
   exponentially enumerating chains. Explicit/all/fallback selections have no
   claimed impact chain.

Migration: opt in only after reviewing the complete graph, source pins and scope
rules. Older binaries reject the unknown impact field rather than ignore it.
Rollback: remove impact and use --all (or restore reviewed explicit glob coverage)
before downgrading. Removing graph propagation alone can reduce assurance. No
plugin lock, quality policy, baseline, threshold or waiver changes are required.

Cost: graph validation and pinned-input hashing occur per invocation; propagation
adds no subprocesses, collectors or service startup. Staged previews add at most
one size probe, one blob read and filter inspection per declared input. All
profiles use the same propagation. Additional execution cost is exactly the
consumer steps in that profile plus existing prerequisite closure; unrelated
components remain excluded. The 1000-component chain regression measures 100
selections and confirms leaf-only changes do not fan out backwards.
