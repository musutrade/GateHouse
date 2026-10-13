## Purpose

为 JSON 测试结果建立保守、可审计且向后兼容的计数契约，避免错误诊断、
耗时字段或任意嵌套数字被误报为测试已执行。

## ADDED Requirements

### Requirement: JSON result counts SHALL use an explicit or recognized result shape

当配置 `count_path` 时，系统 SHALL 只接受该路径上的数组或非负整数；
当未配置 `count_path` 时，系统 SHALL 接受裸顶层数组，或恰好一个受支持
字段的直接数组值。受支持字段为大小写敏感的 `testcases`、`testCases`、
`test_results`、`testResults`、`results`。系统 SHALL 允许穿过对象包装层
寻找候选，但不得进入数组元素继续发现，不得把任意数字或无关数组解释为测试数量。

#### Scenario: An explicit array path is configured

- **WHEN** `count_path` 指向 JSON 数组
- **THEN** parser SHALL 返回数组长度并继续应用 configured minimum

#### Scenario: An explicit numeric path is configured

- **WHEN** `count_path` 指向非负整数
- **THEN** parser SHALL 返回该整数并继续应用 configured minimum

#### Scenario: An unrelated duration is the only number

- **WHEN** JSON 只包含 `duration_ms`、状态和错误消息而没有受支持的结果字段
- **THEN** parser SHALL 返回 `RESULT_PARSE_FAILURE`，不得满足 minimum

#### Scenario: An unrelated nested array is present

- **WHEN** JSON 包含日志、元数据或附件数组但没有受支持的结果字段
- **THEN** parser SHALL 返回 `RESULT_PARSE_FAILURE`，不得把该数组长度作为测试数

#### Scenario: A bare root array is supplied

- **WHEN** 未配置 `count_path` 且文档根是数组
- **THEN** parser SHALL 返回该数组长度，保留现有根数组兼容

#### Scenario: A recognized array is inside an object wrapper

- **WHEN** 文档只有一个候选，例如 `{"suite":{"results":[{},{}]}}`
- **THEN** parser SHALL 返回 2 并应用 minimum

### Requirement: Automatic discovery SHALL reject ambiguous result documents

未配置 `count_path` 的 JSON 文档如果含有多个同等候选结果字段、候选字段
类型错误或数字候选，系统 SHALL fail closed；错误 SHALL 可追溯到 parser
失败，而不是被转换成成功计数。

#### Scenario: Multiple recognized result fields disagree

- **WHEN** JSON 同时包含 `results` 和 `testcases` 且二者计数不同
- **THEN** parser SHALL 返回 `RESULT_PARSE_FAILURE`

#### Scenario: A recognized result field is not countable

- **WHEN** 自动模式中的 `results` 是对象、字符串或数字而不是数组
- **THEN** parser SHALL 返回 `RESULT_PARSE_FAILURE`

#### Scenario: Multiple recognized fields have equal lengths

- **WHEN** 两个不同路径的候选数组长度相同
- **THEN** parser SHALL 拒绝歧义，要求显式 `count_path`

#### Scenario: An explicit path has an invalid count

- **WHEN** 显式路径缺失或其值是对象、布尔值、null、负数、浮点数或不可表示的整数
- **THEN** parser SHALL 返回 `RESULT_PARSE_FAILURE`，不得回退到自动发现

#### Scenario: No result shape is present

- **WHEN** JSON 文档没有显式路径且不存在受支持的结果字段
- **THEN** parser SHALL 返回 `RESULT_PARSE_FAILURE`

### Requirement: Existing parser failure semantics SHALL remain stable

解析失败、零结果和部分结果 SHALL 保持现有机器结果字段、failure code、
minimum 和退出非零语义；成功结果 SHALL 继续写入 parser evidence。

#### Scenario: A supported result array is empty

- **WHEN** `results` 是空数组且 minimum 为 1
- **THEN** step SHALL fail with zero-result evidence and non-zero CLI status

#### Scenario: A supported result array meets minimum

- **WHEN** `results` 数组长度达到 minimum
- **THEN** step SHALL pass and report observed count, minimum and complete=true

#### Scenario: Malformed JSON is supplied

- **WHEN** output is not valid JSON
- **THEN** step SHALL fail with parser failure evidence and SHALL NOT pass by exit code alone

#### Scenario: A valid count is below minimum

- **WHEN** 合法结果数量大于零但小于 configured minimum
- **THEN** step SHALL 返回 `RESULT_PARTIAL`，parser.complete=false，CLI 退出非零

#### Scenario: A command succeeds while its result document is unrelated

- **WHEN** 进程退出 0，只输出含 `duration_ms: 5000` 的错误对象
- **THEN** CLI SHALL 退出非零，报告 SHALL 包含 `RESULT_PARSE_FAILURE` 和 incomplete parser evidence

### Requirement: JSON parser behavior SHALL be documented and migrated explicitly

配置文档 SHALL 说明自动发现的受支持字段、显式 `count_path` 语法和不受支持的
通用 JSON；迁移指南 SHALL 提供旧配置的显式路径示例。

#### Scenario: A user follows the documented migration

- **WHEN** user adds an explicit `count_path` to an existing result producer
- **THEN** config check SHALL pass and verification SHALL preserve the producer's count

## Implementation Plan and Success Criteria

对应 [design 的 Phase A](../../design.md#migration-plan)，预计实施 Day 1–2；
验收以全部正反场景的单元及 CLI 回归通过为准，不以文档生成作为实现完成。

```toml
[parsers.json-results]
kind = "json"
count_path = "summary.total"
minimum = 1
```

## Alternatives and Rollback

“继续递归接受任意值”和“总取第一个候选”均被拒绝，因为不能可靠排除假通过；
强制所有调用方使用显式路径则兼容代价过大。回退时保留错误对象拒绝规则，
使用显式路径迁移受影响 producer，不能恢复任意数字计数。
