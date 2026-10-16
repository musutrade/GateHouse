# Quality Coverage 分片并行方案

## 背景

`quality-coverage` 是CI的关键路径（7-11分钟）。它串行执行4个阶段：

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

四点改动：

1. **`tools/quality/ci_quality.py`**
   - `collect()` 新增 `--stages` 参数，只运行指定阶段；
   - 仅当阶段集合完整时执行全量 `verify()`；分片由 merge 作业统一验证。

2. **`.github/workflows/ci.yml`**
   - 新增 `quality-fast`、`quality-risk`、`quality-matrix` 三个分片作业（均 `needs: [classify]`，彼此并行）；
   - 原 `quality-coverage` 变为 merge 作业：`needs: [classify, quality-fast, quality-risk, quality-matrix]`，下载三个分片产物、合并 `candidate.json`（校验 base/head/run_id 一致、阶段无重叠）、运行 `verify`、`seal` 并上传最终证据 `quality-coverage-<run>-<attempt>`；
   - 下游 `quality-generic-shadow` 与 `quality-required` 的依赖不变。

3. **测试与契约**
   - `ci-topology.json` 冻结契约登记3个新作业与新的 merge 语义；
   - `test_ci_topology`、`test_ci_state`、`test_ci_quality`、`test_ci_acceptance`、`test_ci_tool_setup` 更新为分片所有权断言（采集只允许发生在3个分片作业；merge 只验证/封装；shadow 只消费）。

4. **保留的约束**
   - `cache-target: 'false'`（测量作业不恢复编译缓存）；
   - 分片与 merge 都不使用 `continue-on-error`；
   - 分片上传使用 `always()` 保留失败证据；
   - fail-closed：任一分片失败 → merge 被跳过 → `Required Quality Aggregate` 失败。

## 预期收益与代价

- **墙钟时间**：关键路径从 `legacy+risk+matrix`（约6.6-10分钟）降为 `setup + risk（约3.6-5.7分钟） + merge（约1分钟）`，预计节省 **2-3.5分钟**。
- **Runner成本**：每个分片独立编译，runner分钟数增加（3份 setup + 编译）；这是用成本换墙钟时间。
- **验证方式**：本PR自身的CI运行（push触发全量quality）即为第一组测量数据。

## 回滚

单提交回滚（`git revert`）即可恢复单作业拓扑；分片作业与 merge 相互独立，无数据迁移。
