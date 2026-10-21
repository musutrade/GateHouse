# 容器运行时插件边界提案

状态：#62 的设计草案，已完成隔离实验，修复后的隔离实验已通过独立复审；不代表插件已交付。
源代码核对基线：Core `d755db0879b03950fa58d8662f4dfb979d6a17bf`，
GateProbes `53c01505bed2ff61dfad1d106abbec2dcd961d05`。
本提案不修改 [Engineering Policy](engineering-policy.md) 第 7 节，也不改变
质量阈值、证据身份、历史债务、非退化规则或最终判定权。

## 已确认的边界

| 职责 | 目标归属 | 状态持有人 | 失败责任 |
| --- | --- | --- | --- |
| requiredness、依赖、重试、期限、取消、最终阻断 | Core | Core invocation | Core 判定并阻断 |
| 报告目录保护、资源租约、PID/start identity、心跳 | Core | Core 私有持久状态 | Core 保留不确定状态并报告 |
| Docker/Podman 命令及结果规范化 | 候选 GateProbes adapter | 引擎持有对象；adapter 返回事实 | Core 验证事实后决定后续动作 |
| 服务特定健康检查 | GateProbes recipe | 检查结果 | recipe 返回事实；Core 决定 required gate |
| 崩溃恢复 | Core 调度固定 adapter | Core 持久化操作意图及身份 | 缺失或不兼容 adapter 阻断清理 |

#51 的 `service/lease/container.rs` 是 Core 内部拆分，不是插件接口。
报告目录保护和通用租约仍留在父模块，不能按文件总行数估算可移出的生产代码。

GateProbes 的 `protocol/service-check.schema.json` 只定义
`protocol/plugin/version/mode/success`，禁止额外字段。Core 的
`recipes/mod.rs` 核对制品摘要、响应身份及模式，执行具有输出和时间边界。
Postgres 插件 external 模式调用健康检查；managed 分支不执行容器生命周期操作。
因此现有 `gate-service-check/v1` 不提供启动、移除、持久租约、心跳或恢复协议。
生命周期需要独立版本契约；不得把 managed success 解释为生命周期能力。

## 候选生命周期契约

协议版本与插件发布版本分离。Core 在任何副作用前校验固定制品及所需能力；
请求和响应绑定 invocation、operation key、租约 generation、操作类型及运行时命名空间。
拒绝重复字段、尾随对象、身份不符及该版本不允许的字段。
这些是待实现要求，不是现有接口。

inspect 返回 `present`、`absent` 或 `unavailable`；修改操作返回
`completed`、`rejected-before-effect` 或 `unknown`。派发后超时、取消、非零退出、
丢失或损坏响应默认是 unknown，不能推断引擎操作已经撤销。
只有相同固定运行时命名空间中**精确对象 ID 不存在**才可以完成移除意图的对账；
按名称不存在或普通检查失败均不足以证明成功。

Core 保存 engine、endpoint/context、用户/rootless namespace、对象 ID、所有权标签、
租约 generation 与 adapter 身份。恢复不得继承已变化的 Docker/Podman context。
标签与对象 ID 都匹配后才允许移除，且只能向 adapter 传已验证 ID，不能回退到名称。
当前 stop 路径实际执行强制移除，不承诺优雅停止。

在资源互斥保护下重新核对租约 generation，并持锁至状态提交：

1. 副作用前持久化 prepared intent；操作键绑定完整目标和请求，同键不同请求拒绝。
2. 经检查绑定对象身份，移除前持久化 remove-pending。
3. 结果不明确则保留 unknown；新进程加载同一意图进行精确 ID 对账。
4. 只有有效完成证明才能提交 completed 并删除同一 generation 的标记。

操作键本身不提供幂等保证。现有 lease v2 对缺少 runtime_object_id 的容器标记
采取保留调查处理，不能宣称已支持部分启动自动恢复。报告目录锁也不能替代通用资源操作锁。

Core 使用本地单调时钟强制期限和有界终止/排空预算；传输的墙钟截止时间只供参考。
保留进程树终止、单流及合计输出上限、受限环境和不泄漏秘密的诊断。
取消后的清理使用独立有界预算，不能因父操作已取消而直接跳过；杀死插件进程不证明
引擎端请求没有生效。adapter 只返回事实，无权授予租约所有权或最终成功状态。

实现时按操作验证条件响应：inspect present 必须携带非空、类型合法的对象身份及标签；
absent/unavailable 必须携带 null object。remove 只允许 completed、rejected-before-effect
或 unknown，不能携带对象字段。能力列表必须是无重复的字符串列表，协议标识和所有
绑定字段必须按声明类型精确匹配。错误类型、额外字段及矛盾组合均拒绝，不得对账成功。
可信 rejected-before-effect 表示该次调用尚未产生副作用，应保留意图并报告拒绝原因；
重新派发前仍须检查身份、能力和剩余预算，不能据此清除之前调用留下的 unknown。

## 最小验证与验收边界

第一切片为独立实验 host 加假运行时 adapter，仅 inspect 和按已验证 ID 移除。
预置已绑定租约，不接管生产分配/启动，不修改默认配置。实验必须真实启动子进程，
记录目标调用并用独立断言核对持久状态；不能仅模拟协议对象后宣称执行路径成立。

| 场景 | 必须观察到的结果 |
| --- | --- |
| 正常 inspect/remove | 仅移除已验证 ID，提交匹配 generation |
| 缺制品、摘要/协议/能力/响应身份不符 | 零破坏性派发，标记保留 |
| 外来标签、项目/ID/namespace 变化 | 零移除；inspect 后名称复用也只针对旧 ID |
| 移除后响应丢失 | 首次 unknown；新 host 加载意图，仅凭同命名空间精确 ID 缺失对账 |
| inspect unavailable | unknown 保留，不伪装 absent |
| 移除前后退出或超时 | 不直接成功；重试不能触碰占用旧名称的新对象 |
| 同键不同请求、并发清理或标记换代 | 拒绝混用；不能提交另一 generation 的租约 |
| owner 存活或身份不能证明 | 心跳过期也不清理 |
| adapter 被替换或删除 | 恢复失败并保留标记，不自动选新二进制 |
| 取消、超量或截断输出 | 有界终止进程树，不接受残缺 JSON，不改变报告目录状态 |

本地独立假运行时子进程实验20 个场景通过，覆盖正常移除、响应丢失和超时后
恢复、检查不可用、协议错误、损坏/超量输出、拒绝操作、外来所有权、存活 owner、
错误摘要和操作键，以及能力缺失、标记换代、并发清理、恢复时制品替换/丢失，以及取消/超时后的进程组终止。
进程树实验仅覆盖未脱离进程组的 Linux 子进程，不代表跨平台终止保证。
独立代码审查确认上述记录与源码摘要一致，但发现以下实验限制，不能作为迁移验收。

- 首轮审查发现锁等待和 stdin 写入缺乏期限；现已增加非阻塞期限/取消检查，
  并用锁超时、等待锁时取消和 adapter 不读取大请求三个子进程场景验证。
  不承诺操作系统调度或文件系统 fsync 的硬实时上限。
- 首轮审查发现条件响应不完整；现已补严格类型及条件校验，新增十项直接校验测试。
  rejected-before-effect 保留 prepared；若此前已有 unknown/remove-pending，仍保留 unknown。
  修复后的独立复审已接受声明范围内的实验，无代码阻断项。
- 同名替代对象在检查前就存在；未模拟检查后发生的名称替换竞态。
- 损坏/超量输出发生在能力预检阶段；不证明修改操作响应或 stderr/合计上限的全部路径。
- owner 为固定模拟输入，错误 key 为损坏值；不证明真实 owner 判活或完整重放攻击检测。
假运行时中的 owner/heartbeat 为模拟事实。
实际 Docker/Podman、启动部分成功、真实心跳中断和父进程退出、跨平台终止以及完整
安装/升级/回退仍需各自验证，实验通过不能替代这些验收或关闭全部迁移工作。

## 安装、兼容和后续实施

候选 adapter 以固定摘要的不可变安装路径发现，安装目录写权限属于受信安装者；
仍有租约引用的旧制品必须保留。执行前后摘要检查不能单独消除路径替换竞态，
实现须同时约束安装目录和依赖来源，不得动态选择未经固定的运行时依赖。

新生命周期协议与现有 v1 检查共存。现有内置 lease v2 继续由原有路径处理，
不得无验证重写成插件来源。升级只影响新选择，旧租约恢复使用原固定身份；
回退前检查所有活跃租约的协议及制品是否仍可用。缺制品时保留标记并给出恢复指引，
不自动下载最新版或回退按名称清理。

后续分别安排 Core 协议执行/日志及互斥实现、GateProbes adapter 实现、真实引擎故障
矩阵和联合发布验证。默认切换、内置实现移除及配置迁移时间以这些验收为前提，
本设计不预设 feature flag 名称、发布时间或性能收益。若后续迁出通用编排，必须先
提出明确政策变更；本提案只评估运行时实现外移，无需修改既有职责政策。

## 复现与后续验收分工

实验源码和一次性运行入口见 [runtime-lifecycle](../tools/experiments/runtime-lifecycle/README.md)。
运行入口自动清理临时状态与报告，不向仓库写入证据。当前 20 项假运行时场景加
13 项边界检查通过，其中 10 项为直接响应校验，不应表述为 33 项端到端场景。

| 验收内容 | 当前证据范围 | 后续责任 |
| --- | --- | --- |
| 固定对象 ID、unknown 恢复、缺失/替换 adapter | 假引擎子进程已演练 | Core host/state machine；GateProbes 真实实现对照 |
| 协议/能力拒绝、锁并发与取消、输出和期限 | 已演练部分失败路径；不是全组合证明 | Core 协议执行与锁实现补全条件矩阵 |
| 启动部分成功、真实心跳中断、父进程崩溃 | 尚未验证；当前启动未绑定标记保留调查 | Core 持久意图/恢复任务与联合故障注入 |
| daemon context、rootless 身份、真实标签和资源竞态 | 仅模拟命名空间和身份 | GateProbes adapter 与真实 Docker/Podman 验收 |
| 固定制品安装、升级/回退、旧租约兼容 | 仅设计规则，未交付安装实现 | Core/GateProbes 联合发布验收 |

后续 Core 工作包括版本化 wire schema、受控 transport、资源锁及持久意图状态机；
GateProbes 工作包括独立 adapter、引擎身份绑定及 inspect/remove/start 的逐项实现；
联合工作包括真实故障矩阵、安装升级回退和旧租约兼容。各项保持独立验收，
不能以设计实验通过替代默认切换或生命周期迁移完成。

## 原始字节传输准备切片

`process/raw_adapter.rs` 从已有 signed adapter 提取子进程传输：写入请求及
换行、有界 stdout/stderr、等待与取消、正常退出后的进程组收尾及 reader/writer
排空预算。原 signed adapter 已使用该模块；签名、制品再校验、环境构造、响应
协议和最终取消检查仍由原调用方负责。模块返回原始字节，不经过 JSON `Value`，
以免后续严格协议解析前丢失重复字段。

这是现有行为的内部复用准备，不是运行时 adapter host：没有新增命令、引擎
选择、持久意图、能力协商或租约迁移。底层暂复用 `HostPolicy` 的输出预算与
`AdapterError`；调用方仍须鉴权并验证退出状态、输出限制和最终取消状态。
不能直接把这个内部函数当作受信执行入口。现有平台进程清理限制不变。

## 显式旧引擎实现边界

`service/runtime.rs` 的 `ContainerRuntime` 只声明操作，不再提供隐式执行
Docker/Podman CLI 的默认方法。原 CLI 实现在 `service/legacy_cli.rs`；
包括 doctor 镜像检查在内的引擎调用通过该接口进入显式实现。
新后端必须逐项实现操作，不能因遗漏实现而继承环境中的 Docker 命令。
这只是 Core 内部迁移准备，旧服务仍选择原 CLI 后端；并未将引擎实现发布到
GateProbes，也未新增配置选择、v2 mutation、absence 或旧租约转换。
租约所有权检查、恢复、取消与最终判定继续由原 Core 路径负责。

只读 `check_available` 路径通过延迟后端解析函数共享同一编排：未知服务、
外部环境值以及 recipe 拒绝都发生在后端选择之前；引擎 info 成功后才检查镜像，
两次调用共用剩余期限。生产入口仍显式选择配置中的旧 CLI 引擎。
内存假后端测试覆盖 Docker/Podman 选择传递、调用顺序、info/镜像失败、
info 耗尽预算和外部服务绕过；任何启动、删除、端口或容器命令调用都会使测试失败。
这个注入点不接入 ServiceManager 的启动/清理路径，也不把只读结果当作租约权限。
