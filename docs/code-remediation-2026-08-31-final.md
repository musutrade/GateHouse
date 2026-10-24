# Harness-Gate 最终代码审计与整改基线

> 本文合并并复核以下三份审计材料：
> `code-review-2026-08-31-independent.md`、`code-review-2026-08-31.md`、
> `code-audit-2026-08-31.md`。它是去重、重新定级并带验收条件的最终整改版本，
> 不是三份报告的简单拼接。三份源报告已在仓库外归档，仓库内以本文和
> ADR-0034 作为后续整改依据。

> **基线说明（2026-09-01 更新）：** 本文记录的是分支
> `docs/adapter-documentation@97da753` 审计时点的整改基线。随后 PR #60
> 已实现 R-01 至 R-05；当前未完成项和最新状态以
> [`docs/review-followups-2026-08-31.md`](review-followups-2026-08-31.md) 为准。
> 因此下文保留的 P0 验收条件是历史审计记录，不表示这些五项在当前 `main`
> 上仍未修复。

## 1. 审计边界与结论

| 项目 | 结论 |
| --- | --- |
| 审计对象 | `tools/harness-gate`、安装脚本、发布/CI 工作流、配置和证据链路 |
| 代码基线 | 分支 `docs/adapter-documentation`，提交 `97da753` |
| 验证方式 | 静态审查、现有测试/质量门禁、针对性动态复现 |
| Critical | 0 |
| P0 release blocker | 5 |
| P1 next release | 6 |
| P2/P3 | 7，详见第 4 节 |

**该审计基线的最终判断：**项目的常规工程质量较好，但该基线版本尚不能作为不可信仓库或共享 CI runner 上的最终 fail-closed 安全门禁。阻断原因不是测试不足，而是五项发布阻断问题尚未解决，覆盖四类边界：输入快照一致性、破坏性清理的所有权、执行证据完整性、发布物完整性。

三份报告之间的主要分歧是 adapter。复核结果如下：

- “capability allowlist 没有 OS 网络/文件沙箱”是**已在 README 和配置文档中声明的产品限制**，不是隐藏实现漏洞；当前 capability 是协议级 allowlist，不等同于操作系统沙箱。
- “签名只覆盖 declaration，不覆盖 request body”是真实的**请求完整性/信任边界缺口**。若请求文件由不可信来源生成，`args`、`input`、`environment`、`capabilities`、`timeout` 等可被篡改。
- “进程隔离只建立进程组”是真实的**进程生命周期保证不足**：它不是 namespace、网络、文件或资源隔离；Unix 后代进程自建新 session 后可能脱离 kill group，Windows 也只有现有 `taskkill /T` 路径。文档必须降级措辞，或实现并验证更强隔离。

严重度假设 Harness-Gate 可能读取攻击者可控 checkout、写入工作区/报告目录、访问容器运行时或向外部 URL 发请求。若部署环境完全受信且 runner 强隔离，本地攻击面的等级可下降，但 staged、证据和发布问题仍需修复。

### 三份报告的合并裁决

| 原报告结论 | 最终处理 |
| --- | --- |
| staged architecture audit 绕过 | 确认，并扩大到 hook 中所有未显式读取 index 的 external step |
| SBOM 已具备完整签名/证明 | 更正：二进制具备，SBOM 因 glob 错误未纳入 checksum、签名和 provenance |
| lease 管理整体安全 | 保留其原子创建/PID identity 优点，但 cleanup 所有权验证存在可复现高风险缺口 |
| adapter request 未完整签名 | 确认事实；从“严重验签绕过”校准为受信 request 边界下的 Medium 协议风险 |
| capability 没有 OS sandbox | 不作为隐藏漏洞；README 已声明该限制，但 ADR/OpenSpec 和部分措辞仍需纠正 |
| `setsid` 等于完整进程隔离 | 否定；保留“逃逸后代无法可靠终止、持有 pipe 可阻塞 reader”的生命周期问题 |
| 非法 adapter env key 会 panic | 删除误报；当前 Rust 对 NUL 在 spawn 返回 `InvalidInput`，`=` 在 Linux 可接受 |
| 文档体量、100ms 轮询、O(n²) 调度为 High | 降为 P2/P3 可用性或性能债，当前没有足够基准支持 High 定级 |
| 工作区不干净是产品安全缺陷 | 降为仓库卫生事项，不计入产品漏洞数量 |

## 2. P0：发布前必须修复

### R-01：hook 只有部分节点使用 staged snapshot

**证据：**[`verify/scheduler.rs`](../tools/harness-gate/src/verify/scheduler.rs) 仅将 `staged` 传给 secret scan；`run_architecture_audit` 和 `run_external_step` 都使用发现时的 project root。README 却将 `hook` 定义为 staged snapshot。以 [`rust-api.flow.toml`](https://github.com/musutrade/Harness-Gate/blob/da9a1082f698f17418b9d20ed077bd1aca2adfc8/tools/harness-gate/presets/rust-api.flow.toml) 为例，hook profile 中的 fmt/clippy 也会读取 working tree；只有 generic preset 的 `git diff --cached` 因命令自身显式读取 index 而例外。

**影响：**index 中实际提交的架构违规、格式或编译问题可被 working tree 的干净版本掩盖；未暂存内容也可能造成与提交无关的失败。

**复现：**index 暂存包含 `FORBIDDEN_MARKER` 的 `src/bad.rs`，working tree 改为 clean，不重新暂存；`harness-gate hook` 返回 0，架构违规数为 0。

**整改：**建立 invocation 级输入根，在临时目录物化完整 index 快照，让 scope、secret、architecture、配置和普通 external step 使用同一输入。确实需要直接访问 Git index 的步骤必须显式声明并获得原仓库/index 上下文，不能由 cwd 偶然决定语义。

**验收：**architecture、fmt、clippy 至少覆盖两组集成测试：staged 有问题而 working tree 无问题必须失败；working tree 有问题而 staged 无问题不能影响 hook 结果。报告中记录 snapshot tree/index digest，hook 使用的配置也能追溯到同一快照。

### R-02：lease cleanup 可删除未证明归属的容器

**证据：**[`service/lease.rs`](../tools/harness-gate/src/service/lease.rs) 只校验 JSON marker/schema/resource_id；`validate_record` 不检查文件名、项目 identity 或容器 label；当时的 `runtime.rs`（现已[退役](runtime-lifecycle-v1.md#legacy-cli-retirement)）最终直接执行 `rm --force <name>`。

**影响：**伪造过期 lease 可诱导具有 Docker/Podman 权限的 cleanup 删除其他 `harness-gate-*` 容器。

**复现：**fake runtime 记录到 `rm --force harness-gate-victim`。

**整改：**强制 lease 文件名等于 `resource_key(resource_id)`；绑定 canonical project identity、resource kind、invocation；删除前执行 runtime inspect，核对 owner/project/resource/invocation 全部 labels；任一 label 缺失、不一致、inspect 失败均不回收。

**验收：**伪造文件名、跨项目 lease、同名前缀但 label 不匹配、运行中容器和 malformed record 均不得触发 `rm`；必须有 fake Docker/Podman 契约测试。

### R-03：声明的步骤日志缺失仍可 PASS

**证据：**[`verify/report.rs`](../tools/harness-gate/src/verify/report.rs) 只校验路径形状；manifest 只枚举现存文件，验证也只验证 manifest 已列出的条目。

**影响：**报告可引用不存在的日志并标记 `evidence_complete=true`，审计者无法证明成功步骤有对应证据。

**复现：**步骤删除自己的日志后 verify 仍返回 0；报告保留日志路径，manifest 不包含该路径。

**整改：**生成结果时逐项确认日志属于当前 invocation、是普通文件且存在；把报告声明集合与 manifest、磁盘集合做闭集比较；artifact 绑定 invocation/step/type/size/digest。

**验收：**unlink-open-log、缺失 log、symlink log、旧 invocation 文件、额外未声明 artifact 均必须 fail closed；manifest 与结果集合必须双向相等。

### R-04：固定报告写入跟随 symlink

**证据：**[`utils/fs.rs`](../tools/harness-gate/src/utils/fs.rs) 使用 `fs::write`；cleanup、scope、secret、audit 等命令写入可预测固定文件名。

**影响：**攻击者可通过报告目标 symlink 覆盖进程权限范围内的仓库外文件。

**复现：**将 `cleanup.json` symlink 指向外部文件，执行 cleanup 后外部内容被改写。

**整改：**统一使用同目录 `create_new` 临时文件、写入/`fsync`、原子 rename；拒绝 symlink/目录目标，canonicalize 父目录并确认位于受信报告根目录。

**验收：**所有 standalone writers 对预置 symlink、目录、父目录 symlink 和并发替换均 fail closed；正常输出保持原子可读。

### R-05：发布 SBOM 实际不在完整性保护范围

**证据：**[`release.yml`](../.github/workflows/release.yml) 生成 `harness-gate.sbom.cdx.json`，但 checksum、cosign 和 attestation 使用 `harness-gate-*`，不会匹配点号文件名；最终 `dist/*` 仍上传它。README 要求验证 SBOM 签名，但工作流未生成对应文件。

**影响：**SBOM 可脱离二进制签名和 checksum 被替换，消费者无法证明依赖清单与发布构建绑定。

**整改：**显式把 SBOM 加入 SHA256SUMS、cosign sign/verify 和 attestation subject；发布前断言所有签名、证书、摘要和证明存在；release fixture 测试不得只依赖 glob。

**验收：**在临时 `dist` 中运行发布元数据脚本后，二进制、SBOM、manifest 的 checksum/signature/provenance 均存在且逐一可验证；任一漏项阻断 release。

## 3. P1：下一发布周期完成

### R-06：adapter 请求主体未签名，且缺少重放/绑定保护

**证据：**[`process/adapter.rs`](../tools/harness-gate/src/process/adapter.rs) 的 `signing_payload` 只包含 adapter name/version/executable/source_digest；请求的 `args`、`input`、`environment`、`capabilities`、`invocation_id`、`timeout_ms` 和未使用的 `config_digest` 不在签名内。

**风险边界：**adapter CLI 是显式 opt-in，当前文档要求调用方提供 request、trusted key 和 allowlist，并明确 capability 不是 OS sandbox。因此这里是“不可信 request 文件”场景下的完整性缺口，不应描述为所有内置步骤均可被任意执行。

**整改选项：**优先对规范化的完整 request payload 签名，并把 nonce、issued_at/expiry、invocation ID、step ID、config digest 纳入签名和重放检查。环境变量名增加跨平台预校验并返回结构化协议错误；当前 Rust 对 NUL 会在 spawn 返回 `InvalidInput`，因此后者是健壮性改进，不是已确认 panic 漏洞。

**验收：**篡改任一 request 字段均验签失败；同一 request 跨 invocation 或过期重放失败；非法环境 key 在 spawn 前返回稳定的结构化协议错误。

### R-07：adapter “进程隔离/权限”宣传超过实际执行保证

**证据：**[`process/command.rs`](../tools/harness-gate/src/process/command.rs) Unix 只调用 `setsid()`，终止依赖进程组；[`adapter.rs`](../tools/harness-gate/src/process/adapter.rs) 只做 capability 字符串集合比对。没有 seccomp、landlock、namespace、setrlimit 或等效 OS 强制层。

**处理结论：**capability allowlist 本身不是 bug，因为 README 已写明“protocol-level boundary, not an operating-system network sandbox”；但“isolated from host”“only declared network/resource permissions”“整个进程树”属于过强或需限定的平台承诺。

**整改：**短期把 README、中文文档、ADR 和 OpenSpec 统一为“独立进程、清空继承环境、协议级 allowlist、有限的终止尝试”，明确不提供 OS 级网络/文件/资源沙箱。reader 必须有独立 deadline，不能在 child 退出后无界 `join` 持有 pipe 的逃逸后代。中长期如需保留安全卖点，按平台实现可验证 sandbox（Linux namespace/seccomp/landlock、Windows Job Object、macOS sandbox 等），并在 capability 到 OS policy 的映射完成前拒绝声明。

**验收：**文档与实现一致；每个平台都有后代进程、逃逸 session、超时、取消和资源上限测试；未实现的 capability 不得声称已执行强制。

### R-08：安装脚本下载并安装未经验证的二进制

**证据：**[`install.sh`](../install.sh) 使用 grep/sed 解析 API，curl 无 `--fail`，下载后无 SHA256/Sigstore 校验，直接写固定输出名；[`PROJECT_DELIVERY.md`](../PROJECT_DELIVERY.md) 推荐 `raw/main | bash`。

**整改：**固定版本/不可变 tag；下载到 `mktemp -d`；用 `curl --fail --show-error --location`；校验 SHA256、cosign issuer/identity 后再安装；拒绝预置 symlink；文档移除 mutable remote script 推荐。

**验收：**HTTP 404/HTML 响应、摘要不符、签名不符、证书 identity 不符、输出 symlink 和安装目录权限异常均不安装；安装成功后打印已验证版本和摘要。

### R-09：standalone audit 与 parse-logs 可能泄露敏感内容

**证据：**[`audit/report.rs`](../tools/harness-gate/src/audit/report.rs) 原样输出 `Violation.content`；[`audit/log_parser.rs`](../tools/harness-gate/src/audit/log_parser.rs) 直接写提取结果。verify 的 redaction 只覆盖另一条报告路径。

**整改：**统一复用 redaction pipeline；默认仅输出文件、行号、规则和脱敏摘要；JSON、Markdown、stdout、parse-logs 输出和错误上下文分别做不泄露测试。SBOM 生成器也应清理依赖 source URL 中可能存在的凭据。

**验收：**fixture 中的 token、Bearer、数据库 URL、Authorization header、private key 不得出现在任一 audit/parse-logs 输出或 CI stdout。

### R-10：release 触发和质量依赖边界不足

**证据：**[`release.yml`](../.github/workflows/release.yml) 任意 `v*` tag 即发布，publish 只依赖 build，不依赖 test/fmt/clippy/audit/quality-required；Actions 使用可变 tag。CI 中多处 `cargo install --force` 抵消缓存，coverage 同时使用 tarpaulin 和 llvm-cov。

**整改：**release 增加受保护 tag/environment 和完整质量依赖；Actions pin 到 commit SHA；统一工具安装 action，移除不必要 `--force`；coverage 工具择一；build/工具缓存按 lockfile 和平台分 key。

**验收：**未经必需检查或使用非受保护 tag 无法发布；action digest 变更有审查；缓存命中率和发布前检查结果进入构建证据。

### R-11：stdout/stderr、日志和证据资源预算不足

[`process/capture.rs`](../tools/harness-gate/src/process/capture.rs)、[`process/adapter.rs`](../tools/harness-gate/src/process/adapter.rs) 使用无界 `read_to_end`；[`process/task.rs`](../tools/harness-gate/src/process/task.rs) 日志可无限增长；redaction 在 [`verify/report.rs`](../tools/harness-gate/src/verify/report.rs) 读取后才检查大小。逃逸后代若继承 adapter stdout/stderr，即使直接 child 已退出，也可能让 reader `join` 永久等待。改为有界流式读取、单步/单 invocation quota、截断状态、磁盘预算和 reader deadline。

## 4. P2：质量、性能和可维护性改进

### R-12：XML 结果解析接受多个根元素

[`verify/parser.rs`](../tools/harness-gate/src/verify/parser.rs) 只维护标签栈；`<testcase/><testcase/>` 可满足 minimum 并 PASS。要求单根 XML，并按 JUnit/TRX 允许根元素和结构校验。

### R-13：Webhook SSRF 与报告外泄边界不足（条件性）

[`config/validation/mod.rs`](../tools/harness-gate/src/config/validation/mod.rs) 只验证 HTTP(S)，[`verify/report.rs`](../tools/harness-gate/src/verify/report.rs) 可访问 loopback、RFC1918、link-local 或 DNS rebinding 目标。若配置可被不可信仓库控制，存在 SSRF/外泄风险。默认 deny private/link-local，采用显式 host allowlist，并在连接后校验解析地址。

### R-14：非 Linux 平台的 lease 长步骤可能过 TTL 被误回收

[`service/lease.rs`](../tools/harness-gate/src/service/lease.rs) 使用 15 分钟 TTL；macOS、Windows 等非 Linux 平台无法读取 Linux `/proc` start identity，就绪后的长步骤又没有后台 heartbeat。为服务整个存续期增加 heartbeat，并按平台使用可靠进程句柄或启动时间验证。

### R-15：运行期错误码和配置诊断依赖字符串

运行期 `SCHEDULER_FAILURE`、`RESULT_PARSE_FAILURE` 等散落为字符串，`retry_class` 通过字符串比较。配置诊断在 [`config/validation/mod.rs`](../tools/harness-gate/src/config/validation/mod.rs) 通过 `error.contains(...)` 推断路径、ID 和修复提示。引入 `FailureCode`/结构化诊断对象，禁止由展示文本反推机器契约。

### R-16：调度和轮询存在低风险性能开销

[`process/task.rs`](../tools/harness-gate/src/process/task.rs) 固定 100ms 轮询，快命令会额外等待；scheduler 每轮全量扫描、线性定位，规模增大后有 O(n²) 成分。使用短起始值的指数退避或平台 wait primitive；调度器使用节点索引和入度计数。`secrets/matcher.rs` 的 `HashSet<u8>` 热路径可改位图。

### R-17：配置与诊断可维护性

- `config/validation/mod.rs` 对每个 service/parser/check/rule clone 整个 `FlowConfig`，形成 O(n²) clone；改为上下文参数或结构化子校验。
- `kind`/`gate_type` 注释称为 closed vocabulary，却使用 `Option<String>`；改 enum 以在反序列化期拒绝非法值。
- `REPORT_DIR` 无统一前缀，建议弃用并告警，统一 `HARNESS_GATE_*`。
- `verify/scheduler.rs` 的 `catch_unwind(AssertUnwindSafe(...))` 与 `release-small` 的 `panic = "abort"` 语义不同；补 lock-poisoning 测试并记录发布 profile 差异。

### R-18：仓库卫生、社区和发布元数据

- `.gitignore` 补 `__pycache__/`、`*.pyc`，清理或归档未跟踪脚本与一次性文档；
- 质量脚本本身纳入 lint/test，避免“判定者不受门禁约束”；
- 增加 `SECURITY.md`、`rust-version`（MSRV）、Issue/PR 模板和行为准则；
- README 修正 `secrets` 帮助文本：实际扫描文件内容，不是文件名；补充“快速前置扫描，不替代 gitleaks/trufflehog”的定位；
- 插值仅在 TOML basic string 生效、无配置 include/继承、Webhook 缺少鉴权 header/retry 作为产品能力排期，不与安全阻断项混合。

## 5. 不纳入缺陷的核实项

以下项目经复核不应作为当前整改缺陷：

1. `default_step_timeout() = 0` 和空 cwd 是内置 gate 的哨兵值；外部步骤验证会强制有效 timeout/cwd 语义。
2. `ExecutionConfig.parallel` 与 `max_parallel` 语义不同：前者用于兼容路径串行降级，后者是并发上限。
3. `Command` argv 调用本身未发现 shell 拼接注入；这不抵消仓库配置允许执行任意声明程序这一信任模型。
4. capability allowlist 不提供 OS sandbox 已在文档中明确；整改重点是文档/协议一致性和可选的未来强制沙箱，而不是把协议检查误称为网络隔离。
5. “非法 adapter 环境变量键会 panic”未复现：NUL 在 spawn 返回 `InvalidInput`，包含 `=` 的键在当前 Linux/Rust 可执行。保留跨平台预校验，但不计安全漏洞。

## 6. 已通过的验证

| 检查 | 结果 |
| --- | --- |
| `cargo test --locked` | 203 单元 + 17 CLI + 11 集成，全部通过 |
| `cargo fmt --check` | 通过 |
| Clippy all targets/features，`-D warnings` | 通过 |
| `cargo audit` | 203 dependencies、1226 advisories，无已知漏洞 |
| docs consistency | preset、迁移、链接、Schema 同步全部通过；使用可写临时 `CARGO_TARGET_DIR` 排除环境误报 |
| 动态复现 | staged 绕过、缺失日志仍 PASS、多 XML 根 PASS、lease 删除、symlink 覆盖、audit 凭据泄露均已复现 |

## 7. 最终执行顺序

### Release blocker（合并前）

1. R-01 staged 快照统一。
2. R-02 lease/container ownership fail-closed。
3. R-03 evidence/manifest 闭集。
4. R-04 symlink-safe report writer。
5. R-05 SBOM 完整签名、checksum、attestation。

### Next release

1. R-06 adapter 完整请求签名、nonce/expiry/invocation 绑定和环境键校验。
2. R-07 降级并统一 adapter 隔离文案，或实现可验证 OS sandbox。
3. R-08 安装脚本验证链。
4. R-09 全链路脱敏。
5. R-10 release 质量依赖、action pin 和缓存治理。
6. R-11 输出、日志、磁盘和 reader deadline 资源预算。

### 后续迭代

R-12 至 R-18 按影响和成本排期；性能优化不得先于资源上限和安全边界修复。

## 8. Definition of Done

本轮整改只有同时满足以下条件才可宣称完成：

- P0 五项均有代码修复、失败测试和成功路径测试；
- 所有发布文档、README、ADR、OpenSpec 与实际 adapter 保证一致；
- 运行 `cargo test --locked`、fmt、Clippy、audit、docs consistency 和新增安全回归测试全部通过；
- 发布 fixture 能离线验证二进制、SBOM、checksum、签名和 provenance；
- 报告中不再出现未经实现支持的“OS 沙箱/完整进程树隔离”表述；
- 变更说明列出未实现的条件性控制（私网 webhook、非 Linux heartbeat、OS sandbox），避免把残余风险误报为已解决。

本文件仅保存审计与整改结论，不修改现有源码；工作区原有修改应由项目维护者按上述顺序处理。
