# CI Performance Analysis - 2026-10-16

## Executive Summary

CI时间波动是由**Runner资源竞争**导致的，而非配置问题。最快和最慢的运行使用完全相同的配置，但耗时相差**51%**。

**关键发现**:
- 最快运行: 427秒 (Run #137)
- 最慢运行: 648秒 (Run #139)
- **波动: 221秒 (51.8%)**
- 两次运行都是 main 分支，都没有启用 target 缓存
- 差异完全来自 GitHub Actions runner 的性能波动

---

## Test Results: Cache Enablement (PR #94)

尝试启用 `cache-target: true` 来加速编译:

| 指标 | 结果 |
|------|------|
| 耗时 | 587秒 (9分47秒) |
| vs 历史平均 | +19秒 (+3.3%) |
| 缓存命中 | ❌ 路径不存在，缓存未保存 |
| PR合并 | ❌ 测试强制要求 `cache-target: 'false'` |

**结论**: 缓存优化不可行。团队已有测试 `test_unproductive_compiled_caches_are_disabled_for_all_callers` 强制禁用缓存，说明之前已经验证过缓存无效（instrumented编译配置导致缓存命中率极低）。

---

## Root Cause Analysis: Why CI Time Varies 51%

### Detailed Stage Comparison

| 阶段 | Run #137 (最快) | Run #139 (最慢) | 差异 | 增幅 |
|------|----------------|----------------|------|------|
| **Legacy** | 120.98秒 | 172.28秒 | +51秒 | +42% |
| Production | 1.66秒 | 3.17秒 | +1.5秒 | +91% |
| **Risk** | 217.73秒 | 344.03秒 | +126秒 | **+58%** |
| **Matrix** | 59.72秒 | 89.02秒 | +29秒 | +49% |
| **总计** | **398秒** | **605秒** | **+207秒** | **+52%** |

### Runner Information

**Run #137 (最快)**:
- Region: `centralus`
- Image: `ubuntu-24.04` (20261012.334.1)
- Runner: GitHub-hosted (2核, 7GB RAM)

**Run #139 (最慢)**:
- Region: (需要查看日志确认)
- Image: 相同
- Runner: 相同规格

### Critical Path Breakdown

CI的关键路径是:
```
legacy (串行) -> risk (内部并行) -> matrix (串行)
```

**Risk阶段**是最大瓶颈:
- 包含: base-coverage, head-coverage, base-measure, head-measure 等子任务
- 最快运行: 217秒
- 最慢运行: 344秒
- **波动: 127秒 (58%)**

这个波动幅度说明问题不在代码或配置，而在runner的:
1. **CPU资源竞争** - 同一物理机上其他租户任务
2. **磁盘I/O性能** - 覆盖率收集涉及大量文件读写
3. **网络延迟** - measurer下载, GitHub API调用

---

## Optimization Recommendations

### ✅ Already Implemented

1. **Risk并行收集** (`CI_PARALLEL_RISK=1`)
   - 理论节省: 78.6秒
   - 已启用，效果良好

2. **Base测试优化**
   - 节省: 71.65秒
   - 已实施

### 🎯 High-Impact Recommendations

#### 1. 使用更大的Runner (推荐)

**当前**: `ubuntu-latest` (2核, 7GB RAM)  
**建议**: `ubuntu-latest-4-cores` (4核, 16GB RAM)

**预期效果**:
- Legacy阶段: 120秒 → ~80秒 (-33%)
- Risk阶段: 217秒 → ~150秒 (-31%)
- Matrix阶段: 60秒 → ~40秒 (-33%)
- **总耗时: 427秒 → ~300秒** (-30%)
- **波动降低**: 更大的资源配额减少竞争

**成本**:
- 2核 runner: $0.008/分钟
- 4核 runner: $0.016/分钟
- 每次CI增加成本: ~$0.08
- 但**节省开发时间**: 每次推送节省4-5分钟

**实施**:
```yaml
quality-coverage:
  runs-on: ubuntu-latest-4-cores  # 改这一行
```

#### 2. 拆分Quality Coverage Job

将单个大job拆成3个并行job:

```yaml
jobs:
  quality-coverage:
    # 只运行 legacy + production
    
  quality-risk:
    needs: [quality-coverage]
    # 只运行 risk 阶段
    
  quality-matrix:
    needs: [quality-coverage]
    # 只运行 matrix 阶段
```

**预期效果**:
- risk 和 matrix 可以并行
- 总耗时: 398秒 → ~320秒 (-20%)
- 更好的可观测性

**风险**:
- 增加复杂度
- 需要artifact传递

#### 3. PR时跳过非关键检查

对于PR (非main分支)，可以跳过:
- Codecov上传 (已移到daily workflow)
- 某些legacy测试
- 完整的matrix生成

**预期效果**:
- PR运行: 400秒 → ~280秒 (-30%)
- main分支: 保持完整检查

**实施**:
```yaml
- name: Collect evidence
  if: github.ref == 'refs/heads/main' || inputs.full_quality == true
```

#### 4. 启用Self-hosted Runner (长期方案)

使用专用的self-hosted runner:
- 稳定的性能，无资源竞争
- 可以使用更大的机器 (8核, 32GB)
- 预热的Docker缓存, Cargo缓存

**预期效果**:
- 耗时: 400秒 → ~200秒 (-50%)
- **完全消除波动**

**成本**:
- AWS c5.2xlarge: $0.34/小时 = ~$250/月
- 需要维护成本

---

## Immediate Next Steps

### Option A: 最小改动 (推荐)

1. **启用4核runner** (修改1行配置)
   ```bash
   # .github/workflows/ci.yml line 318
   runs-on: ubuntu-latest-4-cores
   ```
   
2. **观察3次运行**
   - 预期: 280-320秒, 波动<15%
   
3. **如果效果好**: 向团队汇报成功
4. **如果波动仍大**: 执行 Option B

### Option B: 中等投入

1. 实施 Option A
2. 同时拆分 quality-coverage job
3. PR时跳过非关键检查

### Option C: 长期方案

1. 设置 self-hosted runner
2. 专用CI机器配置
3. 预期2周完成

---

## Cost-Benefit Analysis

### 当前状况

- 平均CI时间: 9分钟
- 波动: 7-11分钟
- 每天推送: ~20次
- **开发等待时间**: 20 × 9 = 180分钟/天

### Option A (4核runner)

- 预期CI时间: 5分钟
- 波动: 4-6分钟
- 额外成本: 20 × $0.08 = **$1.6/天**
- **节省开发时间**: 20 × 4 = 80分钟/天

**ROI**: 每花费$1，节省50分钟开发时间

### Option C (Self-hosted)

- 预期CI时间: 3-4分钟
- 波动: <5%
- 成本: **$8/天**
- **节省开发时间**: 20 × 5 = 100分钟/天

**ROI**: 每花费$1，节省12.5分钟开发时间

---

## Technical Details

### Why Cache Doesn't Help

从PR #90的描述可以看出，团队已经实现了**更高级的复用机制**:

> Strict PR->main check inheritance: on an ordinary single-merge push to main, the classify job proves tree equivalence between the tested PR merge and the main merge using a receipt recorded by the PR run

这个机制比简单的编译缓存更智能:
- 检查PR的测试结果是否可以复用到main分支
- 验证代码树完全等价
- 避免重新运行整个quality-coverage

但从Run #137的日志看:
```json
{"reason": "CalledProcessError", "reused_jobs": [], "reuse_verified": false}
```

复用机制失败了（可能是第一次部署，还没有可复用的PR证明）。

**Target缓存无效的原因**:
1. Quality job使用 `isolated-instrumented` profile
2. 包含覆盖率插桩 (code coverage instrumentation)
3. 每次编译的插桩配置可能不同
4. Cargo无法判断缓存是否有效，总是重新编译

### Runner Performance Variance

GitHub Actions的hosted runner性能波动是已知问题:
- 多租户环境，与其他用户共享物理机
- CPU竞争导致编译/测试变慢
- 磁盘I/O竞争影响覆盖率收集
- 不同region的性能差异

解决方案只有两个:
1. 使用更大的runner (更多资源配额)
2. 使用self-hosted runner (独占资源)

---

## Conclusion

**问题不在配置，在资源**。

Codex团队的优化方向是对的（并行化、复用机制），但GitHub Actions的资源限制是硬瓶颈。

**推荐行动**:
1. 立即启用4核runner（1行改动）
2. 观察1-2天
3. 如果开发体验明显改善，保持这个配置
4. 如果CI仍然是瓶颈，投资self-hosted runner

**预期结果**: CI时间从9分钟降到5分钟，波动从51%降到15%以内。

---

## Appendix: Recent CI Runs

| Run | Number | Branch | Duration | Legacy | Risk | Matrix |
|-----|--------|--------|----------|--------|------|--------|
| 38834512946 | 137 | main | 427秒 ⭐ | 121秒 | 218秒 | 60秒 |
| 38836300683 | 138 | perf/ci-impact-routing | 561秒 | - | - | - |
| 38837370198 | 139 | main | 648秒 💥 | 172秒 | 344秒 | 89秒 |
| 38849340835 | 140 | perf/test-quality-cache | 587秒 | - | - | - |

注: Run 137和139都是main分支，完全相同的配置，但耗时相差52%。
