# CI 并行 Risk 实施状态

2026-10-15：实验功能默认关闭，第一组本地完整 Risk 对照已完成，暂不默认启用。

已修正默认开关、清理后的环境传递、独立构建目录、命令记录同步，以及失败后等待在途任务并跳过比较的行为。串行和并行共用逐提交收集及完整性检查。

新增测试覆盖：实际路由、双任务交叠、传至实际命令的环境隔离、串行构建目录复用、失败后等待及失败落盘、陈旧覆盖率拒绝、真实并发子进程日志与记录。测试位于 `tools/quality/tests/test_parallel_risk.py`，由现有 `run_core.py` 发现。

此前 R2 交接声称 32 个测试通过，但独立同进程复测为 36 个测试、2 个失败、10 个错误，不能作为通过证据。原日志保留在本地任务目录 `~/.local/state/herdr-issue-flow/musutrade-GateHouse/ci-parallel-risk-20261015/review-r2-tests.log`。后续修复替换了互相污染的线程内 mock，并测试真实收集器行为。

针对性验证命令：

```bash
cd tools/quality/tests
python3 -B -m unittest test_parallel_risk test_ci_quality -v
```

完整 CI 收集入口示例（从仓库根目录运行，输出目录必须尚不存在）：

```bash
CI_PARALLEL_RISK=0 python3 tools/quality/ci_quality.py collect \
  --base-sha "$BASE_SHA" --head-sha "$HEAD_SHA" \
  --run-id serial-experiment --output /tmp/quality-serial-experiment
CI_PARALLEL_RISK=1 python3 tools/quality/ci_quality.py collect \
  --base-sha "$BASE_SHA" --head-sha "$HEAD_SHA" \
  --run-id parallel-experiment --output /tmp/quality-parallel-experiment
```

`BASE_SHA`、`HEAD_SHA` 必须为已存在的完整提交 SHA，head 必须对应当前 checkout，编译输入须与提交一致。入口会执行所有四个阶段；专门调用 `Collector.risk()` 的本地诊断只测量 Risk 阶段，不能冒充完整 CI 候选通过。

后续是否推广仍需目标 GitHub runner 上的重复测量，当前不默认启用。


## 本地实测结果

34 个针对性测试同进程通过；opencode 只读审查 PASS。
固定 base `4620e211d11d15644eaaa2030ce99d7aed118190`、head `2a552a17b894dfa9282fc102d09a1b3c50a5d90a`，本机 8 个逻辑 CPU，先串行后并行，仅一组完整 Risk 阶段对照。

| 指标 | 串行 | 并行 |
| --- | ---: | ---: |
| 进程总耗时 | 525.28 秒 | 457.56 秒 |
| Risk 阶段耗时 | 524.11 秒 | 456.42 秒 |
| 采样进程树 RSS 峰值 | 2.47 GiB | 4.90 GiB |
| base / head 测试 | 各 574 通过 | 各 574 通过 |
| base / head coverage 命令 | 235.80 / 230.06 秒 | 412.08 / 416.21 秒 |

总耗时减少 67.73 秒（12.89%）。RSS 是每 0.5 秒对进程树求和的采样值，共享页会重复计入，短暂峰值可能漏采；不是精确独占内存峰值。并行 base/head 各保留 1981 个 profile，实际路径分别位于 `build/base/llvm-cov-target` 和 `build/head/llvm-cov-target`。

最终 `risk.json` 相同，所有 selected failures 均为空。原始计数不完全一致：base/head 分别有 99/91 个函数记录不同。head 的覆盖率摘要、复杂度、CRAP 和通过状态相同；base 的非选定函数 `preset/initialize.rs::project_id` 区域覆盖为 30/32 与 29/32。该差异也出现在串行 base/head 之间，而两提交的 Rust 源码无变化，不能仅凭这一对实验归因于并行。原始差异完整保留，未按“等价”删除。

结论：代码修复与第一组诊断完成，保留默认关闭，不修改 CI workflow。没有证据支持原先节省约 240 秒的预估，也不把本机单组结果外推为 GitHub CI 收益。没有启动额外重复测量、提交或推送。

[结构化结果](ci-parallel-risk-local-result.json) 保留指标、差异摘要及证据摘要哈希。原始日志和报告位于本地 `/mnt/dev-ssd/gatehouse-ci-parallel-20261015-r3/`；测量脚本及审查证据位于 `~/.local/state/herdr-issue-flow/musutrade-GateHouse/ci-parallel-risk-20261015/`。
