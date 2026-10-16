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
