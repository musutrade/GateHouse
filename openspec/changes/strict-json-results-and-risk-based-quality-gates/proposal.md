# Proposal: Strict JSON Results and Risk-Based Quality Gates

**Status:** Implementation issues GH-88–GH-96 completed; GH-97 final Linux verification passes at `381d5f418655969d29b81f89069845e6667f35c9`. Tasks 9.1, 9.2 and 9.4 are supported by retained evidence. Task 9.3 and full-change acceptance remain open: historical macOS/Windows contracts do not certify the final implementation, and the retained macOS run has an unresolved test/benchmark failure. GH-96 required PR CI passed at its recorded merge SHA. See the [GH-97 acceptance index](validation-gh-97.md) and [validation evidence](../../../docs/quality/gh-97-validation.md). No replacement baseline has been accepted.
**Date:** 2026-09-05  
**Review baseline:** `c9101c3191be7a5fd639c64c3781ccb154e0ce34` / v0.3.7

## Why

本次评审复现了 JSON parser 的假通过：未声明 `count_path` 时，只有
`duration_ms: 5000` 的错误对象也被计为 5000 个测试。现有质量门禁虽然通过，
但只阻塞六个核心模块的行覆盖率，不能充分约束命令编排、staged snapshot、
doctor 和服务所有权逻辑，也没有可复现的函数级 CRAP 风险门禁。

## What Changes

- **BREAKING — JSON 自动发现收紧：** 不再把任意数字、无关数组或含糊的多个
  结果字段当成测试数量；保留显式 `count_path` 的数组/非负整数计数接口。
  依赖旧宽松行为的调用方需要显式配置路径，或输出受支持的结果数组。
- 为错误对象、空结果、歧义结果及受支持格式补充 parser 单元测试和 CLI
  回归测试；保留 `RESULT_PARSE_FAILURE`、`RESULT_ZERO`、`RESULT_PARTIAL`
  的现有机器契约。
- 将 `app`、`project`、`doctor` 和可确定性测试的 service-core 纳入阻塞性
  覆盖率边界；真实容器 CLI 适配层继续单列，不混入核心聚合分母。
- 建立版本化的函数复杂度、行/区域覆盖率和 `crap_line` 报告；分支覆盖率
  在工具链支持时单独记录，不用行覆盖率冒充。
- 将关键路径绑定到实际测试、源代码符号及可观察失败行为，不能仅凭模块
  整体通过就认定一条关键路径有证据。
- 先补充行为测试，再对选定的高风险编排函数进行职责拆分，保持调度、
  取消、清理、报告顺序和有效配置行为不变。

## Capabilities

### New Capabilities

- `strict-json-result-counting`：JSON 自动发现、显式路径兼容、失败分类及
  CLI 端到端验收。
- `risk-based-quality-evidence`：扩展覆盖边界、版本化函数风险指标、
  CRAP 增量门禁及高风险函数重构约束。
- `critical-path-source-traceability`：关键路径的源符号与测试证据绑定、
  强制失败路径和 CI 失败关闭规则。

### Modified Capabilities

无。当前仓库没有已发布的 `openspec/specs/` 能力文件；相关历史约束位于
已有 change 和 ADR 中。本变更新增 delta，不重写或重开已完成的 XML、
Phase 1 baseline 和安全修复记录。

## Goals

1. JSON 解析无法仅凭耗时、错误码或无关数组产生测试已执行的假证据。
2. 扩展后的每个阻塞边界及其聚合行覆盖率均达到至少 80.0%。
3. 风险度量可追溯到同一提交、目标平台、工具链、测试运行和源符号；
   不把评审中的近似数值直接接受为质量基线。
4. 新增/修改函数遵循增量 CRAP 门禁；选定热点具有失败路径测试，
   行/区域覆盖率至少 80.0%，`crap_line <= 30`。
5. 取消、进程树清理、报告完整性、JSON 假通过及租约所有权等强制路径
   证据全部通过，其他适用路径继续保持至少 95.0% 可追溯通过率。

## Non-goals

- 初始规划阶段只创建文档；后续按 Issue 授权实施。GH-93 仅实施任务 5.1–5.6，不代表整个提案已接受。
- 不修复未纳入本提案的 scope、调度性能或其他待评审问题。
- 不重新设计 JUnit/TRX，不把“测试数量”扩展为通用测试结果成败解释器。
- 不改变 CLI 参数、flow schema 版本、机器结果 schema 或 DevRail 权限。
- 不新增 OS sandbox、容器提供者，也不把真实容器测试伪装成无外部依赖测试。
- 不用一次性脚本估值证明全项目 CRAP 达标，不要求一次清空所有历史技术债。
- 不更改已有分支保护、发布授权或已接受基线；实现后的基线须独立评审。

## Success Metrics

| Area | Acceptance |
| --- | --- |
| JSON 假通过 | 错误对象和含糊结果在 CLI 中退出非零，报告为失败且 parser 不完整。 |
| JSON 兼容 | 显式路径计数、支持的自动发现、minimum 和错误分类均有正反测试。 |
| 覆盖边界 | 原六模块、`app`、`project`、`doctor`、service-core 各自及聚合均至少 80.0%。 |
| 风险指标 | JSON/Markdown 保留 CC 规则版本、覆盖口径、源码身份、原始计数和原始证据定位。 |
| 热点治理 | 设计中选定热点及其拆分后函数满足覆盖/CRAP 标准，行为契约不退化。 |
| 关键路径 | 每条强制路径具有实际边界测试和源证据；缺失、陈旧或失败证据不能变绿。 |
| CI | 本地与 CI 使用同一规则；Required Quality Aggregate 拒绝所需任务失败、取消或跳过。 |

## Impact

运行时主要影响 `tools/harness-gate/src/verify/parser.rs` 和
`verify/steps.rs`；测试及职责拆分涉及 `app`、`doctor`、`project`、
`process/adapter`、`verify` 与 `service`。

质量工具影响 `tools/quality/coverage.py`、`critical_paths.py`、
`critical_paths.toml`、对应工具测试和 CI 质量任务。复杂度分析只作为开发/
CI 工具，不增加发布二进制依赖；分析器选择、版本和许可检查是实现任务。

配置参考、质量工具 README 和相关 ADR 的后续更新应说明兼容迁移及新增门禁。
当前记录不宣称实现完成，也不宣称远端 CI 或跨平台验收已完成。

## Risk Assessment

**Risk: Medium.** 文档本身无运行时风险；后续 JSON 收紧可能暴露调用方
对隐式计数的依赖，扩展门禁可能揭示真实测试缺口，重构可能改变失败顺序。
控制措施为显式路径迁移、先测试后拆分、同口径重建基线和分阶段验收。
新增指标采集的 CI 耗时须测量；不以缩小源边界或降低阈值消除失败。

## Related Records

- [Design](design.md) / [Tasks](tasks.md)
- [ADR-0025: Phase 1 quality gates](../../../docs/adr/0025-phase-1-quality-baseline-gates.md)
- [ADR-0034: Fail-closed trust boundaries](../../../docs/adr/0034-fail-closed-trust-boundaries.md)
- [ADR-0038: Post-remediation hardening](../../../docs/adr/0038-post-remediation-hardening.md)
- [Phase 1 quality baseline change](../phase-1-quality-baseline-gates/proposal.md)
- [Strict standard XML requirements](../strict-standard-result-xml/specs/standard-result-xml/spec.md)
- [Post-remediation requirements](../post-remediation-hardening/specs/post-remediation-hardening/spec.md)
