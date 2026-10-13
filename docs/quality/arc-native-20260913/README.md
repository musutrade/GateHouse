# Arc Admin 原生质量接入验收 — 2026-09-13

Core **0.4.2** 的正式发布构建已经完成本机安装和 full／hook 验收。发布前验收的 Linux 文件与公开签名安装后的文件 SHA-256 完全相同：`eda5179d96b32269124150980ffcaa7e57878605133bc6ead51e5aeb8a6ad0a0`。

接入验收通过，项目质量结果仍是 **FAIL**。25 个原有执行步骤及 2 个前置检查全部通过；3 个认证 producer 提交 1,921 条真实记录，Git merge-base 基线可用，完整报告发布成功。5,760 项质量判定与原数值评估逐项相同，33 个 manifest 产物全部通过哈希核验。阈值、必需项、系列身份和失败阻断语义保持不变。

| 验收项 | 实际结果与凭据 |
| --- | --- |
| 正式 Core full | [完整执行回执](core042-full-acceptance.json)：Arc `859daa95b180c88f86fb817418e6b6ff5a5b1123`，真实基线 `d5e113ec2d49cd7dc30aaf2a14db016ba9d7ca94`；执行通过、质量 fail、证据完整 |
| 正式 Core hook | [精确构建验收](core042-artifact-acceptance.json)：调用实际 `harness-gate hook`，暂存区与工作目录内容不同，执行通过；完整质量明确为 `not_collected` |
| 报告完整性 | 同一精确构建回执记录全量判定比较、报告摘要和 33 项 manifest 核验；没有截断大型 JSON |
| 环境与服务 | 实际 api_flow 测试进程移除了 DATABASE_URL 并注入 TEST_DATABASE_URL；正常服务全部 CLEANED；[失败清理控制](service-failure-control.json)单独记录无效测试数据库名拒绝 |
| 真实 Git 基线 | [基线控制](baseline-controls.json)：正确基线接受，篡改摘要、缺失 manifest、错误 Git base 均拒绝；旧 lockfile 系列未映射到新系列 |
| 证据反例 | [集成传输控制](transport-controls.json)：篡改、缺失、过期、旧上下文、测试输入变化和缺少必需能力均拒绝；回执明确保留其原源码范围 |
| 路由 | [路由控制](routing-controls.json)：backend／frontend 各自命中；质量配置命中三组件；Core 与原 staged scope 一致 |
| 安装 | [公开安装回执](../release-0.4.2/public-user-install.json)：Core 0.4.2、collector 0.1.0-rc.3、installer 0.1.0-rc.4，旧版本保留 |

## 采集复用与实际质量

当前主分支的 chacha20 lockfile 从 0.10.1 改为 0.10.2，旧采集因此正确拒绝；只对变化的后端补采集，保留原二进制并两次重新导出一致，覆盖 1,778 个生产函数。前端和 API 输入未变，继续使用已审阅的原生数据。本轮 Core 修复和插件新版本验收没有重跑原生测量工具链的编译与采样。

前端 24 个测试文件、85 个测试通过，全部 141 个非测试 TS 文件进入测量。行覆盖率 877/1986（44.15%），函数覆盖率 239/578（41.34%），低于原有 80% 阈值。API 破坏性变更、生成漂移和兼容性三项通过；部分后端覆盖率和 CRAP 仍失败。5,760 个判定为 2,110 fail、3,391 pass、118 not_applicable、141 unsupported。Angular CRAP 保留 unsupported，没有合成替代数值。

项目测量包摘要为 `c3e17d97984ab8832a736b3ea6d475f8f6e0f38a88f20f2d63c49561748189b4`。项目继续使用其已接受的 RC2 来源数据及 measurement series；RC3 的独立分发兼容性验收不重标历史证据、不建立自动跨系列等价。Rust 插件负责测量，Core 负责信任、基线和质量决策。

## 成本观察边界

[完整成本记录](complete-quality-cost.json)保留原三 job 并发执行 345.851 秒，加同输入质量聚合 38.485 秒，阶段合计 384.336 秒；修复候选的 full 阶段合计 308.396 秒。另一次正式 Core 0.4.2 验收的主机准备与 full 合计 305.652 秒，精确时间见其回执。

两侧 300 项源码／测试／工具输入和 5 项质量配置字节相同，复用相同原生采集；原三 job 重复执行前置检查，总计 31 步，统一 full 为 27 步，业务步骤均为 25。共享主机 Cargo 缓存和锁，执行顺序导致后续缓存变热；阶段合计不含中间修复、下载、首次采集或候选构建。原拓扑质量聚合与统一报告的输出布局不同。只有这组有边界的观察，不能据此宣称统计性加速、冷缓存收益或替换正式 CI 的成本收益。

## 历史与移交

Core 0.4.1 的 [大型报告失败](released-core-large-report-failure.json)和[未发布候选凭据](arc-fixed-full-acceptance.json)原样保留。0.4.2 修复暂存区主机输入和大型 Core JSON 报告发布边界，外部证据的 16 MiB 限制仍保持。

Arc Admin 的 [接入 PR #40](https://github.com/musutrade/arc-admin/pull/40)、[当前系列 PR #41](https://github.com/musutrade/arc-admin/pull/41)和[正式验收文档 PR #42](https://github.com/musutrade/arc-admin/pull/42)已合并；正式 `cargo flow`、原 hook 和 CI 路由继续保留。本次完成接入与交付验收，不提升插件为稳定版，也不批准移除旧门禁。原始大报告、业务源码、采集二进制和私钥保留在操作主机；此目录只提交小型回执、摘要与可审阅结果。
