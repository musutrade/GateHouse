# Quality Coverage 分片并行方案

## 政策声明

本变更仅调整CI拓扑（采集在三个分片作业并行执行、由合并作业统一验证），**engineering-policy 语义不变**：
required gates、阈值、测量身份（series/identity）、覆盖率与 CRAP 规则、证据边界与 fail-closed 行为均未改变；
没有把任何必需检查放宽为 advisory 或跳过。冻结拓扑契约与所有权测试按新拓扑更新，负向测试覆盖合并边界。

## 背景

`quality-coverage` 是CI关键路径（7-11分钟）。它串行执行4个阶段：

```
legacy -> production -> risk -> matrix
```

实测各阶段耗时（两次main运行）:

| 阶段 | 最快运行 | 最慢运行 |
|------|---------|---------|
| legacy | 121s | 172s |
| production | 2s | 3s |
| risk | 218s | 344s |
| matrix | 60s | 89s |
| **关键路径合计** | **398s** | **605s** |

risk 阶段占比最大（55-57%），matrix 只依赖 git 历史，legacy/production 是快速基线。

## 方案：三路分片 + 合并验证

```
quality-fast    (legacy, production) ─┐
quality-risk    (risk)               ─┼─> quality-coverage (merge + verify + seal)
quality-matrix  (matrix)             ─┘
```

### 采集侧（`ci_quality.py collect --stages ...`）

- `--stages` 只运行指定阶段；拒绝重复阶段与未知阶段；`production` 必须与 `legacy` 同批。
- 分片收集完成后：任一阶段失败则该命令**非零退出**（分片作业失败，merge 被跳过）。
- 仅完整集合在收集末尾立即 `verify()`；分片由 merge 统一验证。

### 合并侧（`ci_quality.py merge`，独立可测函数）

在复制任何文件之前执行（全部 fail-closed）：

1. 恰好是预期的三个分片（fast/risk/matrix）；
2. 每个分片 `candidate.json` 有效；
3. 三个分片的 base/head/run_id 身份一致；
4. 每个分片恰好持有其预期阶段，且状态全部 success；
5. 每个名列的 artifact 摘要与分片自身字节一致（防篡改）；
6. artifact 路径不得越出分片目录；
7. **同一路径不得被两个分片声明**（默认拒绝，不存在静默覆盖）；
8. 三个分片记录的采集工具身份一致；
9. 至多一个 measurer pin，且所有记录它的分片一致。

随后才复制文件、写入合并 `candidate.json`（携带 measurer），merge 作业运行 `verify`、`seal` 并发布与原拓扑相同的 `quality-coverage-<run>-<attempt>` 产物。

### 证据来源（采集身份 vs 封装身份）

- **采集身份**：每个分片在收集后用 `ci_quality.py tool-identity` 记录本机 rustc/cargo/nextest/llvm-cov 版本与 `configuration_sha256`（`tool-identity.json`）；merge 校验三者一致后随合并产物保留。它回答“这些证据由什么工具采集”。
- **封装身份**：`ci_artifact.py seal` 在 merge 机器上重新记录工具版本写入 manifest。它回答“产物由什么工具封装”。
- 两者分工明确；manifest 的 `tools` 字段语义未改变。

### 显示名与下游兼容

merge 作业保留显示名 `Quality Coverage and Critical Paths`，因为：

- `certified_base()` 基线发现按该名称匹配定时/手动运行；
- `ci_reuse.py` 的 PR 等价复用按该名称校验检查项。

回归测试 `test_coverage_display_name_matches_baseline_and_reuse_consumers` 把
`ci.yml` 中的显示名、`ci_quality.COVERAGE_JOB` 与 `ci_reuse.REQUIRED_JOBS` 绑在一起，改名会直接失败。

### fail-closed 传播

```
分片 stage 失败
  -> collect() 非零退出（保留失败证据并上传）
  -> 分片作业 fail
  -> quality-coverage (merge) needs 失败 -> 跳过
  -> Required Quality Aggregate 失败
```

分片上传使用 `always()` 保留失败证据；全链路无 `continue-on-error`。

## 实测数据（hosted，PR #95 修复后运行）

| 作业 | 耗时 |
|------|------|
| Quality Fast | 3m19s |
| Quality Risk | **4m41s** ← 关键路径 |
| Quality Matrix | 1m54s |
| Merge+verify+seal | 25s |
| **quality 管道（分片开始→merge完成）** | **5m10s** |

对比旧单作业墙钟：main #137 10m48s、PR #94 9m47s、main #135（近期最快）7m07s。

**成本口径（修正）**：分片 runner 分钟合计 ≈9.9 分钟。相对近期最快单作业 7.1 分钟增加约 **+39%**；
相对最慢 10.8 分钟下降约 8%；与两次运行均值（约 9.3 分钟）基本持平。本变更用并发换墙钟，
是否接受取决于团队更看重墙钟还是 runner 分钟。

## 验证

- `run_core` 全套测试通过；新增合并负向测试（身份不一致、阶段缺失/错配、失败阶段、
  摘要篡改、重复路径、路径逃逸、工具身份不一致、measurer 冲突、输出目录已存在）。
- 本地端到端模拟：分片 → `merge` CLI → `verify` → `seal` → `verify`（rc 0）。
- 宿主测量：本 PR 自身运行 + 后续观察 1-2 次（按 Codex 建议，含一次定时/手动基线发现）。

## 回滚

单次 `git revert` 恢复单作业拓扑；分片与 merge 相互独立，无数据迁移。

## 2026-10-17 进一步拆分：Risk base/head 独立 runner

分片并行后，最新一次完整 PR 运行（run `39038404664`）的必需检查关键路径为：

| 阶段 | 墙钟 |
| --- | ---: |
| Classify | 7s |
| Quality Risk Stage | 412s |
| Quality Coverage merge/verify/seal | 25s |
| Required Quality Aggregate | 7s |
| **必需检查合计（创建→aggregate 完成）** | **457s** |

其中 Risk 的 `Collect risk stage` 为 377s，base/head 两条 coverage 命令在同一台 2 核
runner 上并行时分别耗时 334.6s / 331.4s；其余 setup、cache restore、工具安装与 artifact
上传约 35s。同一 runner 上两个 instrumented build 仍会争抢 CPU，因此风险阶段继续是
必需检查关键路径。

新拓扑不改变四阶段证据合同，只把 Risk 采集再拆成两个独立 worker：

```
quality-risk (matrix.side=base) ─┐
quality-fast                    ─┼─> quality-coverage (assemble risk -> merge -> verify -> seal)
quality-risk (matrix.side=head) ─┤
quality-matrix                  ─┘
```

- 两个 worker 分别执行 `ci_quality.py collect-risk-side --side base|head`，只采集自己一侧；
- 每个 worker 上传 `quality-artifacts-risk-<side>-<run>-<attempt>`；
- `quality-coverage` 先执行 `ci_quality.py assemble-risk`，验证两侧 candidate、side identity、
  artifact digest、tool identity 与 measurer pin，然后运行原有 risk compare；
- assemble 后再走原有 fast/risk/matrix 三 shard `merge`、`verify` 和 `seal`，最终证据
  schema、显示名和必需检查名称不变；
- 两侧共有的下载/安装 telemetry 若字节相同只保留一份；若因耗时等正常原因不同，则分别
  命名到 `shared/base/` 与 `shared/head/`，不会静默覆盖。

这会增加一台 runner 的分钟数，用并发换墙钟。首个 hosted run 必须先验证两侧工具身份一致、
artifact 完整、Risk 比较通过且无 OOM；在拿到该运行数据前，不把预期收益写成已实现收益。

## 2026-10-17 Cross-platform 测试分片

Risk 拆分后，最新观测的 macOS 测试会成为新的潜在关键路径：

| Job | 总墙钟 | 编译 | 运行测试 |
| --- | ---: | ---: | ---: |
| Test (macos-latest) | 313s | 73s | 214s / 710 tests |
| Test (windows-latest) | 266s | 78s | 132s / 562 tests |

因此 `test-cross-platform` 在原有 OS matrix 上增加 shard：macOS 2 个 worker，
Windows 3 个 worker。matrix 只生成 macOS `shard=1/2` 和 Windows `shard=1/3,2/3,3/3`：

```bash
cargo nextest run ... --no-fail-fast --partition hash:<shard>/<2|3>
```

约束与验证：

- 保留作业显示名 `Test (${{ matrix.os }})`，避免改变仓库 ruleset 中的必需检查名；
- matrix 中任一 shard 失败都会让 `test-cross-platform` job 失败，进而使
  `Required Quality Aggregate` 失败；
- `--no-fail-fast` 保持不变；
- Windows 当前约 78s 编译 + 132s 测试；按该 run 的实际测试耗时估算，2-way hash
  分区的测试耗时和约为 220s / 307s，3-way 为 122s / 179s / 227s，因此 Windows
  使用 3 个 worker 以降低最重分片；
- 本地对当前清单执行 `cargo nextest list --partition hash:1/2` 与 `hash:2/2`：
  361 + 363 = 724；`hash:1/3`、`hash:2/3`、`hash:3/3` 为 245 + 221 + 258 = 724，
  各集合交集均为 0，确认全部测试恰好覆盖一次；
- 编译仍会在每个 shard 各发生一次，因此这是用额外 runner 分钟换墙钟；最终仍以
  hosted 首次运行为准。

## Generic Quality Shadow 优化暂缓

`Generic Quality Shadow` 的投影步骤仍有约 117s（hosted 观测），但
`tools/quality/rust_reference.py` 是 `docs/quality/ci-plugin-test-handoff.json`
中受冻结兼容性 inventory 约束的 Core consumer。任何源码变更都会使
`run_core.py` 拒绝现有 GateProbes parity receipt；仅更新本地哈希也不满足
executed-inventory 约束。

因此本次没有保留对该文件的性能改动。若后续需要优化，必须先取得新的
GateProbes compatibility 执行证据，而不是在当前 CI PR 中顺带修改。

## 2026-10-17 Linux Test 分片

Linux `Test` job 最新为 221s：约 56s 编译 + 124s nextest 运行 + 约 40s setup/后置校验。
它与 Risk、Quality Fast 一起成为 Risk worker 拆分后的下一层候选关键路径。

`test` job 现在使用两个 shard：

- 其余 722 个测试按 `hash:<shard>/2` 分配；
- shard 1 额外运行两个会生成后续 artifact/收据的测试：
  - `configured_ci_acceptance_retains_parity_reuse_and_negative_evidence`
  - `html_export_combined_thousand_subjects_preserves_fail_and_rejects_tampering`
- acceptance 校验、HTML 上传、schema 校验、GateProbes onboarding 和所有 upload
  只在 shard 1 执行，避免同名 artifact 冲突；
- shard 2 只执行测试，任一 shard 失败都会使 matrix job 失败并阻塞
  `Required Quality Aggregate`。

本地 verifier：

- 分区测试：361 + 361；
- shard 1 独有测试：2；
- 合计 724，交集 0，并集 724；
- 保留作业显示名 `Test`，不改变 ruleset 必需检查名。

预计 Linux Test 从 221s 降至约 150–180s；代价是每个 shard 各编译一次。最终以
hosted 首次运行为准。

## 2026-10-17 Risk coverage 再分片与证据合并

Risk base/head 独立 worker 之后，每个 worker 仍要执行完整 instrumented test suite。
现在每个 side 再按 `hash:<shard>/2` 拆成两个 test partition，共 4 个采集 job：

```text
quality-risk (base, 1/2) ─┐
quality-risk (base, 2/2) ─┼─> quality-coverage
quality-risk (head, 1/2) ─┤     assemble-risk-shards
quality-risk (head, 2/2) ─┘     -> risk compare -> fast/risk/matrix merge
```

每个 shard 只执行对应测试集合，但仍完整执行 `source_measure.prepare`、
`cargo llvm-cov nextest`、LCOV/Cobertura 导出和 partial risk measure。partition
不完整导致的 `selected_failures` 不会让分片提前失败；最终 side 结果由合并后的
coverage/line hits 重新计算。

合并侧新增并测试了：

- LLVM coverage JSON 的 function/region 计数合并；
- LCOV 的 DA/FNDA/BRDA 计数合并与汇总重算；
- Cobertura 的 class/method line hits 合并与 rate 重算；
- partial risk report 的 raw region、line hits、instances 合并，并重新计算
  line/region coverage、CRAP、passed、selected failures 和 debt；
- 四个 shard 的 identity/tool identity/measurer pin/artifact digest 全部一致后才允许合并。

本地用当前提交的两个真实 Risk head partition 验证：

- shard 1 coverage 162.3s、measure 17.8s；
- shard 2 coverage 174.3s、measure 20.3s；
- 合并后 Risk report：2154 functions，selected failures 0，nonselected debt 361；
- 与已有完整 report 对比，所有 2154 个同名函数的 `lines`、`regions`、
  `crap_exact`、`passed` 一致；只有原始 region hit 计数因测试集合/提交不同而不同。

## 2026-10-17 CI debuginfo 压缩

CI 测试和 instrumented coverage 不需要完整 dev/test debuginfo。workflow 全局设置：

```yaml
CARGO_PROFILE_DEV_DEBUG: '0'
CARGO_PROFILE_TEST_DEBUG: '0'
```

`CARGO_TERM_COLOR`、`RUST_BACKTRACE` 和 panic 错误仍保留。该设置降低编译/链接时间
与测试二进制体积；coverage mapping 由 `-C instrument-coverage` 提供，不依赖完整
debuginfo，已用 `cargo llvm-cov` 实际导出验证。

## 未实施：legacy coverage 与 Risk head 复用

评估后没有把 `legacy` 与 Risk head 的 coverage run 合并，因为二者不是同一测量：

- `legacy` 在原始源码上执行 `cargo llvm-cov nextest --package harness-gate`；
- Risk 先在 staged snapshot 上执行 `source_measure.prepare`，注入 closure black-box
  边界，再执行 workspace coverage，并按 AST/instrumentation manifest 映射函数和区域。

直接复用会改变 legacy 的源码/工具/区域语义，属于证据身份变更而不是纯性能优化；
在未完成新的 measurement-series 与 acceptance 前不复用。

## 2026-10-18 按实际耗时重新平衡 native shard

hosted 成功 run `39092702264` 显示 hash partition 仍有明显偏斜：

| Job | 墙钟 | 测试执行 |
| --- | ---: | ---: |
| Test (macos-latest) | 144s | 73s |
| Test (macos-latest, shard 2) | 249s | 119s |
| Test (windows-latest) | 206s | 49s |
| Test (windows-latest, shard 2) | 198s | 71s |
| Test (windows-latest, shard 3) | 230s | 75s |

因此在不改变测试集合的前提下，把少量已知耗时测试从重分片移动到轻分片。每个
平台的全部 shard 都先从 partition 中排除这些测试，再由唯一指定 shard 单独执行：

- macOS：`frozen_policy_and_reference_acceptance_match_python`
  从 shard 2 移到 shard 1；
- Windows：`large_core_quality_json_is_redacted_and_finalized_without_truncation`
  移到 shard 1，`frozen_evidence_and_integrity_matrix_matches_python` 移到 shard 2。

本地用 `cargo nextest list` 验证：

- macOS 路由：361 + 362 + 1 = 724，交集 0；
- Windows 路由：245 + 221 + 256 + 1 + 1 = 724，交集 0。

这只改变测试在 shard 间的分配，不改变测试集合、失败传播或必需检查名。

## 2026-10-18 Risk shard 耗时平衡

hosted 成功 run `39094259393` 中 Risk shard 出现偏斜：

| Side / shard | 墙钟 | 测试执行 |
| --- | ---: | ---: |
| base 1/2 | 141s | 169.5s cumulative |
| base 2/2 | 183s | 242.1s cumulative |
| head 1/2 | 125s | 142.4s cumulative |
| head 2/2 | 181s | 265.9s cumulative |

将少量稳定重测试显式移出 hash partition，并由 shard 1 单独执行：

- base 排除 `html_export_combined_thousand_subjects_preserves_fail_and_rejects_tampering`、
  `cow_no_hit_borrows_and_mixed_replacements_match_frozen_g_bytes` 和
  `waiter_terminates_on_writer_failure_and_observed_output_limits`；
- head 排除 `frozen_policy_and_reference_acceptance_match_python`。

移动量按 wall-clock 而不是测试时长总和估计：额外命令只执行少量可并行的重测试，
避免把 base/head shard 1 变成新的串行长尾。

`collect-risk-shard` 新增 `--exclude-tests` / `--extra-tests`。每个分片先以
`cargo llvm-cov nextest --no-report` 执行 partition，再以同样方式执行额外测试，
最后只生成一次 JSON/LCOV/Cobertura report，确保所有 profile 都进入同一份证据。

本地 `cargo nextest list` 验证两种 side 的 partition + extra 路由都恰好覆盖
724 个测试且交集为 0；两次 `--no-report` 后生成的 coverage union 为两次单独
覆盖的并集。
