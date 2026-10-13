# Implementation Tasks

本清单对应 [proposal](proposal.md) 和 [design](design.md)。前置 Issue 系列已闭环，
任务 0.3–10.4 的 merged PR、required CI 和验收证据已汇总于
[closure ledger](../../../docs/quality/architecture-closure.md)。GH-119 完成最终 proposal/closure 准备；最终 CI 和控制器验收仍待完成。迁移期间
现有 Rust required gates 始终是发布权威，generic path 先以 shadow/equivalence
模式运行。

S = 小于 1 小时，M = 1–2 小时，L = 2–不足 4 小时；超出估算时继续拆分。
每个任务必须有独立验证证据；本地验证不代表远端 CI、基线或架构接受。

## 0. Entry criteria and scope freeze

- [x] 0.1 [P0][S] 确认当前 `strict-json-results-and-risk-based-quality-gates` 相关 Issues 已完成、关闭或明确移出范围；验收：前置 Issue 系列已闭环，本 change 以 `main` `ad54d8df6d21d3f6e3a0b5ee83918ae078a84d61` 为 review baseline，不隐式接管旧 Issue。
- [x] 0.2 [P0][S] 冻结本 change 的非目标和迁移边界；验收：明确“不重写 Rust 指标语义、不在本 change 实现 Angular/Python/Java 全套 adapter、不更改 branch protection/required-check identity”；现有 Rust required path 在 equivalence acceptance 前保持 authoritative。
- [x] 0.3 [P1][M] 建立架构迁移兼容性清单；验收：至少列出 `ci_quality.py collect/verify/aggregate`、candidate schema 1、production coverage、GH-94 Rust risk series、isolated critical-path matrix、`quality-evidence.schema.json`、artifact digests、`Quality Coverage and Critical Paths` 与稳定的 `Required Quality Aggregate` consumers，并标记必须保持兼容的机器契约。

## 1. Define the language-agnostic project model

- [x] 1.1 [P1][M] 定义 `Project`、`Component`、`Target`、`SourceBoundary`、`Subject` 和 cross-component relationship schema；验收：同一 fixture 声明 Angular、Rust、Python、Java 四组件且无需 core language branch。
- [x] 1.2 [P1][M] 定义 versioned subject kinds 与 generic subject identity；验收：函数/方法同名、路径冲突、缺 digest、重复 identity、未知 kind 均 fail closed。
- [x] 1.3 [P2][M] 定义 rename/move/split identity mapping；验收：无显式 mapping 时不能继承历史有利基线，歧义 mapping 失败。
- [x] 1.4 [P2][S] 建立 synthetic multi-component fixtures；验收：fixture 不依赖真实语言工具链，也能覆盖 component graph、subject identity 和 cross-component relation。

GH-111 validation: [compatibility inventory](../../../docs/quality/migration-compatibility.md)
(task 0.3), [project/identity/mapping contracts](../../../docs/quality/project-model.md)
(tasks 1.1–1.3), and [synthetic fixtures](../../../tools/quality/fixtures/project-model/README.md)
(task 1.4). [Local results and retained evidence](../../../docs/quality/gh-111-validation.md)
cover positive and fail-closed cases. Required CI and full architecture acceptance
remain pending; no later task is accepted by this implementation.

## 2. Introduce `harness-evidence/v1`

- [x] 2.1 [P1][M] 定义并新增 `harness-evidence/v1` normalized envelope schema；验收：包含 project/component、collector、series、subject、metrics、capabilities、commit/target/run、artifacts、status 和 source integrity 字段；不得原地重解释现有 Rust `quality-evidence.schema.json`。
- [x] 2.2 [P1][M] 实现 schema validator 与 canonical serialization；验收：缺字段、未知状态、非法 metric value、重复 evidence ID、错误 digest 均失败关闭。
- [x] 2.3 [P1][M] 定义 raw artifact linkage 和 artifact digest 规则；验收：路径越界、缺 artifact、digest 不符和 stale artifact 报 measurement_error。
- [x] 2.4 [P2][M] 定义 generic metric namespace 和 typed value forms；验收：ratio、raw covered/total、integer count、boolean、duration/size 等不通过无类型 JSON 数字混用。
- [x] 2.5 [P2][S] 增加 Rust/TypeScript/Python/Java synthetic evidence samples；验收：相同 generic policy 能读取不同 ecosystem series，不宣称指标 series 可互换。

## 3. Add capability and measurement-series contracts

- [x] 3.1 [P1][M] 实现 capability 状态：`supported/unsupported/not_configured/not_collected/measurement_error/not_applicable`；验收：任何非 supported 状态都不能被转换为 0%/100%。
- [x] 3.2 [P1][M] 定义 measurement-series identity；验收：tool/rule/identity/normalization 语义变化产生不兼容 series，base/head compare 拒绝静默混用。
- [x] 3.3 [P2][M] 定义 capability requirement policy；验收：required metric unsupported 时按 policy 产生 blocked/measurement_error，而 informational metric 可保留 unsupported。
- [x] 3.4 [P2][M] 增加 series/capability negative fixtures；验收：缺 base、错误 series、tool version 漂移、能力谎报后缺证据均失败关闭。

GH-112 validation: [normalized schema and contract](../../../docs/quality/harness-evidence.md)
(tasks 2.1–2.4, 3.1–3.3), [four-ecosystem and negative fixtures](../../../tools/quality/fixtures/harness-evidence/README.md)
(tasks 2.5, 3.4), and [local results with retained machine evidence](../../../docs/quality/gh-112-validation.md).
The focused suite validates all nine scoped tasks, including artifact integrity,
typed values, six distinct capability states and incompatible series rejection.
Required CI remains pending; later tasks and full architecture acceptance remain unchecked.

## 4. Separate collector adapters from policy

- [x] 4.1 [P1][M] 定义 collector request/response protocol；验收：request 包含 project/component、commit/target、requested capabilities、workspace/output roots 和 collection parameters，response 只返回 evidence/artifacts/error，不返回最终 release decision。
- [x] 4.2 [P1][L] 实现 core collector runner 与 evidence validation boundary；验收：subprocess 非零、超时、输出越界、malformed JSON、未声明 artifact 都有 typed failure。
- [x] 4.3 [P2][M] 支持 internal/reference adapter 使用与 external subprocess 相同的 evidence contract；验收：policy engine 不需要知道 adapter 是内部还是外部。
- [x] 4.4 [P2][M] 增加 synthetic collector fixtures；验收：至少覆盖成功、unsupported、measurement error、stale commit、duplicate subject、artifact tamper。

GH-113 validation: [collector protocol and boundary](../../../docs/quality/collector-protocol.md)
(tasks 4.1–4.3), [synthetic adapter fixtures](../../../tools/quality/fixtures/collectors/README.md)
(task 4.4), and [local results and retained evidence](../../../docs/quality/gh-113-validation.md).
The focused collector suite validates both transports and typed negative paths.
Required CI remains pending; later tasks and full architecture acceptance remain unchecked.

## 5. Implement the generic policy engine

- [x] 5.1 [P1][L] 定义 typed policy schema 和 scope selectors；验收：支持 project/component/boundary/changed-subject/critical-subject scopes，不引入任意代码 DSL。
- [x] 5.2 [P1][M] 实现 generic numeric/boolean/count comparisons；验收：coverage、CRAP、mutation、breaking-change count 等 synthetic metrics 通过同一 evaluator。
- [x] 5.3 [P1][M] 实现 gate result states：`pass/fail/warning/informational/skipped/unsupported/not_applicable/measurement_error/blocked`；验收：`fail` 与 `measurement_error` 在机器结果和 aggregate 中保持不同原因。
- [x] 5.4 [P1][M] 实现 fail-closed aggregate；验收：required child gate failure/cancel/measurement_error/blocked/skipped 按配置不能变绿，informational 不影响 required aggregate。
- [x] 5.5 [P2][M] 输出 machine-readable violation/remediation contract；验收：Agent 可获得 component、subject、metric、base/head、limit、series、evidence links 和 remediation class。

Validation evidence for 5.1–5.5 (GH-114): the
[policy contract](../../../docs/quality/policy-engine.md),
[focused evaluator tests](../../../tools/quality/tests/test_policy_engine.py), and
[actual local validation record](../../../docs/quality/gh-114/validation-summary.json)
cover typed selectors/comparisons, all distinct result states, required-child
aggregation and agent remediation records. This is a standalone shadow evaluator;
GH-115 ratchet validation is recorded below; Rust migration and required-gate
equivalence remain unaccepted.

## 6. Generalize baseline, debt and ratchet

- [x] 6.1 [P1][L] 实现 compatible base/head evidence comparison；验收：新/修改/未修改 subject 分类稳定，缺 base 或 incompatible series 阻止增量 pass。
- [x] 6.2 [P1][M] 实现 debt ledger 与 no-regression ratchet；验收：未修改 legacy debt 可单列，新增 debt 失败，改善 debt 记录 improved，不伪称全库合格。
- [x] 6.3 [P2][M] 实现 absolute threshold 与 regression policy 同时存在；验收：例如 base CRAP 64 -> head 55 可标 improved/debt，而新函数 CRAP 31 仍失败。
- [x] 6.4 [P2][M] 接入 exception metadata 但保持 exception 不自动变 pass；验收：owner/issue/reason/expiry/compensating control 缺失或过期失败，exception 只改变审查状态。

Validation evidence for 6.1–6.4 (GH-115): the
[policy contract](../../../docs/quality/policy-engine.md),
[acceptance fixtures](../../../tools/quality/tests/test_policy_ratchet.py), and
[actual local validation record](../../../docs/quality/gh-115-validation.md)
cover compatible history, stable explicit identity classification, conservative
split treatment, debt/trend alongside absolute limits, and exception review that
preserves quality failures. Only these shadow-engine tasks are implemented;
production baseline acceptance and remaining proposal tasks are not claimed.

## 7. Migrate Rust as the reference adapter

- [x] 7.1 [P1][L] 将现有 Rust production coverage evidence 包装为 generic evidence，不改变 raw counts/series；验收：现有 coverage 报告与 generic projection 数值、source digest 和 gate outcome 一致。
- [x] 7.2 [P1][L] 将现有 Rust complexity/CRAP evidence 包装为 generic function-risk evidence；验收：现有 `crap_line`、CC、line/function/region raw counts 和 subject identity 不被重新解释。
- [x] 7.3 [P1][M] 将 Rust capability 状态映射到 generic model；验收：现有 branch unsupported 等语义保持。
- [x] 7.4 [P1][L] 在 shadow mode 运行 current evaluator 与 generic policy engine；验收：复用同一 candidate/raw evidence、base/head/run identity，所有 required Rust gate outcome、debt classification、unsupported/measurement-error 语义一致；任何差异产生 compatibility failure，不替换现有 required gate。
- [x] 7.5 [P1][M] 增加 historical fixture compatibility tests；验收：至少覆盖 accepted baseline、failed gate、measurement error、legacy debt、changed-function ratchet。

Validation evidence for 7.1–7.5 (GH-116): the
[Rust shadow adapter contract](../../../docs/quality/rust-reference-adapter.md),
[historical acceptance tests](../../../tools/quality/tests/test_rust_reference.py),
and [local validation record](../../../docs/quality/gh-116-validation.md) cover
unchanged raw counts, source digests, exact/display CRAP, identities, debt,
capabilities and outcomes on the same retained candidate. Negative fixtures
block mismatches and required-stage failures without collecting again. The
accepted Phase 1 baseline retains its incompatible original series; the GH-97
passing candidate is not relabeled as an accepted baseline. Current Rust gates
remain authoritative; full migration, rollout and proposal acceptance are pending.

## 8. Add cross-component contract abstractions

- [x] 8.1 [P2][M] 定义 contract subject 和 participating component relationship；验收：一个 synthetic OpenAPI contract 可同时绑定 frontend/client 与 backend/provider。
- [x] 8.2 [P2][M] 定义 generic contract metrics：schema valid、breaking changes、generated-client drift、compatibility status；验收：不依赖具体 OpenAPI 工具即可跑 synthetic policy fixtures。
- [x] 8.3 [P2][M] 验证 project aggregate 可同时包含 component-local gate 与 cross-component gate；验收：两个组件本地全绿但 breaking contract 仍阻止 project pass。

## 9. Configuration and reporting migration

- [x] 9.1 [P2][M] 决定 component manifest 落点并扩展 config/schema；验收：旧单组件 Rust 配置仍兼容或有明确迁移错误，不静默改变默认路径。
- [x] 9.2 [P2][M] 增加 project/component/gate machine report；验收：报告可按 component、subject、policy、status 聚合，并保留 raw evidence link。
- [x] 9.3 [P2][M] 更新 CLI/docs 示例；验收：展示单 Rust 项目和 Angular+Rust+Python+Java synthetic 项目，不宣称后者 adapter 已实现。

Validation evidence for 8.1–8.3 and 9.1–9.3 (GH-117): the
[contract/config/report decision](../../../docs/quality/project-reporting.md),
[synthetic fixtures](../../../tools/quality/fixtures/contracts/README.md),
[12 acceptance tests](../../../tools/quality/tests/test_project_report.py), and
[local validation record](../../../docs/quality/gh-117-validation.md) demonstrate
explicit provider/consumer bindings, four generic contract metrics, and a
blocking project result despite green component-local gates. Compatible and
multiple-consumer fixtures, missing/tampered provenance and configuration
rejection cases are covered. The additive JSON CLI retains raw links and
component/subject/policy/status indexes without changing Rust `flow.toml`
defaults. Non-Rust and contract adapters remain synthetic; required hosted CI,
CI rollout and architecture acceptance are pending.

## 10. CI rollout and architecture acceptance

- [x] 10.1 [P1][M] 增加 generic shadow CI job；验收：优先消费 `ci_quality.py collect` 已产生的 candidate/raw artifacts，不重复执行昂贵 Rust test/coverage/risk/matrix collection，上传 projection 与 compatibility evidence。
- [x] 10.2 [P1][M] 验证 `Required Quality Aggregate` 在 shadow 模式不被新架构替换；验收：check name、existing required dependencies 和 fail-closed semantics 保持不变，current required gates 继续是唯一发布权威直到独立 equivalence acceptance。
- [x] 10.3 [P1][L] 完成 Rust equivalence acceptance；验收：规定次数/提交范围内 old/new 结果无未解释差异，measurement-error negative fixtures 通过。
- [x] 10.4 [P1][M] 编写 ADR 记录 language-agnostic evidence/policy architecture；验收：明确 Rust 是 reference adapter、series 不统一、collector 不决定 pass/fail、fail-closed trust boundary。
- [x] 10.5 [P1][S] 关闭本 change 前明确下一 change；验收：创建独立的 TypeScript/Angular reference-adapter proposal，用真实第二生态验证抽象，而不是在本 change 偷带实现。

Validation evidence for 10.1–10.4 (GH-118): the
[shadow workflow and frozen aggregate tests](../../../tools/quality/tests/test_rust_equivalence.py),
[defined two-run window](../../../docs/quality/rust-equivalence-acceptance.md),
[ADR-0040](../../../docs/adr/0040-language-agnostic-evidence-policy.md) and
[local validation record](../../../docs/quality/gh-118-validation.md) retain the
completed acceptance evidence. Both pinned runs at distinct head commits replayed
without outcome/debt/capability differences; all 15 adversarial adapter fixtures
passed, including measurement errors and unsupported-state drift. All 315 Rust
and 207 Python tests, formatting, Clippy, docs consistency and strict OpenSpec
validation passed locally. The required aggregate and Rust release authority
remain unchanged. Hosted required CI must pass before merge; retrospective
acceptance does not certify a hosted soak or replace any required gate. These are
submission-time results; the closure ledger records subsequent merged acceptance. Final GH-119 required CI and controller acceptance remain pending.

## Delivery Rule

本 change 不应被单个大型 Issue 一次实现。建议按上述 section 拆成多个可独立验收的 GitHub Issues；每个 Issue 必须创建 PR、等待 required CI、保留兼容/负向证据，并由控制器完成 merge/closure。任何超过 L 的实现必须继续拆分。

## GH-119 final closure evidence

Task 10.5 creates the independent
[TypeScript/Angular proposal](../typescript-angular-reference-adapter/proposal.md),
[design/mismatch register](../typescript-angular-reference-adapter/design.md),
[spec](../typescript-angular-reference-adapter/specs/typescript-angular-reference-adapter/spec.md)
and unchecked implementation tasks. The [closure ledger](../../../docs/quality/architecture-closure.md)
links all predecessor task ranges to accepted deliveries; the
[validation record](../../../docs/quality/gh-119-validation.md) records final local
validation. No non-Rust adapter or required-gate replacement is enabled. The
controller must observe this PR’s required CI before final merge/issue closure.
