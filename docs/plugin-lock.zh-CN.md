# 显式插件接入（v1）

Core 使用通用锁定接口接入数据型预设目录，插件名称无需写入 Core 枚举。
GateProbes 负责产品清单和显式准备、安装、升级、回滚；Core 验证格式、
签名、精确身份、平台、协议、文件摘要和依赖。

项目提交 `.harness-gate/plugins.lock.json`；操作者通过
`--plugin-store PATH --plugin-trusted-key PATH` 指定离线产物与独立信任的公钥。
项目锁文件、预设或产品清单均不能批准密钥、授予权限或安装工具链。
使用 `init --plugin NAME --preset NAME` 选择已经锁定的预设，然后运行
`config check --format json`、`doctor --json`、`verify --all`。

没有锁文件的项目保持原有显式目录接口；不兼容的版本不自动迁移。
暂存区验证与计划读取索引中的锁文件。预检不联网、不执行依赖程序、不消费
签名请求；缺失依赖、摘要/签名错误以及协议/平台/版本不匹配都有稳定诊断及
修复提示。已准备的有效产物可离线复验。

v1 支持 `harness-preset-catalog/v1` 数据目录，完整字段、签名字节约定、
诊断编号、兼容和回滚规则见[协议说明](plugin-lock.md)。
[GateProbes 实例](https://github.com/musutrade/GateProbes/blob/main/docs/plugin-onboarding.md)
提供 Rust、Python 全新项目的实际步骤和验收脚本。
它们明确是仅执行检查的最小预设；不会声称已经配置质量收集器。
启用质量工作流仍需宿主可信状态、完整请求签名、能力授权和重放保护。

安装前先生成并审阅计划；应用时保留旧锁文件的精确字节、目标身份和回滚收据。
显式回滚恢复旧锁，保留不可变产物供审计。安装与升级不修改项目策略、
measurement series、baseline、debt、ratchet、required gates 或系统工具链。
Core 始终保留最终判断和发布权威。v1 不引入 marketplace、自动发现或常驻服务。
