# Documentation / 文档目录

Start with [English README](../README.md) or [中文 README](../README.zh-CN.md).
Current published versions, verified boundaries and issue disposition are in
[release status](release-status.md). Dated issue evidence and ADRs describe what
was true when recorded; use the current guides below for installation and use.

| Task / 任务 | Guide / 入口 |
| --- | --- |
| Install Core or optional Rust plugin / 安装 | [English](../README.md#installation), [中文](../README.zh-CN.md#安装), [Rust installer and offline kit](quality/rust-collector-installation.md) |
| Integrate a project / 接入项目 | [Quality workflow](quality-workflow.md), [reference presets](quality-presets.md) |
| Configure execution / 执行配置 | [English reference](configuration.md), [中文参考](configuration.zh-CN.md), [schemas](../schema/README.md) |
| Configure quality / 质量配置 | [Quality configuration](quality-configuration.md), [trusted collectors](quality-collectors.md) |
| Diagnose failures / 排查 | [Failure codes](failure-codes.md), [中文修复路径](../README.zh-CN.md#常见修复路径) |
| Release or contribute / 发布与贡献 | [Release governance](release-governance.md), [contributing](../CONTRIBUTING.md), [engineering policy](engineering-policy.md), [CI guide](quality/ci-topology/README.md) |
| Route documentation-only CI changes / 文档 CI 分流 | [Documentation-only routing](quality/ci-docs-routing.md) |
| Keep the checkout small / 仓库与本地产物整理 | [Incremental size limits](repository-size-policy.md), [Local storage preview and cleanup](local-storage.md) |
| Review delivered work / 验收与待办 | [Current release and issue status](release-status.md), [Arc Admin native acceptance](quality/arc-native-20260913/README.md), [historical specifications](specs/README.md), [decisions](adr/README.md) |
| Restore historical evidence / 历史证据恢复 | [Archive index, scope and verified restoration](quality/archived-evidence.md) |

Core installation, collector installation, native measurement certification and
project policy adoption are separate milestones. A preset or an installed plugin
does not create a trusted baseline or prove the project's quality gates pass.
