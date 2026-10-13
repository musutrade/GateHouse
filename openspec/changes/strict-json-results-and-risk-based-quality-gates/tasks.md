# Implementation Tasks

本清单对应 [proposal](proposal.md) 和 [design](design.md)，实施进度以逐项验证证据为准。
S = 小于 1 小时，M = 1–2 小时，L = 2–不足 4 小时；超出估算时继续拆分，
不能把多个未验收步骤合并勾选。实现与交付按 Issue 授权；本地验证不代表远端 CI 或基线接受。

## 1. Freeze evidence and measurement contracts

- [x] 1.1 [P1][M] 核对评审基线、测试二进制来源及子进程插桩；验收：保留提交/target/工具版本与 profile 关联证据，解释文件级/模块级口径差异。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。Added `tools/quality/measurement_contract.py` and [measurement contract](../../../docs/quality/measurement-contract.md); the generator records review/candidate SHAs, target directory/triple, dev/release-small profiles, Rust/Cargo/nextest/llvm-cov/OpenSpec versions, nextest binary paths/digests, LLVM instrumentation flags/profile pattern, child-process instrumentation requirements, unsupported branch status, and file/module/aggregate coverage interpretation. The review SHA is not present in this shallow checkout and remains explicitly unaccepted; candidate evidence is reproducible with the documented commands.
- [x] 1.2 [P2][M] 选择并锁定开发用复杂度分析器，记录许可与语法限制；验收：固定 `if/match/guard/loop/short-circuit/?/closure/macro` 样例具有可重建 CC 期望值。关联：[design](design.md#decisions)。Locked the in-repository stdlib-only analyzer `harness-gate-complexity` 0.1.0 (MIT) in [complexity_analyzer.py](../../../tools/quality/complexity_analyzer.py) with rule `mccabe-rust-1` v1 and the documented exact CC formula; the fixed fixture [controls.rs](../../../tools/quality/fixtures/complexity/controls.rs) (1,771 UTF-8 bytes, SHA-256 `e049647d1177f5bbbb2bd2f73cf11169722544a571b16fbc3aef823a393e3bb8`) reproduces the committed [controls.expected.json](../../../tools/quality/fixtures/complexity/controls.expected.json) raw counts and CCs (5, 5, 5, 2, 1, 1, 2, 1, 1, 1, 1) under the commands in [complexity-analyzer.md](../../../docs/quality/complexity-analyzer.md), including locked syntax limits, closure/nested-function/macro handling, and fail-closed rejection of non-braced closures and unsupported tokens. Repro CLI and fixture unit tests pass.
- [x] 1.3 [P2][S] 定义质量证据 schema、series identity 和源符号身份；验收：示例 JSON 含原始计数、源码摘要、工具/规则版本，并能区分不兼容 series。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。Defined [quality-evidence.schema.json](../../../tools/quality/schema/quality-evidence.schema.json) (`schema_version=1`, `kind=complexity-evidence`, raw counts, source digest/bytes, analyzer/rule/toolchain versions, optional 40-hex commit) with stdlib-only validator [quality_evidence.py](../../../tools/quality/quality_evidence.py); canonical series keys cover kind, analyzer/rule name+version, target/Python toolchain, and source path/digest/bytes; symbol ids are `source_path::kind::qualified_name@start:line:column:sha256[:12]`; `validate` and `compare-series` CLI plus 39 quality tests pass, including fail-closed schema, raw-count, digest, canonical-id, and incompatible-series negative cases in [test_quality_evidence.py](../../../tools/quality/tests/test_quality_evidence.py).

## 2. Close JSON result false positives

- [x] 2.1 [P1][M] 新增错误对象、无关嵌套数组、数字候选和多候选的 parser 回归；验收：修复前断言能抓住假通过，包含等长候选和零结果优先于无关数字的场景。关联：[JSON spec](specs/strict-json-result-counting/spec.md)。
- [x] 2.2 [P1][M] 实施显式路径/受支持自动发现规则，删除任意值回退；验收：正反单元测试通过，根数组及对象包装兼容，非法显式路径不会退回自动模式。关联：[JSON spec](specs/strict-json-result-counting/spec.md)。
- [x] 2.3 [P1][M] 添加跨平台 CLI fixture，进程退出 0 但输出 `duration_ms: 5000` 错误对象；验收：CLI 非零、失败报告和 `RESULT_PARSE_FAILURE`/incomplete evidence 均被断言。关联：[JSON spec](specs/strict-json-result-counting/spec.md)。Structured CLI contracts and the Linux snapshot both pass.
- [x] 2.4 [P1][M] 验证成功、零结果、partial、命令自身失败及显式数值路径；验收：现有 failure codes、minimum、结果 schema 与 CLI contracts 保持兼容。关联：[JSON spec](specs/strict-json-result-counting/spec.md)。`verify::tests::json_parser_compatibility_preserves_success_zero_partial_and_command_failures` and `json_parser_explicit_numeric_path_preserves_count_and_minimum` pass under locked nextest; the existing structured CLI parse-failure contract also passes.
- [x] 2.5 [P1][S] 更新中英文 parser 文档和迁移示例；验收：示例 config check 通过，清楚说明哪些旧自动发现输入现在拒绝。关联：[JSON spec](specs/strict-json-result-counting/spec.md)。Updated `docs/configuration.md`, `docs/configuration.zh-CN.md`, `README.md`, and `README.zh-CN.md` with recognized fields, explicit `count_path` migration, and rejected legacy inputs; `openspec validate strict-json-results-and-risk-based-quality-gates --strict` passes.

## 3. Expand production coverage boundaries

- [x] 3.1 [P2][M] 建立版本化源清单，新增 `app/project/doctor/service-core` 并显式列出其他模块；验收：每个生产范围有唯一归属，不同边界不能互相提高聚合。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。 Versioned exhaustive inventory validates unique ownership of every source, with ten blocking boundaries and explicit informational modules. Evidence: [production coverage candidate](../../../docs/quality/production-coverage.md).
- [x] 3.2 [P2][L] 划分 service-core 和 daemon adapter，保留所有权/清理/heartbeat 决策在 core；验收：混合源范围有审查映射，fake runtime 测试执行实际核心逻辑。关联：[design](design.md#decisions)。 Extracted deterministic command construction and inspection parsing into service-core; reviewed mixed-source mapping keeps lease, cleanup, heartbeat and orchestration blocking. All 301 Rust tests pass, including actual core execution through fake runtimes. Evidence: [production coverage candidate](../../../docs/quality/production-coverage.md).
- [x] 3.3 [P2][M] 处理独立与内联测试、生成及 benchmark-only 范围排除；验收：仅增加测试函数不会提高生产代码 coverage，未命中生产代码仍在分母。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。 Reasoned file and lexed inline exclusions preserve unhit production denominators; test-only additions and generated/benchmark exclusion fixtures do not inflate production counts. Evidence: [production coverage candidate](../../../docs/quality/production-coverage.md).
- [x] 3.4 [P2][M] 聚合扩展边界的 line/function/region 证据并保留原始计数；验收：边界各自及聚合不足 80.0% 均非零，service adapter 单独 informational。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。 Generated JSON/LCOV/Cobertura and separate raw line/function/region counts; candidate correctly returns 1 at 8,856/11,273 blocking lines (78.56%), with adapters informational. Existing CI adoption remains task 8.1; no baseline acceptance. Evidence: [production coverage candidate](../../../docs/quality/production-coverage.md).
- [x] 3.5 [P2][M] 为边界统计增加负向单元测试；验收：空边界、未知路径、重复归属、缺少原始报告和阈值临界值都能正确失败。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。 All 52 quality tests pass, including 13 production coverage tests for empty/unknown/duplicate/missing evidence and exact unrounded threshold boundaries. Rust fmt, Clippy, docs consistency and strict OpenSpec validation pass; project-local config/CI verification is not applicable. Evidence: [production coverage candidate](../../../docs/quality/production-coverage.md).

## 4. Add reproducible function risk evidence

Validation: [GH-92 implementation evidence](../../../docs/quality/gh-92-validation.md)
(301 Rust tests, 85 Python tests, fmt/clippy, docs consistency and strict OpenSpec
validation; project-local config/ci checks not applicable). This covers 4.1–4.5
only and does not accept the remaining change or a production baseline.

- [x] 4.1 [P2][L] 实现复杂度输出与 LLVM 源函数/区域映射及实例去重；验收：同名函数、泛型实例、闭包、未命中函数和源移动 fixture 不会错配或漏报。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。
- [x] 4.2 [P2][M] 输出 `risk.json/risk.md` 和版本化 `crap_line`；验收：`CC=10,cov=0.8` 得到 10.8，阈值比较不用舍入值且每项链接原始证据。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。
- [x] 4.3 [P2][M] 增加 line/function/region 分项和 branch supported/unsupported 状态；验收：无分支插桩不会被呈现为 0%/100%，关键 line/region 门禁不被禁用。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。
- [x] 4.4 [P2][L] 实现 base/head 增量函数集合与 CRAP/high-risk 规则；验收：新增/修改超阈值失败，移动/拆分不能逃逸，未修改历史债务单列不伪称达标。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。
- [x] 4.5 [P2][M] 校验例外记录、series 和证据完整性；验收：缺 base、不兼容工具、陈旧 profile、缺字段/过期例外失败，有效例外不把失败变绿。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。

## 5. Cover orchestration and lifecycle failures

GH-93 本地验收：[逐项证据与命令结果](validation-gh-93.md)；远端 CI 与整体提案接受仍待完成。

- [x] 5.1 [P2][L] 补齐 doctor 各 check kind 的成功和失败测试；验收：调用真实 doctor 边界，覆盖 required/optional 结果而不是只构造报告。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。
- [x] 5.2 [P2][L] 补齐 CLI 路由、selection/profile 及错误退出测试；验收：插桩 `app` 路径被执行，stdout/stderr/退出语义被断言。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。
- [x] 5.3 [P2][L] 补齐 project discovery 和 staged snapshot 回归；验收：暂存有错/工作树无错与反向场景均按 snapshot 判定，失败时快照清理可观察。关联：[traceability spec](specs/critical-path-source-traceability/spec.md)。
- [x] 5.4 [P2][L] 补齐 service-core 续期、清理失败和所有权不确定测试；验收：fake runtime 记录未授权 remove 从未调用，失败保留租约/资源证据。关联：[traceability spec](specs/critical-path-source-traceability/spec.md)。
- [x] 5.5 [P2][L] 固化 verification 的 gate/取消/cleanup/report 错误优先顺序；验收：失败后仍发布应保留证据，取消与失败分类保持独立。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。
- [x] 5.6 [P2][L] 补齐 configured task 的 runner、service env、isolation 和 shard 组合测试；验收：有效参数/环境及报告元数据一致，碰撞/错误配置不能执行。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。

## 6. Decompose selected high-risk functions

GH-94 tasks 6.1–6.6 are locally validated using the operator-authorized
[versioned measurement contract](../../../docs/quality/source-measure-v2.md).
All 312 regressions and 32 selected function gates pass; every production
symbol in the six files has its own observation. The complete
[validation and source/debt evidence](../../../docs/quality/gh-94-validation.md)
retains the prior blocked draft. CI and controller acceptance remain pending.

- [x] 6.1 [P2][L] 按 check kind 拆分 doctor 分发与校验职责；验收：5.1 全部通过，热点及新函数满足 CRAP 和 line/region 标准。关联：[design 热点清单](design.md#decisions)。
- [x] 6.2 [P2][L] 拆分 CLI command handlers，保留统一错误出口；验收：5.2/5.3 与 CLI contract 对比通过，热点风险报告达标。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。
- [x] 6.3 [P2][L] 拆分 verification 协调、结果归并和发布职责；验收：5.5 以及取消/超时测试通过，报告排序不变且热点指标达标。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。
- [x] 6.4 [P2][L] 拆分 task 输入构建、runner/isolation 和环境注入职责；验收：5.6 通过，新旧有效配置的执行输入相同且函数指标达标。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。
- [x] 6.5 [P2][L] 拆分 adapter 的 preflight、进程生命周期和结果验证；验收：签名/能力/预算/超时/取消/路径逃逸回归全部通过且指标达标。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。
- [x] 6.6 [P2][M] 核对 JSON 热点与所有拆分后函数的源身份和测量；验收：选定热点 line/region 各至少 80.0%、`crap_line <= 30`，其他历史债务具有明确清单。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。

## 7. Bind critical paths to source evidence

- [x] 7.1 [P2][M] 升级 inventory，补齐 source/symbol/observable/平台条件和独立 mandatory ID 清单；验收：删除任一强制 row 的 fixture 失败。关联：[traceability spec](specs/critical-path-source-traceability/spec.md)。
- [x] 7.2 [P2][L] 关联隔离关键测试运行与实际源函数/区域；验收：测试 A 不能借测试 B 的覆盖通过，所有证据必须匹配提交与 target。关联：[traceability spec](specs/critical-path-source-traceability/spec.md)。
- [x] 7.3 [P2][M] 将 JSON 假通过、报告完整性和租约所有权场景纳入 mandatory；验收：这些路径及取消/进程树/gate 失败全部通过，适用 matrix 至少 95.0%。关联：[traceability spec](specs/critical-path-source-traceability/spec.md)。
- [x] 7.4 [P2][M] 增加 matrix 负向和故障注入测试；验收：测试跳过/取消、符号移动、混用提交、缺覆盖和 observable 退化均被阻止。关联：[traceability spec](specs/critical-path-source-traceability/spec.md)。

Validation evidence for 7.1–7.4: [GH-95 validation](../../../docs/quality/gh-95-validation.md).

## 8. Integrate gates and document adoption

- [x] 8.1 [P2][L] 将扩展 coverage/risk/matrix 接入 CI 所需质量任务；验收：所需任务失败、取消或跳过使 aggregate 失败，`always()` 上传可用原始证据。关联：[traceability spec](specs/critical-path-source-traceability/spec.md)。
- [x] 8.2 [P2][M] 增加质量工具测试入口与本地命令说明；验收：README 的命令可生成与 CI 相同 schema，质量脚本单测和字节码检查通过。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。
- [x] 8.3 [P2][M] 补充演进 ADR、阈值/例外/基线接受与 rollback 说明；验收：不改写 ADR-0025 历史测量，无自动 baseline accept、阈值降低或分支保护变更。关联：[proposal](proposal.md)。
- [x] 8.4 [P2][M] 生成独立候选基线并记录 CI 采集开销；验收：同口径原始证据完整、无假精度 CRAP 声明，既有性能 fixture 未发生超过原阈值的未解释回归。关联：[quality spec](specs/risk-based-quality-evidence/spec.md)。

GH-96 validation: [raw candidate, local commands, timings and benchmark evidence](../../../docs/quality/gh-96-validation.md); [ADR-0039](../../../docs/adr/0039-required-risk-and-traceability-gates.md). All four stages pass; 315 Rust tests and 119 quality-tool tests pass, including failure/cancel/skip and stale-evidence negatives. Independent candidate collection took 338.72 seconds locally; the unchanged five-sample performance policy reports no regression. These task checkoffs certify local implementation evidence only, not hosted CI, baseline acceptance or the remaining tasks.

## 9. Verify and hand off implementation evidence

- [x] 9.1 [P1][M] 运行 locked nextest、Clippy 和 fmt；验收：原有及新增 Rust 测试无失败，lint/format 通过，保留精确提交与命令结果。关联：[proposal](proposal.md)。
- [x] 9.2 [P2][M] 运行质量工具/发布工具测试、CLI contracts、文档/schema 一致性与扩展门禁；验收：全部必需结果通过，失败 fixture 均返回非零。关联：[proposal](proposal.md)。
- [ ] 9.3 [P2][M] 收集 Linux/macOS/Windows 契约证据并审查平台适用性；验收：没有把本地或 fake runtime 测试当成跨平台/真实容器成功证据。关联：[traceability spec](specs/critical-path-source-traceability/spec.md)。
- [x] 9.4 [P2][S] 更新此 change 的实施状态和验收索引；验收：仅凭实际测试/指标证据勾选任务，运行 `openspec validate strict-json-results-and-risk-based-quality-gates --strict` 通过，未验收项保持未完成。关联：[proposal](proposal.md)。

GH-97 validation: [acceptance index](validation-gh-97.md) and [exact commands, raw evidence, hosted provenance and platform review](../../../docs/quality/gh-97-validation.md). Tasks 9.1/9.2 certify local Linux results at `381d5f418655969d29b81f89069845e6667f35c9`; 9.4 certifies evidence bookkeeping and strict specification validation. Task 9.3 remains unchecked: retained macOS/Windows contracts are from an older SHA, and that run failed macOS tests/benchmarks. No baseline or full-change acceptance is declared.
