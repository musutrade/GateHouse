## Purpose

让质量门禁按实际风险覆盖命令编排、项目输入、服务所有权和高复杂度函数，
同时保留可复现、可审查且不降低现有阈值的证据。

## ADDED Requirements

### Requirement: Blocking coverage boundaries SHALL include high-risk orchestration

质量报告 SHALL 将 `config`、`verify`、`process`、`audit`、`scope`、`secrets`、
`app`、`project` 和 `doctor` 作为阻塞边界；service SHALL 至少拆分为可确定性
核心（阻塞）和真实 runtime adapter（可信息性）边界。每个阻塞边界及其聚合
覆盖率 SHALL 达到 80.0%。

#### Scenario: A newly covered core boundary is below threshold

- **WHEN** 任一阻塞边界低于 80.0%
- **THEN** quality coverage job SHALL fail and publish the raw and summary evidence

#### Scenario: A runtime adapter is informational

- **WHEN** Docker/Podman adapter 因本机没有 daemon 无法收集完整覆盖率
- **THEN** report SHALL 标明 informational，不得提高阻塞聚合，也不得把缺失证据伪装成 pass

#### Scenario: A boundary has no executable production lines

- **WHEN** 阻塞边界没有可执行生产代码
- **THEN** report SHALL fail closed，不得静默跳过；例外只能记录失败，不能改写判定

### Requirement: Coverage denominators SHALL represent production code

质量工具 SHALL 排除版本化清单指定的测试/生成/benchmark-only 代码，
包括内联测试函数；生产源位置 SHALL 只属于一个边界。可测的未命中生产
函数、未归类模块或符号映射错误 SHALL 不得静默丢弃。

#### Scenario: Inline tests increase without production changes

- **WHEN** 增加仅测试源码而没有增加生产执行覆盖
- **THEN** 阻塞生产代码覆盖率 SHALL 不得因为测试代码本身被执行而上升

#### Scenario: Source assignments overlap

- **WHEN** 同一生产范围被分配给 service-core 和 runtime adapter
- **THEN** 清单校验 SHALL 失败，禁止重复计分

### Requirement: Risk metrics SHALL be reproducible and source-traceable

风险报告 SHALL 保存提交身份、目标平台、工具版本、复杂度规则版本、
覆盖口径、函数源文件/行号、原始覆盖数据和生成命令。
`crap_line` SHALL 按 `CC² × (1 - line_coverage_fraction)³ + CC` 计算，
保留规则版本并用未舍入值判定，不得将近似 AST 指标描述为完整分支复杂度。

#### Scenario: The same evidence command is rerun

- **WHEN** 在相同提交、目标和工具版本重新运行分析
- **THEN** 函数清单、原始计数和摘要字段 SHALL 可重建并可比较

#### Scenario: A toolchain or rule version changes

- **WHEN** 工具链、分析器或复杂度规则版本变化
- **THEN** report SHALL 标记为新 series 或不可直接比较，不得静默覆盖旧基线

#### Scenario: CLI subprocess coverage is absent

- **WHEN** 测试调用的二进制未插桩或其 profile 未纳入本次证据
- **THEN** 工具 SHALL 报告 measurement error，不得仅凭测试成功接受基线

#### Scenario: Branch instrumentation is unsupported

- **WHEN** 当前度量工具链不能生成 branch coverage
- **THEN** report SHALL 记录 unsupported、原因和版本，继续强制 line/region 规则，
  不得用 0%、100% 或 line coverage 替代分支指标

### Requirement: Incremental CRAP risk SHALL block unsafe hotspots

所有新增或修改生产函数的版本化 `crap_line` SHALL 不高于 30。
`CC > 10`、绑定强制路径或列入本次热点清单的函数定义为高风险；
这些函数 SHALL 有实际边界测试，函数行及区域覆盖率分别不得低于 80.0%。
未修改历史热点 SHALL 完整报告并单列债务。例外记录 SHALL 包含 issue、
owner、审批人、理由、到期日和补偿控制，但不得把失败自动转换为通过。

#### Scenario: Native coverage omits an expression closure

- **WHEN** a supported production closure has no native LLVM function record
- **THEN** evidence SHALL remain blocked until versioned instrumentation produces
  its own observation and a compiled fixture distinguishes zero from one execution
- **AND** parent counters SHALL NOT be substituted; instrumentation SHALL preserve
  original expressions and inverse mapping SHALL remove only inserted bytes
- **AND** base and head SHALL use the same analyzer, rule, instrumentation and
  mapping implementations; missing, ambiguous or mismatched evidence SHALL fail

#### Scenario: Decomposition retains independently measured closure debt

- **WHEN** GH-94 compares its six original hotspots and 26 extracted named functions
- **THEN** every selected function SHALL meet line AND region coverage >=80.0%
  and `crap_line <=30`, and every production function/closure in those six files
  SHALL retain its own source identity and observations
- **AND** other changed functions SHALL follow the incremental CRAP/high-risk
  rules below; unchanged historical debt and changed low-risk coverage debt SHALL
  remain explicit, and an incremental acceptance SHALL NOT imply coverage passed

#### Scenario: A changed hotspot exceeds the CRAP limit

- **WHEN** 变更函数的 `crap_line` 大于 30
- **THEN** quality aggregate SHALL 失败并列出源位置和修复提示

#### Scenario: A hotspot is split into smaller functions

- **WHEN** 拆分保持外部行为和错误契约不变
- **THEN** 每个新函数 SHALL 单独计量，并由测试证据覆盖实际失败分支

#### Scenario: An approved exception expires

- **WHEN** CRAP 豁免到期
- **THEN** quality aggregate SHALL 失败，直到函数被修复或提交新的审查记录

#### Scenario: A valid exception records technical debt

- **WHEN** 超阈值函数具有有效的临时例外记录
- **THEN** 报告 SHALL 保留原始失败和例外记录，不得自动把 aggregate 变绿

### Requirement: Existing behavior SHALL remain the compatibility baseline

质量指标和边界扩展 SHALL 不改变默认 serial execution、调度依赖、
取消/超时、报告顺序、machine result schema 和现有错误码。

#### Scenario: Quality instrumentation runs on a passing verification

- **WHEN** 以现有有效 preset 运行 verify
- **THEN** 业务结果、报告文件和退出状态 SHALL 与 instrumentation 前一致

#### Scenario: A failure-path test is executed

- **WHEN** 运行取消、超时、服务失败或报告失败测试
- **THEN** 测试 SHALL 验证实际进程/服务/报告边界，而非仅构造 TaskResult

## Implementation Plan and Success Criteria

对应 [design 的 Phase B–D](../../design.md#migration-plan)，预计实施 Day 2–8；
每个阻塞边界至少 80.0%，新增/修改函数 CRAP 和高风险函数 line/region 达标，
并通过源清单、映射、阈值和错误 series 的质量脚本负向测试。

```text
CC = 10, covered = 80, executable = 100
crap_line = 10² × (1 - 0.8)³ + 10 = 10.8
```

## Alternatives and Rollback

全项目单一覆盖率会稀释小型高风险模块；把整个 service 排除会遗漏所有权
决策；直接采用一次性 CRAP 估值则不可重建。新采集器不可靠时保留旧六模块
阻塞门禁并将扩展证据标为失败/未验收，不降低阈值或伪造 pass。
