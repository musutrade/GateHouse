# 未完成的评审需求跟踪

**收口验收基线：** 2026-09-01，在 `main` 提交
`6e17867cf42139a3ecd73d014dc566d21d08d562` 上复核；该历史证据不随之后的
`main` 合并滚动更新。
**适用范围：** 代码评审、发布治理、DevRail 集成和 ADR/OpenSpec 收口
**文档性质：** 跟踪清单；不改变运行时行为，也不替代对应的 ADR、OpenSpec
或管理员审批。

**后续状态：**代码与记录的最新优先级收口见
[2026-09-01 状态页](priority-followups-2026-09-01.md)；本页的历史验收基线
和外部证据结论不回写。

## 当前结论

PR [#60](https://github.com/musutrade/Harness-Gate/pull/60) 已实现五项
fail-closed 边界，PR [#61](https://github.com/musutrade/Harness-Gate/pull/61)
已完成相关 OpenSpec 收口，PR [#62](https://github.com/musutrade/Harness-Gate/pull/62)
已整理本清单，PR [#63](https://github.com/musutrade/Harness-Gate/pull/63)
已完成 Phase 1 required-check 治理收口。当前剩余工作主要分为三类：

1. **发布或切换前的外部证据**：真实 release、DevRail 环境。
2. **下一发布周期的工程整改**：adapter、安装器、脱敏和资源预算。
3. **记录性收口及后续质量债**：更新过时状态，或为 P2/P3 项建立单独变更。

本清单不再把旧审计中的 R-01 至 R-05 当作未完成项；它们的当前状态见
[已解决项](#5-已解决项)。

## 1. 发布和切换阻断项

### G-01：配置并验证 `main` 的 Required Quality Aggregate 规则集

**状态：已完成（2026-09-01）。**

组织规则集 [21098892](https://github.com/musutrade/Harness-Gate/rules/21098892)
现在为 `active`，通过 `ref_name.include: ["~DEFAULT_BRANCH"]` 匹配 `main`，
要求 CI 的实际聚合检查 `Required Quality Aggregate`，并设置
`strict_required_status_checks_policy: true`。权威的 [main 有效规则 API
响应](https://api.github.com/repos/musutrade/Harness-Gate/rules/branches/main)
已返回该 required status check；非默认分支的同一接口不返回该规则。本次收口
采用的验收证据为
[CI run 33466540997](https://github.com/musutrade/Harness-Gate/actions/runs/33466540997)
在提交 `6e17867cf42139a3ecd73d014dc566d21d08d562` 上显示所有质量 job 和聚合
检查通过。旧的 branch-protection 接口为空不影响组织 Ruleset 的有效结果。

**完成条件：**

- [x] 管理员将 `Required Quality Aggregate` 设为 required check，并确认规则集
  通过 `~DEFAULT_BRANCH` 匹配 `main`；
- [x] 用收口时的 `main` CI 运行确认该检查成功执行；故意失败 PR 的阻断演练属于
  可选的后续运维验证，不作为本次收口的前置条件；
- [x] 将证据链接回 [ADR-0025](adr/0025-phase-1-quality-baseline-gates.md)，勾选
  `phase-1-quality-baseline-gates` 的 task 5.2，并将 ADR/OpenSpec 标记为已实现。

### G-02：取得一次真实、干净环境的 release 完整性证据

**状态：待下一次 tag release；代码路径已具备。**

v0.3.3 的 [发布页](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.3)
目前只有四个平台二进制资产。当前
[release workflow](../.github/workflows/release.yml) 已使用显式 inventory，
并生成 SBOM、`SHA256SUMS`、Sigstore 签名/证书和 provenance；尚缺一次真实 tag
运行，把这些结果在干净环境中逐一验证并作为可审计证据保存。

**完成条件：**

- 下一次不可变 tag 的 release run 成功完成 build、quality、签名、attestation
  和 upload；
- inventory 中的每个二进制、SBOM、inventory 和 checksum manifest 都有匹配的
  checksum、签名/证书和 provenance；
- `release_inventory.py verify` 及离线消费者验证通过，且 release 页面资产集合
  与 inventory 完全相等；
- 将 run、release URL 和验证输出记录到
  [ADR-0032](adr/0032-harness-gate-devrail-capability-contracts.md) 及其
  OpenSpec 任务中。

### G-03：验证真实 DevRail `.arc-flow` 配置映射

**状态：待外部集成验证。**

仓库内的兼容 request、`compat run/compare/canary/rollback` 和固定 fixture 已
完成；真实 DevRail 的 `.arc-flow` 配置、字段映射、请求/结果关联和消费者行为
尚未在 DevRail 环境中验证。Harness-Gate 只负责执行和证据，不接管 DevRail 的
审批、required-check 或组织策略。

**完成条件：**

- 用真实 `.arc-flow` 配置生成版本化 request，并能在不解析 Markdown 或日志的
  情况下消费 machine-result；
- request ID、invocation ID、scope、配置摘要、环境摘要和失败码可端到端关联；
- DevRail 仍明确拥有策略选择、required-check 和发布决策；
- 运行结果、差异分类和保留期限记录在集成证据中。

### G-04：在真实 DevRail 环境执行 shadow、canary 和 rollback

**状态：待外部运行证据。**

本地实现和契约测试已覆盖规范化比较、有限 canary 和原子 rollback 事件，但
OpenSpec 的“实现完成”不等于真实 DevRail 流量已经切换。

**完成条件：**

- shadow 运行在固定观察窗口内产生可解释的等价/差异结果；
- 一个有边界的 canary slice 成功运行，DevRail 在观察期内继续拥有
  required-check；
- rollback 能恢复旧路径，不删除 invocation evidence，也不回收未证明归属的
  资源；
- 资源、日志、报告和发布验证无泄漏，并保存 canary/rollback 事件证据。

## 2. 下一发布周期的工程项

以下项目应分别建立小范围 OpenSpec/PR，避免与治理证据混在同一个收口提交中。

| 编号 | 未完成需求 | 下一步与验收证据 |
| --- | --- | --- |
| **R-06** | adapter 目前只签署 executable declaration；完整 request 的 `args`、`input`、环境、capabilities、timeout 和 invocation 绑定仍不足，缺少 nonce/expiry/replay 防护。 | 设计新协议版本，签署规范化完整 request，并绑定 nonce、有效期、invocation/step ID 与 configuration digest；任一字段篡改、跨 invocation 重放或过期 request 都必须验签失败。 |
| **R-07** | capability allowlist 是协议声明，不是 OS 网络、文件、资源或进程沙箱；`setsid`/进程组也不是完整后代隔离。 | 短期统一 README、配置文档、ADR-0033 和 OpenSpec 的承诺，并为 reader 增加独立 deadline；长期若保留沙箱宣传，再按平台实现并测试可验证的 OS policy。 |
| **R-08** | 已完成：[install.sh](../install.sh) 要求显式 immutable tag，下载到临时目录并校验 SHA256/Sigstore 后原子安装；离线 fixture 覆盖篡改、签名/身份失败、Windows asset、source tag、signal cleanup 和 symlink。PR [#67](https://github.com/musutrade/Harness-Gate/pull/67) 与 [#68](https://github.com/musutrade/Harness-Gate/pull/68) 已合并，ADR-0036/OpenSpec 已标记 Accepted/Implemented，合并后 `main` CI [33495394825](https://github.com/musutrade/Harness-Gate/actions/runs/33495394825) 全绿。 | 实现与离线契约已收口；下一次真实 release 仍需补 G-02 的发布元数据兼容性和 clean-environment 验证。 |
| **R-09** | standalone audit 和 parse-logs 路径可能原样输出 violation 内容或提取日志；verify 的脱敏边界没有覆盖全部输出。 | 复用统一 redaction pipeline，默认只发布文件、行号、规则和脱敏摘要；token、Bearer、数据库 URL、Authorization header 和 private key fixture 不得出现在 JSON、Markdown、stdout 或错误上下文。 |
| **R-10** | 已完成：tag/source/版本/完整 `main` CI 的 eligibility 校验已接入 release workflow；tag ruleset [21989651](https://github.com/musutrade/Harness-Gate/rules/21989651) 与受保护 `release` environment 已配置。PR [#66](https://github.com/musutrade/Harness-Gate/pull/66) 的 20 项 CI 检查（含 `Required Quality Aggregate`）通过并合并。 | 下一次 immutable tag 的真实发布和 clean-environment 验收仍属于 G-02；CI 工具安装与 coverage 选择作为独立维护债，不扩大 R-10 发布阻断边界。 |
| **R-11** | adapter/capture 使用无界 `read_to_end`，单步日志和磁盘使用没有统一 quota；逃逸后代可能让 reader 无界等待。 | 增加流式读取、单步/单 invocation 字节上限、磁盘预算、截断标记和 reader deadline；超限必须产生结构化失败且保留可审计的部分证据。 |

## 3. P2/P3 后续质量债

这些项目当前不阻断已实现的五项 trust-boundary 修复，但应纳入后续排期并为每项
补充测试或基准：

| 编号 | 需求 | 建议验收 |
| --- | --- | --- |
| **R-12** | XML parser 接受多个根元素。 | 按 JUnit/TRX 允许的根元素和结构校验；多根、缺根和尾随内容 fail closed。 |
| **R-13** | 不可信配置下 webhook 可能访问 loopback、RFC1918 或 link-local 地址，存在条件性 SSRF。 | 默认拒绝私网/本地链路目标，使用显式 host allowlist，并在连接后重新校验解析地址。 |
| **R-14** | 非 Linux 长步骤缺少可靠 heartbeat，可能在 lease TTL 后被误回收。 | heartbeat 覆盖完整资源生命周期；没有可靠平台身份时宁可保留资源，不得猜测归属。 |
| **R-15** | 运行期错误码、retry 分类和配置诊断依赖展示字符串。 | 引入 `FailureCode` 和结构化诊断对象，禁止用 `contains` 或展示文本反推机器契约。 |
| **R-16** | 固定 100ms 轮询和 scheduler 线性扫描带来低风险性能开销。 | 使用平台 wait primitive 或短起始值指数退避，并用节点索引/入度计数降低扫描成本；补同系列基准。 |
| **R-17** | 配置校验 clone、字符串枚举、环境变量命名和 `panic = abort`/`catch_unwind` 语义存在维护风险。 | 改用校验上下文、闭集 enum 和统一 `HARNESS_GATE_*` 命名；补 lock poisoning 与发布 profile 差异测试。 |
| **R-18** | 仓库卫生、社区文件和质量脚本治理不完整。 | 处理 `__pycache__`/一次性脚本，补 `SECURITY.md`、MSRV、Issue/PR 模板，让质量脚本自身受 lint/test 约束，并修正文档中 secrets 扫描语义。 |

## 4. 记录性收口

这些不是新的代码缺陷，而是现有实现与记录状态不一致。D-03 至 D-05 已在
2026-09-01 完成记录收口；D-02 仍需等待外部证据，D-06 有意保持 Proposed：

| 编号 | 记录 | 当前不一致 | 收口动作 |
| --- | --- | --- | --- |
| **D-01** | [Phase 1 OpenSpec tasks](../openspec/changes/phase-1-quality-baseline-gates/tasks.md) 与 [ADR-0025](adr/0025-phase-1-quality-baseline-gates.md) | 已收口：task 5.2 已勾选，ADR 已 Accepted，规则集适用范围和收口 green evidence 已记录。 | 已完成；故意失败 PR 若需要，作为独立运维演练跟踪。 |
| **D-02** | [DevRail capability OpenSpec](../openspec/changes/harness-gate-devrail-capability-contracts/.openspec.yaml) 与 [ADR-0032](adr/0032-harness-gate-devrail-capability-contracts.md) | metadata 为 `implemented-pending-ci`，proposal/ADR 仍是 `Proposed`；真实 release/canary 证据尚未完成。 | 先完成 G-02 至 G-04，再统一更新 proposal、tasks、metadata 和 ADR 状态。 |
| **D-03** | [parallel-scheduling metadata](../openspec/changes/parallel-scheduling/.openspec.yaml) | 已收口：metadata 为 `implemented`，ADR-0028、PR #36 和 green CI 证据保持关联。 | 已完成；未重新实现 scheduler。 |
| **D-04** | [project-scoped-configuration proposal/tasks](../openspec/changes/project-scoped-configuration/proposal.md) | 已收口：proposal/tasks 均标记为 Implemented，并记录 PR #38 与 green CI。 | 已完成；运行时行为未改变。 |
| **D-05** | [close-refactor-documentation README](../openspec/changes/close-refactor-documentation/README.md) 与 [ADR-0021](adr/0021-refactor-documentation-closeout.md) | 已收口：README/OpenSpec 标记为 implemented，ADR-0021 为 Accepted，并记录 PR #28/#32、CI 及 PR #57 的 Phase 2 观察证据。 | 已完成。 |
| **D-06** | [report-template-renderer proposal](../openspec/changes/report-template-renderer/proposal.md) | `Proposed` 是有意保留的设计状态，不应误标为完成。 | 由产品/维护者决定渲染模式、schema、兼容性和 sandbox 约束；决定前不扩大配置契约。 |

## 5. 已解决项

旧评审中的以下五项已由 PR #60 实现，并由
[ADR-0034](adr/0034-fail-closed-trust-boundaries.md) 接受：

| 原编号 | 已解决边界 | 证据 |
| --- | --- | --- |
| R-01 | hook 的 staged invocation input 统一 | [PR #60](https://github.com/musutrade/Harness-Gate/pull/60) 的快照/回归测试；跨平台 CI 通过。 |
| R-02 | lease 与 runtime object ownership fail closed | PR #60 的 label、immutable identity 和 cleanup 契约测试。 |
| R-03 | invocation evidence/manifest 闭集校验 | artifact registry、digest、缺失/逃逸证据测试。 |
| R-04 | 安全的原子输出发布，拒绝 symlink 目标 | safe publisher 及 standalone writer 测试。 |
| R-05 | 单一 release asset inventory 覆盖 SBOM、checksum、签名和 provenance | `release_inventory.py` 测试、PR #60 CI；G-02 只是在真实 tag 上补运行证据。 |

适配器协议本身也已经作为独立的 P2 设计/实现记录在
[ADR-0033](adr/0033-signed-out-of-process-adapter-protocol.md) 和
[harness-gate-adapter-protocol](../openspec/changes/harness-gate-adapter-protocol/proposal.md)；
R-06/R-07 是对其信任边界和文案的后续强化，不是重新开启协议实现。

## 6. 推荐执行顺序

1. **下一次发布完成 G-02**，把 inventory、SBOM、checksum、签名和 provenance 的
   clean-environment 结果保存为 ADR-0032 的证据。
2. **在 DevRail staging 环境完成 G-03/G-04**，再决定是否允许 required-check
   ownership 迁移。
3. **按 R-06 至 R-11 分拆工程变更**；每个变更都要有失败路径测试、兼容性说明和
   rollback 方案。
4. **G-02 至 G-04 完成后收口 D-02**；D-03 至 D-05 已完成，R-12 至 R-18
   进入后续迭代 backlog。
5. **D-06 保持 Proposed**，直到渲染契约和安全边界得到明确决策。

## 7. 完成定义

本轮“评审需求已整理完毕”不等于所有整改已完成。可以宣称治理闭环完成的最低
条件是：

- G-01 有管理员确认和新的阻断性 CI 证据；
- G-02 有真实 tag release 的完整 inventory 验证结果；
- G-03/G-04 有 DevRail 真实环境的 mapping、shadow、canary 和 rollback 记录；
- R-06 至 R-11 各自有明确 owner、目标版本和验收测试；以及
- D-01、D-03 至 D-05 的状态不再与已合并事实矛盾；D-02 在外部证据完成后
  收口，D-06 保持显式 Proposed。

## 参考

- [ADR-0034：Fail-Closed Trust Boundaries](adr/0034-fail-closed-trust-boundaries.md)
- [ADR-0032：Harness-Gate 与 DevRail 能力边界](adr/0032-harness-gate-devrail-capability-contracts.md)
- [ADR-0025：Phase 1 Quality Baseline Gates](adr/0025-phase-1-quality-baseline-gates.md)
- [Fail-Closed OpenSpec](../openspec/changes/fail-closed-trust-boundaries/proposal.md)
- [DevRail Capability Contracts OpenSpec](../openspec/changes/harness-gate-devrail-capability-contracts/proposal.md)
- [Release workflow](../.github/workflows/release.yml)
- [CI workflow](../.github/workflows/ci.yml)
