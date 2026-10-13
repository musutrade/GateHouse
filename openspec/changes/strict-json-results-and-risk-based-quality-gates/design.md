# Design: Strict JSON Results and Risk-Based Quality Gates

## Context

动机和授权边界见 [proposal](proposal.md)。本文件描述后续实现，
不是实现已完成的证明。

2026-09-05 在评审基线提交上运行的 290 个 Rust 测试、30 个 Python 测试、
installer 测试、Clippy 和格式检查通过。原六模块阻塞聚合行覆盖率为
83.27%，service 为 70.96%（informational）。这些是本地历史观察，
不是本次新边界的验收基线，也不代表跨平台/远端 CI 已验收。

需纠正评审摘要的口径：23.38% 对应 `doctor/checks.rs`，不是整个 doctor
模块；`project/input.rs` 的 24.83% 是文件级观察。原始 LLVM totals 的
78.33% 与上述六模块聚合不同，不能互换。一次性 AST + LCOV 脚本的 CRAP
排名只用于选取调查对象；函数实例合并、内联测试排除及 CLI 子进程插桩
必须重新验证，不能把其 0% 或精确分值直接复制成 CI 基线。

`strict-standard-result-xml` 已完成 XML 文档边界收紧且明确排除了 JSON；
本次 JSON 契约独立实施。质量边界扩展将演进 ADR-0025 的原始边界，
需要后续 ADR 记录说明，不能改写历史测量的含义。

## Goals / Non-Goals

**Goals:** 将计数策略、minimum 判断、证据发布分开；质量报告具有独立、
版本化源边界；行为测试先于热点拆分；整个度量流程可进行负向自测。

**Non-Goals:** 不增加结果元素内容的业务 schema 校验，不解析通用
`status`/`failure` 字段来替代命令退出码；不强制将未修改的全库历史热点
一次清零；不在此次文档生成中接受基线或改变分支保护。

## Decisions

### 1. JSON counting uses two explicit modes

对应 [JSON spec](specs/strict-json-result-counting/spec.md)。

显式模式保持现有点分 `count_path`；路径值只能是数组或可表示的非负整数。
不添加 JSONPath、数组索引或通配语法；负数、浮点数、缺失路径和错误类型
仍是解析失败。计数为零或低于 minimum 由现有 step 边界分类。

自动模式保留裸顶层数组的长度计数。对于对象，只接受
`testcases`、`testCases`、`test_results`、`testResults`、`results`
这些大小写敏感字段的直接数组值。允许通过对象包装层寻找这些字段，
但不进入结果数组或无关数组的元素继续发现，也不把包装层上的标量当计数。
存在非数组的受支持字段立即失败；恰好一个候选才接受，
多个不同路径的候选即使长度相同也拒绝，要求调用方显式选择路径。

| Input | Auto mode | Explicit mode |
| --- | --- | --- |
| `{"duration_ms":5000,"status":"error"}` | Parse failure | 只有调用方明确选择一个合法数字路径才计数；这是调用方声明，不是自动推断。 |
| `{"metadata":{"attachments":[1,2]}}` | Parse failure | `metadata.attachments` 可按显式声明计数。 |
| `{"results":[{},{}]}` | Count 2 | `results` counts 2 |
| `{"suite":{"results":[{}]}}` | Count 1 | `suite.results` counts 1 |
| `[{},{}]` | Count 2 | 本变更不新增空路径或根路径语法。 |
| `{"results":2}` | Parse failure | `results` counts 2 |
| `{"results":[]}` | Count 0; fails minimum 1 | 同左 |
| `{"results":[{}],"testcases":[{}]}` | Ambiguous; parse failure | `results` counts 1 |

推荐迁移示例（现有运行时配置，不新增配置字段）：

```toml
[parsers.json-results]
kind = "json"
count_path = "summary.total"
minimum = 1
```

计数接口仍返回 count/minimum；语法/形状失败转换为
`RESULT_PARSE_FAILURE`、`parser.complete=false`。命令成功退出但输出错误
对象的 CLI fixture 必须同时断言非零退出、失败报告及 parser evidence；
命令自身非零退出不能被成功解析覆盖。

**Alternatives:** 禁止所有自动发现兼容损失过大；只删数字分支仍会误数
无关数组；选择第一个候选让字段顺序决定成功，故拒绝。整个测试结果成败
schema 校验超出“计数器”的职责。

### 2. Coverage uses production-source boundaries

对应 [quality spec](specs/risk-based-quality-evidence/spec.md)。

将源清单从 Python 常量演进为版本化、可测试清单。阻塞边界为原六模块、
`app`、`project`、`doctor`、`service-core`，每项及聚合至少 80.0%。
其他生产模块仍在完整清单中显式报告，不能因没有阻塞阈值而消失。

service-core 包括状态机、租约/heartbeat、所有权验证、清理决策和
Postgres 外部值校验；初始来源是 `service/mod.rs`、`lease.rs`、
`postgres.rs` 及 `runtime.rs` 中确定性解析/校验逻辑。只有真正依赖 daemon
的启动/探测/命令执行边界属于 runtime adapter。混合文件应先按职责拆分，
或者以可审查的源符号范围分配，不能把整个混合模块排除。

同一生产代码位置只能归属一个边界。排除项必须精确、版本化并有理由，
涵盖测试函数、内联测试模块、生成代码和 benchmark-only 代码；
不得通过搬文件、加入高覆盖测试代码或丢弃未命中项来提高覆盖率。
空边界、路径拼写错误、重叠归属、缺失原始数据均失败关闭。

**Alternatives:** 全项目单一 80% 会稀释输入/清理风险；把整个 service
设为阻塞会把 daemon 可用性混入纯逻辑验收；继续全量 informational
又漏掉可用 fake runtime 测试的安全决策。

### 3. Versioned metrics replace one-off CRAP estimates

开发工具输出 `risk.json` 和 `risk.md`，并保留 LLVM JSON/LCOV/Cobertura、
测试运行证据及复杂度分析器原始输出。分析器是开发依赖，版本、安装方式、
许可和固定样例必须在采纳基线前锁定，不加入发布二进制。

```text
coverage_fraction = covered_production_lines / executable_production_lines
crap_line = CC * CC * (1 - coverage_fraction)^3 + CC
```

CC 规则须明确 `if`、循环、`match` arms/guards、短路运算、`?`、
闭包、嵌套函数和宏的处理。使用分析器原生规则并通过样例冻结，
不同规则版本属于不同 series。报告同时提供原始 covered/total；
阈值比较使用未舍入值，不能用显示百分比重算。

函数身份包含相对文件、限定符号及源码范围/摘要，不能只按函数短名匹配。
覆盖率实例按源位置合并且不重复计分；不可解析、不可映射或支持平台上的
生产函数缺失数据报 measurement-error，不得默认为 100%。
宏/条件编译的不可测范围需要显式说明与审查。

function、line、region coverage 分开输出。若工具链不支持 branch
instrumentation，输出 `branch_status=unsupported`、原因及工具版本，
不写成 0% 或 100%，也不把它声称为已经落地的分支门禁。
核心行门禁和热点区域门禁仍然阻塞。要启用分支阈值须后续同口径基线评审，
不能仅为该指标强制改变发布 Rust 工具链。

采集前检查 CLI 测试实际使用本次插桩二进制，子进程 profile 被收集，
原始证据与源码/目标匹配。清理和隔离仅使用工具自有临时目录，
不得清空开发者共用 Cargo target 或合并陈旧 profile。

### 4. Risk gates use an incremental ratchet

GH-94's 2026-09-07 operator authorization permits a new development measurement
series because the frozen parser rejects production syntax and native LLVM
coverage omits some expression closures. The versioned
[source-measure contract](../../../docs/quality/source-measure-v2.md) uses a
locked Rust AST helper, parsed macro expressions and insertion-only closure
instrumentation in disposable source snapshots. A real compiled fixture must
distinguish an unexecuted closure from one execution; parent counters are never
substitutes. Both base and head use identical tool and rule digests. Inverse
UTF-8 mapping removes only synthetic bytes, retains all original production
symbols, and blocks missing or ambiguous observations. Version 1 remains
reproducible; its numbers cannot be compared to this series.

For tasks 6.1–6.6, the six original functions and all 26 extracted named
responsibilities are explicitly selected. Every one must meet both coverage
thresholds and the CRAP limit below. Independently inventoried closures also
follow the incremental rule below: unchanged debt is recorded, changed low-CC
closures require CRAP <=30, and changed CC >10 closures require both coverage
thresholds. An incremental acceptance never changes a failed coverage result
to a pass. The [GH-94 evidence](../../../docs/quality/gh-94-validation.md)
includes every old/new identity and separately identifies changed coverage debt.
That GH-94 record alone does not certify repository-wide gates or tasks 7–9,
and does not accept a replacement baseline.

GH-96 tasks 8.1–8.4 integrate this bounded series with the original six-module
gate, extended production coverage and GH-95 isolated traceability collection.
Changes to Rust source outside the six measured files fail pending measurement
review, including test-only edits in those files; unmeasured changes cannot
silently pass. PR and push collection use the same candidate schema and explicit
commit/base/run identity. All applicable required results must be `success`;
raw evidence uploads use `always()`. The [evolution ADR](../../../docs/adr/0039-required-risk-and-traceability-gates.md)
defines unchanged thresholds, exceptions, explicit baseline acceptance and
reviewed rollback. The [GH-96 evidence](../../../docs/quality/gh-96-validation.md)
distinguishes local CI-command overhead from hosted runner measurements.

所有新增或修改的生产函数要求 `crap_line <= 30`。高风险函数定义为
`CC > 10` 或出现在强制路径/本次热点清单中；这些函数额外要求函数行、
区域覆盖率分别至少 80.0%，以及断言相关失败行为的测试。

未修改历史函数仍完整输出并列入可追踪债务，不因一次扫描达不到 30
而声称全库合格。比较基于 CI base/head SHA；没有 base 或不兼容 series
时阻止增量通过，先走基线评审。重命名、移动和拆分须记录身份映射；
拆分后函数不能继承旧高风险基线来逃避检查。

临时例外沿用 ADR-0025：issue/owner/审批人/理由/到期日/补偿控制必须
存在，但只是失败的审查记录，不能把超阈值变成自动 pass；
过期或字段缺失本身也是策略失败。本次选定热点的验收不允许以例外替代达标。

选定热点和重构关注点：

| Source / function | Test before decomposition |
| --- | --- |
| `doctor/checks.rs::run_check` | 各 check kind 的成功、失败及 optional/required 行为。 |
| `app/commands.rs::run` | 命令路由、显式 component/profile、hook snapshot 与错误退出。 |
| `verify/mod.rs::run_selected` | gate 失败、取消、服务清理、报告发布及错误优先顺序。 |
| `verify/steps.rs::configured_task` | 参数/环境展开、runner、service 注入、isolation、shard 输入。 |
| `process/adapter.rs::run_with_cancel` | 签名/能力拒绝、输出预算、超时、取消及 artifact 失败。 |
| `verify/parser.rs::count_json_results` | 显式路径、受支持自动发现、无关数据及多候选拒绝。 |

`compat::run` 等其他历史热点完整报告；没有业务修改时先登记债务，
不因排名高就无边界扩大本次重构。拆分目标是降低真实决策耦合，
不是把同一个复杂函数机械拆成不能独立验证的包装层。

### 5. Critical paths bind tests to source and observations

对应 [traceability spec](specs/critical-path-source-traceability/spec.md)。
升级 inventory 版本，记录稳定 row ID、源符号、平台条件、测试身份、
observable、所需证据。覆盖、risk 和 nextest 证据必须源于同一提交/目标；
强制路径使用隔离测试运行的覆盖证据或等价的测试到源区域关联，
不能用整个 suite 中其他测试命中的函数为它背书。

mandatory 集合不只从当前 inventory 自我声明：策略校验器有版本化的
必需 ID 清单，删除强制 row 也失败。至少包括取消、进程树清理、报告完整性、
JSON 假通过、内置 gate 失败和所有权不确定时保留资源。
原 95.0% 的适用路径通过率保留，但每条 mandatory 必须通过。

测试明确断言退出状态、typed failure、报告存在/完整性或无破坏性清理
等 observable。指标工具不推断断言的业务正确性；通过源码审查和故障注入
fixture 验证测试确实能抓住行为退化。跨平台 not-applicable 有原因、
owner 和复核日期，不能用缺工具自动豁免强制路径。

### 6. CI rollout preserves the existing gate

质量脚本新增自测覆盖统计缺失/重复、阈值边界、源符号移动、错误 series、
陈旧 profile、mandatory 缺失/跳过及 branch unsupported。
Required Quality Aggregate 在所需任务 failure/cancelled/skipped 时失败，
失败时仍上传可用证据，基线接受命令不能在 CI 自动执行。

旧六模块门禁在迁移中保留；新指标可先采集候选但不得称为已阻塞。
补足覆盖后再同时启用扩展边界和增量规则，本 change 只有该阶段完成才可关闭。
性能验收复用固定 fixture、可比较 series 及原基准阈值；报告额外 CI 采集耗时，
不能将环境差异当成性能改进。

## Risks / Trade-offs

- [JSON 兼容收紧] → 文档列出精确候选规则，保留显式路径和根数组并提供迁移 fixture。
- [度量误差] → 校验子进程插桩、源映射、实例去重、工具版本；缺证据不通过。
- [CI 成本增加] → 复用构建缓存，隔离强制路径 profile，测量新增耗时而不削减测试断言。
- [热点重构回归] → 先固化行为/失败顺序，以小步改动对比 CLI contracts。
- [服务划分遗漏] → 保留完整源清单及互斥分配，所有权/清理决策不得归入 adapter 豁免区。

## Migration Plan

时间仅为实施获授权后的工程估算，不是交付承诺；任务均拆成小于 4 小时单元。

| Phase | Estimate | Exit evidence |
| --- | --- | --- |
| A | Day 1–2 | JSON 回归、修复和迁移文档；明确 bug 输入不能通过。 |
| B | Day 2–4 | 可信插桩证据、边界清单和版本化 CRAP 候选报告。 |
| C | Day 4–7 | 编排/输入/服务失败路径补测、热点拆分及关键路径关联。 |
| D | Day 7–8 | 扩展门禁启用，跨平台契约、质量工具负向测试和验收证据。 |

JSON 修复可以先独立交付，不等待质量工具完成。后续需要审批的提交/PR/
CI/基线操作由实现阶段授权，不由本轮生成文档推定。

### Rollback

JSON 出现兼容问题时优先使用显式 `count_path` 并保存合法 producer fixture，
不得恢复无关数字自动计数。必要时只回退有问题的兼容调整，保留假通过回归。
结构重构可按组件回退，但测试与失败契约保持不变。

若新采集器不可靠，保留旧六模块阻塞门禁，将新证据标记失败/未验收并修复；
不能输出伪 pass 或把新 change 标记完成。所有候选/历史数据保留独立 series，
不覆盖已接受结果，也不自动修改分支保护。

## Open Questions

- 复杂度分析器的具体实现/版本须通过小型固定样例、Rust 语法支持和许可检查
  后锁定；无论选择何种工具，上述身份、口径和失败关闭契约不变。
- 真实 Docker/Podman 验收使用哪个具备 daemon 的 runner，由实施时确认；
  本地无 daemon 不阻碍 service-core 的 fake-runtime 测试，也不构成真实容器已验收。
