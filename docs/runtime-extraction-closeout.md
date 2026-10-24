# 容器运行时剥离：设计验收与交付边界

复核日期：2026-10-24。源码基线为 Core `304c28671014989ed4a8f5ca3e13b58b27091e9d`；
发布基线为 Core 0.14.1，准确身份见[公开回执](releases/0.14.1-public-verification.md)。
本文对照 [Issue #62](https://github.com/musutrade/GateHouse/issues/62) 的设计验收，
不把设计完成解释为所有平台、所有引擎故障场景均已认证。

## 设计要求与证据

| #62 要求 | 已交付的设计、实现或验证依据 |
| --- | --- |
| 职责矩阵 | [边界矩阵](runtime-plugin-boundary.md#已确认的边界)：Core 持有通用编排和最终判定，插件执行引擎操作 |
| 状态持有人及失败责任 | [生命周期日志与失败策略](runtime-lifecycle-v1.md#lease-state-machine-and-failure-policy-revision-2)：Core 写持久意图；不确定结果保留；插件响应不能授予所有权 |
| 协议、能力和错误模型 | [受保护 host 与 wire 契约](runtime-lifecycle-v1.md#protected-host-and-wire-format-revision-2)；独立 lifecycle-v1 不复用 service-check 的成功结果授予修改权限 |
| 身份、互斥、期限、恢复及幂等性 | [revision 3 操作矩阵](runtime-lifecycle-v1.md#revision-3-operation-matrix)、[显式操作员恢复](runtime-lifecycle-v1.md#revision-4-explicit-operator-reconciliation)；未知 create 不自动重放，移除须证明精确 ID 缺失 |
| 最小迁移切片和 Core 最终权威 | 历史[隔离实验](../tools/experiments/runtime-lifecycle/README.md)及后续[公共服务路由](runtime-lifecycle-v1.md#public-managed-service-routing-checkpoint)；报告目录保护与 requiredness 留在 Core |
| 故障验证及边界 | 下节列出源码回归和真实引擎验收的不同范围；不以正常路径成功冒充完整故障矩阵 |
| 配置、安装、升级/回退及发布 | [显式安装契约](runtime-lifecycle-v1.md#authority-and-installation)、[旧路径退役与迁移步骤](runtime-lifecycle-v1.md#legacy-cli-retirement)和公开交付回执 |
| 政策变更与后续实施 | [Engineering Policy 第 7 节](engineering-policy.md#7-project-owned-validation-and-extension-boundaries)未改变；旧引擎实现已移除，通用状态管理不属于遗漏的剥离工作 |

## 验证范围

可审查的回归包括：

- [安装校验测试](../tools/harness-gate/runtime-protocol/src/lifecycle/installation/files/tests.rs)：保护文件、精确产品/版本与引擎身份，不从环境自动选后端。
- [日志测试](../tools/harness-gate/runtime-protocol/src/lifecycle/journal/storage/tests.rs)：原子写失败窗口、创建令牌不可重放、移除后崩溃及 tombstone 恢复。
- [资源槽测试](../tools/harness-gate/runtime-protocol/src/lifecycle/journal/storage/slot/tests.rs)：并发、失败启动、父进程退出后子进程持锁、外来或旧租约不得隐式接管。
- [Core/adapter 联合测试](../tools/harness-gate/src/process/runtime_lifecycle_host/tests/joint.rs)：启动/健康/停止/移除失败、错误端口、非精确缺失、显式操作员恢复和标记保留。
- [退役测试](../tools/harness-gate/src/service/tests/retirement.rs)：缺 lifecycle host 时拒绝，陷阱可执行文件证明没有调用旧引擎 CLI；旧租约诊断保留。

这些是源码回归范围，不是声称本次文档修改重新执行了全部测试。
发布回执记录精确 CI 运行及下载后的真实 Linux x86_64 / rootless Podman 5.7.0
正常生命周期验收。daemon 重启、额外平台及 OS containment 不在该真实验收声明中。
心跳、锁或父进程退出不能单独授权销毁对象；状态不确定时保留并要求显式恢复。

## 历史制品与兼容声明

`runtime-importer-v0.7.0` 已发布，并正式声明 Core `0.13.0` 与 `0.14.1` 兼容；
下载归档对两个已发布 Core 二进制都通过了安装与真实引擎验收。当前目录状态为
已发布，见 [importer 0.7.0 回执](https://github.com/musutrade/GateProbes/blob/7fb3cf289dba4db1ce1ce8e790e61c170ad1ba48/docs/releases/runtime-importer-0.7.0-acceptance.md)。

podman-lifecycle 0.3.0 的不可变归档仍只声明 Core 0.13.0，Podman 归档中的
开发候选状态和“尚未发布”文字也是打包时的旧值；这是不可变的打包历史，
不是当前“未发布”的事实。当前发布状态与已验证互操作见
[adapter 状态回执](https://github.com/musutrade/GateProbes/blob/7fb3cf289dba4db1ce1ce8e790e61c170ad1ba48/docs/releases/podman-lifecycle-0.3.0-status.md)，
该回执不会改变归档字段。历史 importer 0.1–0.6 制品、pin 与 tag 保持不变。
后续兼容声明必须绑定原始制品身份并明确验收范围；不得替换旧 tag、签名或归档，
也不得把补充声明伪装成 Core 安装校验的替代品。真实验收范围不因本文扩大。

旧 CLI/v2 租约继续保留诊断与显式迁移边界。部署环境应按迁移步骤逐一检查，
不能从某台机器的零记录审计推断其他环境无需迁移。业务仓库的外部数据库配置
属于业务侧配置，不代表 Core 内含数据库引擎实现。
