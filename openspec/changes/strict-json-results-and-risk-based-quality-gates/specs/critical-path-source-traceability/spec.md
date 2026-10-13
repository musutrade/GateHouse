## Purpose

把关键失败路径绑定到真实测试和源代码证据，使模块总体覆盖率通过不能掩盖
取消、清理、解析、报告和输入校验等高影响路径未被执行。

## ADDED Requirements

### Requirement: Critical paths SHALL identify executable evidence

每条关键路径 SHALL 记录稳定 ID、适用平台、源模块/符号、实际测试、
预期观察值、证据文件和规则版本；测试必须执行真实边界，不能只 mock 结果对象。
所有证据 SHALL 匹配同一提交和目标；强制路径 SHALL 有测试到源区域的关联，
不能用同一 suite 中其他测试的源命中代替。适用路径通过率 SHALL 至少 95.0%。

#### Scenario: A critical path has a passing boundary test

- **WHEN** 测试结果、源符号覆盖和预期 observable 均存在
- **THEN** matrix row SHALL 标记 traceable pass

#### Scenario: A test name exists but source evidence is missing

- **WHEN** 测试执行成功但绑定源函数没有覆盖或路径已变更
- **THEN** row SHALL 标记 fail，quality aggregate SHALL 不得通过

#### Scenario: A platform-specific path is not applicable

- **WHEN** 路径声明为其他平台且包含原因和适用性说明
- **THEN** row SHALL 标记 not-applicable，并从当前平台分母中排除

#### Scenario: Evidence belongs to another commit

- **WHEN** nextest、coverage 或 source inventory 的提交/目标身份不一致
- **THEN** matrix SHALL 失败，不得拼接为一次有效验收

### Requirement: Mandatory safety paths SHALL always be blocking

进程取消、进程树清理、报告完整性、JSON 假通过、内置 gate 失败和服务
所有权不确定路径 SHALL 是强制 blocking rows；缺失、取消、失败或陈旧证据
不能被阈值平均稀释。

#### Scenario: A mandatory row is missing

- **WHEN** inventory 中不存在强制 safety path
- **THEN** matrix generation SHALL fail before calculating a percentage

#### Scenario: A mandatory test is skipped or cancelled

- **WHEN** nextest evidence 显示测试 skipped、cancelled 或无结果
- **THEN** corresponding row SHALL fail and quality aggregate SHALL fail

### Requirement: Evidence updates SHALL be reviewable and fail closed

质量 job SHALL 上传原始测试证据、覆盖产物、CRAP 报告和 matrix 摘要；
任何依赖失败、取消或跳过 SHALL 使 Required Quality Aggregate 失败。

#### Scenario: Required PR evidence is cancelled or skipped

- **WHEN** any applicable required job is failed, cancelled, skipped or missing
- **THEN** Required Quality Aggregate SHALL fail, and the evidence job SHALL
  attempt to upload available raw artifacts with `always()`
- **AND** only explicitly push-only jobs SHALL be non-applicable to PRs;
  coverage, risk and matrix collection SHALL be required on both events

#### Scenario: A candidate combines stale or incomplete artifacts

- **WHEN** candidate commit, base or run identity differs from the requested
  collection, a required raw artifact is missing or has a different digest,
  or any collection stage is unsuccessful or incomplete
- **THEN** candidate verification SHALL exit non-zero without accepting a baseline

#### Scenario: Coverage command fails after tests pass

- **WHEN** coverage process exits non-zero
- **THEN** aggregate SHALL fail and retain available test artifacts

#### Scenario: Matrix fixture is stale

- **WHEN** inventory 引用不存在的测试或源符号
- **THEN** validation SHALL fail with actionable ID/path diagnostics

#### Scenario: A reviewed baseline update is accepted

- **WHEN** candidate evidence uses matching series and has reviewed diff
- **THEN** 基线接受流程 SHALL 要求显式批准且保留原始候选证据，不得修改阈值

## Implementation Plan and Success Criteria

对应 [design 的 Phase C–D](../../design.md#migration-plan)，预计实施 Day 4–8；
所有强制 row 和至少 95.0% 的适用 row 通过，删除 mandatory row、
陈旧源符号、跳过测试和混用提交的 fixture 均必须使工具退出非零。

以下只是规划中的 inventory 形状，具体字段在实现中版本化：

```toml
id = "parser.json_unrelated_output"
mandatory = true
source = "src/verify/parser.rs"
symbol = "verify::parser::count_json_results"
observable = "RESULT_PARSE_FAILURE; parser.complete=false; CLI exit non-zero"
```

## Alternatives and Rollback

只验证测试名称或模块整体 coverage 无法证明目标失败分支已执行，故不采纳。
工具故障时保留已有关键路径门禁和原始证据，新关联功能标为未验收；
不得删除强制 row 或把 failed/skipped 证据改成 pass。
