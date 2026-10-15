# 预览验证执行计划

统一入口为 verify --plan；--json 输出
[verification-plan/v1](../schema/verification-plan.schema.json)。scope 继续提供
较窄的路径和组件信息，不增加另一套 plan/--explain 选择实现。

~~~bash
harness-gate verify --plan
harness-gate verify --plan --staged --profile hook
harness-gate verify --plan --base origin/main --profile full --json
harness-gate verify --plan --all --json
harness-gate verify --plan --components backend,frontend --profile full --json
~~~

组件显式选择与 staged/base/all 互斥，profile 来自显式参数或项目默认值。
check 别名同样支持 --plan；不带 --plan 的 verify --json 会报用法错误。

成功预览退出 0，status=planned；这只表示计划已构造，verification_result
始终为 null，runtime_inputs=not_validated，不代表验证 PASS。
配置缺失/非法、未知组件/profile、依赖错误、unmatched=fail 等退出 1，
返回 status=invalid、plan=null 和包含具体说明的 PLAN_INVALID 诊断，
保留配置诊断的字段位置等信息，不把不完整计划标成验证通过。
CLI 参数用法错误继续使用 Clap 的非零退出和用法输出。

计划展示路径→匹配规则→组件→profile→步骤，以及依赖、强制内置门禁、
服务消费者与检查绑定、质量采集器/策略的参与或排除、能力/测量系列、
组件/subject 声明、workflow state、可信密钥和基线请求路径。
每个选择或排除项都有稳定原因代码和人类说明；完整代码表见[英文契约](verification-plan.md)。
规则索引从 0 开始。选中步骤按执行的稳定拓扑顺序排列，排除步骤按配置顺序排列；
execution_order 只包含选中节点。有效计划没有时间戳、invocation ID 或临时快照路径。

组件/profile 过滤不会移除强制 secrets/audit 门禁，依赖闭包可以补入其他
组件/profile 的步骤。跨组件重命名包含旧、新两端，支持空格和换行文件名；
unmatched=all 只有真实未匹配路径时才回退，干净树仍只选强制内置门禁。
选择逻辑、编译规则匹配器和 VerificationPlan::build 与实际执行共用。

## 只读边界

预览不调用 Project::prepare，不创建 reports/leases，不执行步骤、服务、
采集器或工具探测，不获取基线、不认证或消费签名请求、不更新 nonce ledger。
只读取和校验必要配置、策略文件及 Git 元数据；禁用 Git 可选 index 写入、
文件系统监视钩子及外部 diff/textconv 命令。不会输出连接字符串或环境变量值。

staged 使用私有临时配置视图，保留 index 路径存在性，只读取必要 flow、
quality 和 policy blob；不运行 write-tree、checkout-index 或 checkout filter，
不读取源码/签名请求正文，结束后清理临时目录。冲突、软链接、submodule
index 条目会明确报错。配置文件若在 index 或工作区属性中使用 checkout filter、working-tree-encoding 或 ident，
预览也明确拒绝，因为只读预览不能执行过滤器或承诺实际配置输出；
普通源码文件的过滤器不运行，也不会妨碍预览。

服务可用性、程序/环境、签名真实性、nonce 新鲜度和运行证据尚未验证，
列出的输入是需求，不是批准。Git/配置之后变化会改变计划，应执行普通 verify
获得权威结果。requiredness、阈值、baseline/debt/ratchet、waiver、测量系列
和 Core 最终决策及发布权威保持不变。

## 兼容与回滚

这是新增 CLI/JSON 接口，不修改 flow v2、quality v1、scope JSON 或验证报告。
消费方检查 schema 标记；不兼容输出变化必须升级版本。回滚时一起撤回 plan
参数、schema 和解释模块，不重置已有证据或基线。

需要 Git 的预览要求 Git 支持 --no-lazy-fetch。部分克隆缺失对象时明确报错，不调用远程 helper、不获取或写入 Git 对象；普通执行命令保留原有 Git 行为。预览也跳过可选的 HEAD 对象探测，因为它不生成执行用的源码身份。

Core 自测源码选择扩展为 gh29-verify-plan/1，base/head 按同一扩展集合重新测量；旧报告保留，阈值和债务棘轮不变，详见[测量迁移记录](quality/issue-29/README.md)。这不改变产品插件的测量系列。

组件影响图 v1 为每个组件增加 `impact_chain`（确定性的最短来源链）、
`impact_sources`（原始改动路径和规则索引）以及 `depends_on_components`
（完整声明的依赖关系）。间接选中原因是 `COMPONENT_IMPACT`。参见
[组件影响配置](configuration.zh-CN.md#可声明的组件影响传播可选)。
