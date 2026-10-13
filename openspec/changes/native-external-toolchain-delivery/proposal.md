# Native collector delivery with external dependencies

User-authorized normative policy delta: preserve the existing native engine and
publish our precompiled program using Core's release flow. The matching compiler,
LLVM tools, Python and system linker remain external. Stable rewriting and bounded
macro exemptions are not part of this delivery. See Engineering Policy section 10.

The program embeds the existing native driver and required adapter/schema sources;
its verified cache contains only those payloads. It selects an explicit external
sysroot, validates the pinned compiler/LLVM identity and never changes defaults.
Core continues to own final policy decisions. Historical anchors and metrics remain
unchanged; new artifact/tool identities are recorded without automatic adoption.
