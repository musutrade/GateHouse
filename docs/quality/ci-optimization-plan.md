# Risk 收集并行实验

目标是验证在同一台 runner 上并行执行 base/head 的新鲜覆盖率收集，能否降低 Risk 阶段耗时。代码默认关闭，通过 `CI_PARALLEL_RISK=1` 显式启用；CI workflow 现已显式启用（见文末）。

历史 CI 数据见 [issue-61](issue-61/README.md)。历史串行耗时不能直接推导并行收益：两个构建会竞争资源，独立 target 目录也失去了串行模式的部分构建缓存复用。此前约 240 秒的节省估算不是实测结果。

实现约束：

- 两种模式共用源码范围检查、逐提交收集、哈希和源码身份校验、风险比较。
- 并行任务从收集器清理后的环境复制配置，分别使用 `build/base`、`build/head`；串行模式保留原 `build` 目录。
- 命令结束后，在同一把锁内更新命令记录并写入 `candidate.json`。锁不覆盖子进程执行。
- 一个任务失败时等待其余在途任务结束，保留原异常，不执行比较。收集器记录失败并拒绝将该候选视为通过。
- 保持现有测试范围、覆盖率和 CRAP 阈值，不复用旧覆盖率。

验证顺序：先完成针对性测试和源码审查，再在固定 base/head、工具链、依赖缓存和机器条件下做完整 Risk 阶段串行/并行对照。保留原始计数、失败日志、工具版本、命令耗时和资源采样。比较 `base-risk.json`、`head-risk.json`、`risk.json` 及原始覆盖率；路径和时间等元数据应明确区分，不能消除生产计数差异。

本地单组实验只能判断这台机器上的表现。目标 runner 的配对实测已完成（见[实施状态](ci-optimization-status.md)）：同一代码基线上，并行使 Risk 阶段从 398.99 秒降至 320.4 秒（-78.6 秒，-19.7%），Quality job 从 761 秒降至 627 秒，两个 run 均通过全部检查且无 OOM。据此在 `.github/workflows/ci.yml` 的质量收集步骤显式设置 `CI_PARALLEL_RISK: '1'`；建议对随后 1-2 个 PR run 观察稳定性后再将收益视为已确立。结构化证据见 [hosted 结果](ci-parallel-risk-hosted-result.json)。

进度和实际证据见 [实施状态](ci-optimization-status.md)。
