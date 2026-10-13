# harness-gate schema v2 配置参考

主流程为 **`init -> verify -> one project decision`**。先阅读[质量快速开始与架构](quality-workflow.md)，了解 capability 状态、认证范围、baseline/profile、artifact 信任、迁移与故障排查。生态/语言/框架名称属于配置与 pack 数据，通用层不需要为新增语言名称修改核心代码；`quality evaluate` 和 `adapter run` 保留为高级接口。

可选的 `.harness-gate/quality.toml` v1 配置、采集器权限和跨配置校验见
[质量配置参考](quality-configuration.md)。没有该文件时保留原有 flow 行为。


本文档说明 `.harness-gate/flow.toml` 和审计规则文件的完整配置模型。首次接入请先阅读上一级的 [操作手册](../README.md)，再按需查阅本参考。

## 1. 文件与加载顺序

默认项目配置是仓库内的 `.harness-gate/flow.toml`。`harness-gate` 从当前目录向父目录查找该文件，也可以显式指定：

```bash
harness-gate --project-root /path/to/project config check
harness-gate --project-root /path/to/project --config config/ci-flow.toml config check
```

配置路径必须位于项目根内部。相对路径按项目根解析；绝对路径、`..` 越界路径和指向仓库外的符号链接会被拒绝。

默认情况下只读取当前项目的 `.harness-gate/flow.toml`。配置字段声明的
环境变量只覆盖对应字段；需要选择其他项目或工作流文件时，显式使用
CLI 的 `--project-root` 和 `--config`。

使用以下命令查看配置是否有效以及环境覆盖后的结果：

```bash
harness-gate config check
harness-gate config check --format json
harness-gate config print --resolved
harness-gate schema export
```

`schema export` 将当前配置模型导出到 `schema/flow.schema.json`，可提交到编辑器或 CI
进行静态补全和结构检查。配置字符串支持一次性的环境变量插值：`${NAME}` 要求变量已设置，
`${NAME:-default}` 在变量缺失时使用默认值。不支持递归插值或表达式。插值发生在 TOML 解析前，
随后仍按现有专用环境变量覆盖规则处理。`paths.audit_config` 是例外：必须使用字面量的
仓库相对路径，不支持环境插值，以确保审核策略始终属于当前项目。

`config check` 会在启动 service、子进程或写入报告前完成静态验证。默认输出面向
终端；`--format json` 将 stdout 固定为一个版本化诊断对象，方便 CI 和编辑器集成。每项
错误包含稳定的 `HGCFG-*` ID、字段路径、原因、修复建议，以及文件输入可用时的行/列；
不会显示插值后的环境变量值、连接字符串或模板内容。JSON Schema 只验证结构，环境插值、
环境覆盖、仓库路径、跨字段引用和资源安全必须由 `config check` 验证。

### 编辑器 Schema 关联

仓库中的 `schema/flow.schema.json` 是 `.harness-gate/flow.toml` 的权威本地 Schema。
先在仓库根执行 `harness-gate schema export`（CI 会检查其与已提交文件一致），再使用以下
本地关联；两种方式都不要求远程 URL：

**VS Code（Even Better TOML）** 在工作区 `.vscode/settings.json` 中加入：

```json
{
  "evenBetterToml.schema.associations": {
    "./schema/flow.schema.json": [".harness-gate/flow.toml"]
  }
}
```

**Taplo** 在仓库根的 `.taplo.toml` 或个人 Taplo 配置中加入：

```toml
[[schema.associations]]
url = "./schema/flow.schema.json"
include = [".harness-gate/flow.toml"]
```

编辑器应先展示结构性错误，再运行 `harness-gate config check --format json` 获取路径、
依赖与资源安全诊断。

## 2. 命名和路径约束

所有 project、component、profile、step、parser、service、doctor check 和 path alias ID 必须：

- 非空；
- 只包含小写 ASCII 字母、数字、`.`、`-`、`_`；
- 例如 `api.tests`、`frontend`、`pre_commit` 合法，`API Tests` 不合法。

环境变量名只能包含大写 ASCII 字母、数字和下划线。`program` 必须是 PATH 中的可执行文件名，不能包含 `/` 或 `\\`。

仓库路径必须是非空相对路径，不能包含父目录跳转。以下占位符可用于参数、Doctor 路径和部分配置值：

| 占位符           | 含义                                                       |
| ---------------- | ---------------------------------------------------------- |
| `{root}`         | 项目根的绝对路径                                           |
| `{reports}`      | 报告目录的绝对路径                                         |
| `{audit_config}` | 审计规则文件的绝对路径                                     |
| `{<alias>}`      | `[paths.aliases.<alias>]` 解析后的绝对路径                 |
| `{host_port}`    | 仅 Docker service 的 `connection` 使用，由随机宿主端口替换 |

步骤的 `cwd` 必须是单个 `{root}` 或 `{<alias>}`，不能写成 `{root}/backend`。需要子目录时先声明 path alias。

## 3. 顶层结构

```toml
version = 2

[project]
# ...

[paths]
# ...

[policy]
# ...

[execution]
# parallel = false
# max_parallel = 4

[notifications]
# [[notifications.webhooks]]
# url = "https://hooks.example.test/events"
# allowed_hosts = ["hooks.example.test"]
# on_failure = true
# on_success = false

[[doctor.checks]]
# ...

[services.example]
# ...

[parsers.example]
# ...

[[scope.rules]]
# ...

[[steps]]
# ...
```

| 区域                | 必需 | 用途                        |
| ------------------- | ---- | --------------------------- |
| `version`           | 是   | 当前固定为 `2`              |
| `[project]`         | 是   | 项目标识和默认 profile      |
| `[paths]`           | 是   | 报告、审计规则和路径别名    |
| `[policy]`          | 否   | 声明不可缺失的步骤          |
| `[execution]`       | 否   | 控制依赖就绪步骤的并行调度  |
| `[scope]`           | 否   | 未匹配变更路径的处理策略    |
| `[[doctor.checks]]` | 否   | 本机环境体检                |
| `[services.*]`      | 否   | 外部环境或 Docker 临时服务  |
| `[parsers.*]`       | 否   | 从测试日志计算结果数        |
| `[[scope.rules]]`   | 是   | 变更路径到 component 的映射 |
| `[[steps]]`         | 是   | 实际执行的命令步骤          |
| `[report_templates]` | 否  | 可选 HTML 模板和 JUnit 输出配置 |
| `[notifications]`   | 否   | 可选 HTTP Webhook 通知         |

未知字段会直接导致解析失败，避免拼写错误被静默忽略。

## 4. `[project]`

```toml
[project]
name = "orders-api"
default_profile = "full"
hook_profile = "hook"
```

| 字段              | 说明                                      |
| ----------------- | ----------------------------------------- |
| `name`            | 项目 ID，也用于临时容器名称               |
| `default_profile` | `harness-gate verify` 未传 `--profile` 时使用 |
| `hook_profile`    | `harness-gate hook` 使用的快速 profile        |

两个 profile 都必须至少被一个步骤引用。profile 不需要单独声明，它由步骤的 `profiles` 集合产生。

## 5. `[paths]` 和 aliases

```toml
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"

[paths.aliases.api]
path = "services/api"
env = "ARC_FLOW_API_DIR"

[paths.aliases.web]
path = "apps/web"
```

| 字段                | 说明                               |
| ------------------- | ---------------------------------- |
| `reports`           | JSON、Markdown 和步骤日志目录      |
| `audit_config`      | auditor 规则文件                   |
| `secrets_config`    | Secret Scan 规则及占位符策略文件   |
| `aliases.<id>.path` | 仓库内目录或文件路径               |
| `aliases.<id>.env`  | 可选；覆盖 alias 路径的环境变量    |

`root`、`reports`、`audit_config`、`host_port` 是保留 alias 名称。

通用覆盖变量：

| 环境变量                                    | 覆盖字段             |
| ------------------------------------------- | -------------------- |
| `REPORT_DIR`（兼容别名，告警至 2027-03-01）或 `HARNESS_GATE_REPORTS` | `paths.reports`        |
| `HARNESS_GATE_SECRETS_CONFIG`               | `paths.secrets_config` |

`paths.audit_config` 始终从当前项目的 `flow.toml` 读取，且必须是字面量的
仓库相对路径，不能使用 `${...}` 环境插值。`PROJECT_ROOT`、
`HARNESS_GATE_CONFIG`、`AUDITOR_CONFIG` 和 `HARNESS_GATE_AUDIT_CONFIG` 不参与
项目或审核配置发现，避免共享终端、IDE 或 CI 环境中的一个项目影响另一个项目。
需要选择其他项目或工作流文件时，显式使用 `--project-root` 和 `--config`。

### 5.1 Invocation input 与步骤输入

每次验证都会先建立 invocation input descriptor，并记录输入模式、项目身份、源身份、执行根目录和配置摘要。
普通 `verify` 默认使用工作区；`scope --staged` 和 `hook` 会把完整 Git index 物化到一个私有临时目录，
再从该 snapshot 加载配置、选择 scope、运行内置门禁和普通步骤。临时 snapshot 在 invocation 结束后自动清理。
因此 hook 不会把暂存内容和未暂存工作区内容混合成一个结果。

步骤的 `input` 字段默认为 `"snapshot"`（`"staged"` 是兼容别名）：

| `input` 值 | 行为 |
| --- | --- |
| `snapshot` | 从当前 invocation 的执行根读取文件；hook 中指向完整 staged snapshot |
| `repository` | 显式使用原始 checkout，适用于确实需要 Git 元数据或工作区能力的兼容步骤 |

`repository` 是需要审阅的显式兼容能力，不会改变 secret scan、architecture audit 或 scope 的输入语义。
步骤的 `{root}`、path alias 和参数占位符都会按所选输入解析。machine result 和 `invocation.json` 会记录
`input_mode`、`source_identity`、`execution_root` 与 `configuration_digest`，便于复现一次决定。

## 6. `[execution]`

默认配置保持串行执行。只有显式设置 `parallel = true` 才会并发运行没有依赖路径的步骤：

```toml
[execution]
parallel = true
max_parallel = 4
```

| 字段           | 默认值 | 说明 |
| -------------- | ------ | ---- |
| `parallel`     | `false` | 是否启用受依赖关系约束的并行调度 |
| `max_parallel` | 并行时为 `4` | 同时运行的步骤上限，显式值必须为 `1` 到 `64` |

`max_parallel = 0`、负数或大于 `64` 会在执行前拒绝。并行步骤仍受
`depends_on`、服务资源预检和运行时服务锁约束；secret scan 与 architecture audit
的默认门禁顺序不会被绕过。每个步骤使用独立日志文件，主线程按稳定计划顺序合并结果和打印输出，
因此完成时间先后不会改变 CLI 或报告顺序。取消、超时和失败会阻止后续依赖节点，并清理已启动的进程
与服务。

当依赖失败导致节点未执行时，JSON 报告会额外写入 `skipped_steps`，其中包含节点
`id`、`label` 和阻塞 `reason`；成功报告会省略空数组以保持原有字段形状。Markdown
报告使用 `SKIPPED` 行，JUnit 报告使用 `<skipped>` testcase。被跳过的节点不会被计为
已执行失败，但会使整个验证结果保持失败。

## 6.1 `[[steps]].runner`

步骤可以声明一个版本化的测试 runner 契约。省略 `runner` 时保持旧版步骤行为；声明后，
runner 的有效参数和线程环境会写入该步骤的机器结果摘要：

```toml
[[steps]]
id = "backend.tests"
label = "backend tests"
component = "backend"
profiles = ["full"]
program = "cargo"
args = ["test", "--manifest-path", "backend/Cargo.toml"]
cwd = "{root}"
log = "backend_tests.log"
timeout_secs = 300

runner = { version = 1, kind = "cargo-test", threads = 4,
           result_format = "junit", isolation = "schema-per-worker" }
```

| 字段 | 必需 | 说明 |
| --- | --- | --- |
| `version` | 是 | runner 契约版本，当前为 `1` |
| `kind` | 是 | runner 类型 ID；`cargo-test` 要求 `program = "cargo"` |
| `threads` | 否 | runner worker 数，范围 1 到 256；`cargo-test` 会生成 `-- --test-threads N` |
| `threads_env` | 否 | 将线程数注入子进程的环境变量；设置时必须同时设置 `threads` |
| `args` | 否 | runner 专用参数 |
| `args_position` | 否 | 将 `args` 插入原始步骤参数的索引；省略时追加 |
| `result_format` | 否 | `regex`（默认）、`junit`、`trx` 或 `json`；标准结果解析契约仍在逐步建设 |
| `isolation` | 是 | `shared`、`schema-per-worker` 或 `database-per-worker` |

`threads > 1` 时不能使用 `shared` 隔离；非 `cargo-test` runner 还必须声明 `threads_env`，
否则 `config check` 会在启动 service 或 worker 前失败。schema/database runner 会为每次
invocation 生成唯一 worker ID，并通过 `HARNESS_GATE_ISOLATION_*` 环境变量传入；执行结束
会写入 terminal marker 并清理 active state。启用全局并行时，所有外部步骤也必须声明 runner
隔离，避免隐式共享资源。不要通过字符串拼接或未声明的环境变量修改测试线程参数。

## 7. `[policy]`

```toml
[policy]
required_steps = ["api.format", "api.clippy", "api.tests"]
```

`required_steps` 中的每个 ID 都必须出现在 `[[steps]]` 中，而且不能重复。这用于防止项目基础门禁被误删。它不规定步骤属于哪个 profile；profile 仍由步骤自身配置。

### 7.1 Waiver、重试和分片

例外必须显式记录责任、审批和补偿控制，并且有 RFC3339 创建/到期时间：

```toml
[[policy.waivers]]
id = "incident-123"
step = "api.tests"
scope = "api"
risk = "medium"
reason = "upstream fixture unavailable"
owner = "team-api"
approved_by = "security-reviewer"
created_at = "2026-08-30T00:00:00Z"
expires_at = "2026-09-02T00:00:00Z"
compensating_control = "manual smoke test attached to incident"
```

过期、撤销、越权或自批准 waiver 在执行前失败；有效 waiver 在机器结果中标记为 `WAIVED`，
不会伪装成普通 `PASS`。重试只允许配置的失败类别且最多 5 次：

```toml
[execution.retries.api.tests]
max_attempts = 3
backoff_ms = 250
retryable = ["timeout", "exit"]

[execution.shards.api.tests]
index = 0
total = 2
```

结果会记录每次 attempt、`retry_count`、flaky 标记、parser 完整性和分片 merge identity；
标准 parser 支持 `junit`、`trx`、`json`，解析错误、零结果和部分结果分别失败。

## 8. `[[doctor.checks]]`

每项检查都有公共字段：

```toml
[[doctor.checks]]
id = "tool.git"
label = "git"
required = true
timeout_secs = 15
help = "install Git and ensure it is on PATH"
kind = "command"
program = "git"
args = ["--version"]
```

| 字段           | 默认值 | 说明                                    |
| -------------- | ------ | --------------------------------------- |
| `id`           | 无     | 唯一检查 ID                             |
| `label`        | 无     | 终端和 JSON 报告中的显示名称            |
| `required`     | `true` | `true` 失败计为 FAIL，`false` 计为 WARN |
| `timeout_secs` | `15`   | 单项检查硬超时，范围 1 到 300 秒        |
| `help`         | 无     | 失败时追加的修复提示                    |
| `kind`         | 无     | 检查类型                                |

支持的 kind：

| kind          | 字段                                     | 行为                                           |
| ------------- | ---------------------------------------- | ---------------------------------------------- |
| `command`     | `program`, `args`                        | 执行命令并要求退出码为 0                       |
| `path`        | `path`, `path_type`, `path_scope`        | 检查任意路径、文件或目录                       |
| `glob`        | `pattern`                                | 要求 glob 至少命中一个路径                     |
| `env`         | `name`                                   | 要求环境变量存在                               |
| `env-or-file` | `env`, `path`, `contains`, `path_scope`  | 环境变量存在，或文件中有以 `contains` 开头的行 |
| `git-config`  | `key`, `expected`                        | 要求 Git 配置等于预期值                        |
| `git-remotes` | 无                                       | 检查 Git remote 配置                           |
| `version`     | `program`, `args`, `path`, `trim_prefix`, `path_scope` | 比较命令输出与版本文件           |
| `service`     | `service`                                | 检查 service 的外部变量或 Docker 可用性        |

`path_scope` 取 `repository`（默认）或 `host`。repository 作用域的路径必须留在项目内：
仓库相对路径，或以一个 `{root}`/别名占位符开头，不能含 `..`，也不能经符号链接解析到仓库外；
`config check` 和 `doctor` 都会拒绝其他路径。只有确实要检查宿主位置（已安装工具、用户级凭据
文件）时才设置 `path_scope = "host"`；报告只给出路径，不回显文件内容。

示例：

```toml
[[doctor.checks]]
id = "node.version"
label = "Node version"
kind = "version"
program = "node"
args = ["--version"]
path = "{root}/.node-version"
trim_prefix = "v"

[[doctor.checks]]
id = "frontend.dependencies"
label = "frontend dependencies"
kind = "path"
path = "{web}/node_modules"
path_type = "directory"
help = "run `cd apps/web && npm ci`"

[[doctor.checks]]
id = "test.database"
label = "test database"
required = false
kind = "service"
service = "test-postgres"
```

`path_type` 可取 `any`、`file`、`directory`，默认 `any`。命令、Git 配置、remote 和 service 探测都受 `timeout_secs` 约束，超时时会终止整个子进程组。CI 中通常使用 `harness-gate doctor --strict`，把 WARN 也视为失败。

### 8.1 资源租约和孤儿回收

管理型 Docker/Podman service 和 invocation 报告目录会在
`<reports>/leases/` 写入带 `owner_marker = "harness-gate"` 的 JSON 租约。
当前 lease schema 为 `2`，包含 `project_identity`、稳定 `resource_id`、
`resource_kind`、`invocation_id`、PID、进程启动身份、创建/心跳/过期时间；容器
租约还记录 runtime、容器名、完整 runtime labels 和创建后读取的不可变
`runtime_object_id`。租约文件名是 resource ID 的确定性键；旧 schema、改名、
缺字段或跨项目记录只会产生 cleanup failure，不会被回收。

租约只是资源声明，不是删除授权。停止或回收容器前会重新 inspect，并比较文件名、
项目、资源、invocation、所有权 labels 和不可变对象 ID；inspect 失败、label 不全、
名称或 ID 不一致时保留容器和租约。过期只说明可以尝试回收，不能替代 ownership proof。

```bash
# 只观察，不停止容器或删除租约
harness-gate cleanup --dry-run --json

# 回收已死亡或已过期的 Harness-Gate 租约
harness-gate cleanup --json
```

每次清理都会在报告根目录生成 `cleanup.json`（cleanup report schema 仍为 `1`）。
`dry-run` 只列出带合法 Harness-Gate marker 的记录；未知 marker、损坏记录和活动
owner 永不回收。容器停止失败会保留租约并返回非零状态，便于 CI 重试或人工处置。

## 9. `[services.*]`

### 8.1 Environment service

适合由 CI、开发机或密钥管理系统提供现成连接值：

```toml
[services.test-redis]
kind = "environment"
source_env = "CI_REDIS_URL"
inject_env = "TEST_REDIS_URL"
```

步骤启动前读取 `source_env`，并以 `inject_env` 注入子进程。变量不存在时步骤失败。

### 8.2 Docker-compatible container service

```toml
[services.test-postgres]
kind = "docker"
runtime = "docker" # docker | podman
image = "postgres:16-alpine"
image_env = "ARC_FLOW_POSTGRES_IMAGE"
external_env = "TEST_DATABASE_URL"
inject_env = "TEST_DATABASE_URL"
external_value_policy = "isolated-postgres"
startup_timeout_secs = 30
timeout_env = "ARC_FLOW_DATABASE_TIMEOUT_SECS"
container_port = 5432
environment = { POSTGRES_USER = "test", POSTGRES_PASSWORD = "test", POSTGRES_DB = "app_test" }
healthcheck = ["pg_isready", "-U", "test", "-d", "app_test"]
connection = "postgres://test:test@127.0.0.1:{host_port}/app_test"
```

| 字段                   | 必需 | 说明                                           |
| ---------------------- | ---- | ---------------------------------------------- |
| `runtime`              | 否   | 容器 CLI，`docker`（默认）或兼容的 `podman` |
| `image`                | 是   | 本机已有的 OCI 镜像；运行时使用 `--pull=never` |
| `image_env`            | 否   | 覆盖镜像名                                     |
| `external_env`         | 否   | 若该变量已设置，直接使用其值并跳过 Docker      |
| `inject_env`           | 是   | 注入测试步骤的变量名                           |
| `external_value_policy` | 否  | 外部值安全策略；测试 PostgreSQL 使用 `isolated-postgres` |
| `startup_timeout_secs` | 是   | 整个 Docker 启动过程的秒数，范围 1 到 300      |
| `timeout_env`          | 否   | 覆盖启动超时                                   |
| `container_port`       | 是   | 容器监听端口；宿主端口随机绑定到 `127.0.0.1`   |
| `environment`          | 否   | 传给容器的环境变量                             |
| `healthcheck`          | 是   | `docker exec` 后的参数列表，不能为空           |
| `connection`           | 是   | 注入值，必须包含 `{host_port}`                 |

服务按需启动，同一轮验证内复用。选定 runtime 的 daemon 探测、容器创建、端口查询和健康检查共享一个启动截止时间；验证成功、失败、超时或收到中断信号时都会在独立清理超时内尝试 `<runtime> rm --force`。镜像不会自动拉取，先用 `<runtime> pull <image>` 准备。清理错误会被收集到验证结果并使验证失败，避免把容器泄漏报告为成功。

`isolated-postgres` 会要求 URL 使用 `postgres`/`postgresql` 协议，数据库名以 `_test` 或
`-test` 结尾，并拒绝与当前 `DATABASE_URL` 指向同一数据库。默认只允许本机回环地址；确需
使用已确认隔离的远程测试库时，额外设置 `ARC_FLOW_ALLOW_REMOTE_TEST_DATABASE=1`。

一个步骤可以依赖多个服务：

```toml
services = ["test-postgres", "test-redis"]
remove_env = ["DATABASE_URL", "REDIS_URL"]
```

每个 service 必须注入不同变量；`remove_env` 也不能删除 service 正在注入的变量。

跨步骤预检同样会在任何执行前运行：没有直接或传递 `depends_on` 顺序的两个步骤，不能引用同一
service，也不能引用两个向子进程注入相同 `inject_env` 的 service。为同一 service 建立明确
顺序时，将后续步骤依赖于前一个步骤：

```toml
[[steps]]
id = "api.integration"
# ...
services = ["test-postgres"]
depends_on = ["api.setup"]
```

不要依赖 profile、当前串行执行或偶然的启动顺序来规避这项规则；它们不能证明未来并行运行
时互斥。校验器不会自动加入依赖或重命名变量。

## 10. `[parsers.*]`

解析器用于防止命令退出码为 0、实际却没有执行任何测试：

```toml
[parsers.rust]
kind = "regex"
patterns = ['(?m)^running ([0-9]+) tests?$']
capture = 1
minimum = 1
```

| 字段       | 默认值 | 说明                          |
| ---------- | ------ | ----------------------------- |
| `kind`     | 无     | 支持 `regex`、`junit`、`trx`、`json` |
| `patterns` | 无     | regex 模式下的一个或多个 Rust regex |
| `capture`  | `1`    | 包含数值的 capture group 索引 |
| `minimum`  | `1`    | 所有匹配计数之和的最低值      |

每个正则都必须包含对应的 capture group。步骤成功后才解析日志；计数低于 `minimum` 时，该步骤改判为失败，
并按 `RESULT_ZERO` 或 `RESULT_PARTIAL` 记录。JUnit 只接受单个 `testsuite` 或 `testsuites` 根元素，
TRX 只接受单个 `TestRun` 根元素；命名空间前缀按 local name 识别。缺根、多根、根元素外非空内容以及
其他 XML/JSON malformed 输入均记录 `RESULT_PARSE_FAILURE`。

JSON 的 `count_path` 是非空的点分对象路径，只能指向数组或非负整数；不支持
JSONPath、数组索引或通配符。例如：

```toml
[parsers.json-results]
kind = "json"
count_path = "summary.total"
minimum = 1
```

未配置 `count_path` 时，为兼容现有 producer，自动发现只接受裸顶层数组，
或对象（可有对象包装层）中恰好一个受支持字段的直接数组。字段名区分大小写，
仅包括 `testcases`、`testCases`、`test_results`、`testResults` 和 `results`。
不会进入结果数组元素继续查找。

以下旧的宽松输入现在明确拒绝并记录 `RESULT_PARSE_FAILURE`：只有
`duration_ms` 等任意数字、metadata/attachments 等无关数组、受支持字段却不是
数组、以及多个候选字段（即使数组长度相同）。显式 `count_path` 缺失、指向
负数/小数/布尔值/null/对象，或路径无效时也只会解析失败，不会回退自动发现。
因此，依赖旧隐式数字计数的 producer 必须声明真实字段路径；合法数组和数字仍
按 `minimum` 产生 `RESULT_ZERO`、`RESULT_PARTIAL` 或成功结果。

## 11. `[scope]` 和 `[[scope.rules]]`

```toml
[scope]
unmatched = "fail"
```

| `unmatched` 值 | 行为                                                       |
| -------------- | ---------------------------------------------------------- |
| `fail`         | 默认值；列出未命中的变更路径并失败                         |
| `all`          | 任一路径未命中时选择全部 component，适合通用或未知结构项目 |
| `ignore`       | 忽略未命中路径，仅适合已明确评估漏测风险的项目             |

`--all` 是显式全量模式，不读取工作区路径，因此不应用 `unmatched` 策略。

```toml
[[scope.rules]]
patterns = ["services/api/**", "shared/contracts/**"]
components = ["api"]

[[scope.rules]]
patterns = [".harness-gate/**", ".github/workflows/**"]
components = ["api", "web", "workflow"]
```

每条规则使用 glob 匹配仓库相对路径，命中后把所有 `components` 加入集合。规则可以重叠，最终 component 去重。每个 component 必须至少有一个步骤。`scope` 的文本和 JSON 输出都包含未匹配文件，便于排查规则覆盖缺口。

建议把共享契约、工作流配置和 CI 文件映射到所有受影响组件，避免只验证单端。

## 12. `[[steps]]`

```toml
[[steps]]
id = "api.tests"
label = "API tests"
component = "api"
profiles = ["full"]
program = "cargo"
args = ["test", "--manifest-path", "{api}/Cargo.toml", "--", "--nocapture"]
cwd = "{root}"
log = "api_tests.log"
timeout_secs = 300
timeout_env = "API_TEST_TIMEOUT"
parser = "rust"
services = ["test-postgres"]
remove_env = ["DATABASE_URL"]
```

Built-in safety gates may be declared explicitly with a closed `gate_type`
vocabulary. The current values are `secret-scan` and `architecture-audit`:

```toml
[[steps]]
kind = "builtin-gate"
id = "builtin.secret-scan"
label = "secret scan"
gate_type = "secret-scan"
profiles = ["full"]
```

When `kind` is omitted, the entry remains an external step for compatibility.
Built-in entries must use their reserved IDs and may not specify external-step
fields such as `program`, `args`, `services`, or `log`. Unknown gate types fail
closed before any command or service starts. If no built-in entries are
declared, verification synthesizes the legacy `secret scan -> architecture
audit -> external steps` chain internally without rewriting `flow.toml`.

| 字段           | 必需 | 说明                               |
| -------------- | ---- | ---------------------------------- |
| `id`           | 是   | 全局唯一步骤 ID                    |
| `label`        | 是   | 终端和报告显示名称                 |
| `component`    | 是   | 变更范围选择单位                   |
| `profiles`     | 是   | 该步骤参与的 profile，至少一个     |
| `program`      | 是   | PATH 中的裸命令名                  |
| `args`         | 是   | 独立参数数组，可使用路径占位符     |
| `cwd`          | 是   | 单个 `{root}` 或 path alias 占位符 |
| `log`          | 是   | 报告目录下的单个 `.log` 文件名     |
| `timeout_secs` | 是   | 运行超时，范围 1 到 3600 秒        |
| `timeout_env`  | 否   | 覆盖运行超时的环境变量             |
| `parser`       | 否   | 成功后使用的 parser ID             |
| `services`     | 否   | 运行前需要准备的 service ID 列表   |
| `remove_env`   | 否   | 创建子进程前删除的继承环境变量     |
| `depends_on`   | 否   | 必须先完成的 step ID；支持传递依赖并决定资源是否可并发 |
| `input`        | 否   | `snapshot`（默认）或显式兼容能力 `repository` |
| `kind`         | 否   | `builtin-gate` 或省略（省略表示外部步骤） |
| `gate_type`    | 否   | 内置门禁类型：`secret-scan` 或 `architecture-audit` |

命令直接通过 `program + args[]` 启动，不执行 shell 拼接。`sh -c`、`bash -lc` 等命令字符串会被配置校验拒绝；管道、重定向和条件逻辑应拆成多个步骤，或封装成项目内受版本控制的可执行程序。

步骤选择条件是：component 已被 scope 选中，并且步骤包含当前 profile。配置顺序就是执行顺序；任一步失败后，报告判为失败，但仍继续执行不依赖该故障 service 的后续步骤。同一 service 启动失败会被缓存，依赖它的步骤快速失败，不会反复等待启动超时。

## 13. Secret Scan 规则文件

`[paths].secrets_config` 指向独立、受版本控制的 TOML 文件。预设会生成一套通用高置信规则，项目可在不重新编译 `harness-gate` 的情况下增加供应商或业务密钥规则。配置版本、规则 ID、正则、捕获组和最小长度都会在扫描前校验；配置缺失、空规则或无效捕获组会直接让门禁失败。

```toml
version = 2

[placeholders]
minimum_unique_characters = 4
maximum_nonalphanumeric_characters = 2
prefixes = ["${", "{{", "<"]
markers = ["change-me", "replace-me", "placeholder"]
exact = ["password", "secret"]

[[rules]]
id = "named-signing-secret"
kind = "value"
pattern = '''(?i)signing_secret\s*=\s*([A-Za-z0-9_-]{12,})'''
capture = 1
minimum_length = 12
```

规则类型：

- `direct`：正则命中即报告，适合有固定前缀或结构的 Token、JWT、私钥头。
- `value`：只对指定捕获组执行长度、占位符和字符多样性判断，适合命名密钥及厂商 Webhook Token。
- `postgres-url`：分别捕获用户名、密码、主机和数据库；`local_test_policy` 可显式配置临时数据库允许的主机、库名后缀及用户名密码约束。
- `webhook-url`：解析捕获到的 URL，检查配置的敏感查询参数或高信息路径末段。

扫描报告只包含命中文件名，不会复制密钥内容。占位符策略只作用于捕获值；`direct` 应仅配置误报概率足够低的模式。为限制内存和单次扫描成本，working tree 与 staged snapshot 中超过 16 MiB 的单个文件会直接使扫描失败；符号链接和非普通文件会跳过。

## 14. 审计规则文件

`[paths].audit_config` 指向独立 TOML 文件。空规则文件可写为：

```toml
version = 2

[engine]
ignore_filename = ".auditignore"
json_report_filename = "review_context.json"
markdown_report_filename = "review_context.md"
markdown_max_bytes = 4096
markdown_occurrences_per_rule = 3

[engine.comment_syntax.rs]
line = ["//"]
block = [{ start = "/*", end = "*/", nested = true }]
strings = [
  { start = 'r###"', end = '"###' },
  { start = 'r##"', end = '"##' },
  { start = 'r#"', end = '"#' },
  { start = 'r"', end = '"' },
  { start = '"', end = '"', escape = '\' },
]

[paths]
exclude = ["target", "node_modules", "dist", ".git"]
```

内置空 preset 还预置 `sql`、`ts`、`tsx`、`js`、`jsx`、`toml`、`yaml` 和 `yml` 的注释与字符串定界符。每条规则使用的扩展名都必须存在对应的 `[engine.comment_syntax.<扩展名>]`；缺少时 auditor 会拒绝运行，避免把注释示例当成真实代码。

<a id="audit-v2-migration"></a>

### Audit v2 迁移

audit v2 是 `harness-gate` 3.0.0 的破坏性配置升级。旧配置不会被静默套用新语义：缺少 `version`、缺少 `[engine]`、未知版本、未知字段或字符串 allowlist 都会 fail closed，并在错误中指向本节。

旧配置可能依赖隐式 engine 默认值，并让字符串内容同时承担路径和正则语义：

```toml
[[hard_rules]]
name = "SQL writes stay in repositories"
severity = "blocker"
paths = ["api"]
extensions = ["rs"]
patterns = ['(?i)INSERT\s+INTO']
allowlist = ["services/api/src/repositories", "^services/api/generated/.*\.rs$"]
```

迁移时先加入 `version = 2`，从当前 `empty.audit.toml` 复制完整 `[engine]` 和所需扩展名的 `comment_syntax`，再逐项明确 allowlist 类型：

```toml
version = 2

[engine]
ignore_filename = ".auditignore"
json_report_filename = "review_context.json"
markdown_report_filename = "review_context.md"
markdown_max_bytes = 4096
markdown_occurrences_per_rule = 3

[engine.comment_syntax.rs]
line = ["//"]
block = [{ start = "/*", end = "*/", nested = true }]
strings = [
  { start = 'r###"', end = '"###' },
  { start = 'r##"', end = '"##' },
  { start = 'r#"', end = '"#' },
  { start = 'r"', end = '"' },
  { start = '"', end = '"', escape = '\' },
]

[[hard_rules]]
name = "SQL writes stay in repositories"
severity = "blocker"
paths = ["api"]
extensions = ["rs"]
patterns = ['(?i)INSERT\s+INTO']
allowlist = [
  { kind = "path-prefix", path = "services/api/src/repositories" },
  { kind = "regex", pattern = '^services/api/generated/.*\.rs$' },
]
```

字符串 allowlist 无法可靠推断原意，因此不自动迁移。完成转换后运行 `harness-gate config check` 和 `harness-gate audit`，确认路径引用、正则和报告配置有效。

### 13.1 Hard rule

```toml
[[hard_rules]]
name = "SQL writes stay in repositories"
severity = "blocker"
paths = ["api"]
extensions = ["rs", "sql"]
patterns = ['(?i)INSERT\s+INTO', '\.execute\s*\(']
allowlist = [
  { kind = "path-prefix", path = "services/api/src/repositories" },
  { kind = "path-prefix", path = "services/api/migrations" },
  { kind = "path-prefix", path = "services/api/tests" },
]
exclude_patterns = []
```

`paths` 可以引用审计文件 `[paths]` 中的 alias。`allowlist` 必须显式使用 `path-prefix` 或 `regex` 类型，避免根据字符串内容猜测语义。`exclude_patterns` 用正则排除文件路径。

### 13.2 Architecture rule

```toml
[[arch_rules]]
name = "handlers do not query SQL"
layer = "handler"
paths = ["services/api/src/handlers"]
extensions = ["rs"]
forbidden_patterns = ['sqlx::(query|query_as|query_scalar)!?\\s*\\(']
allowed_patterns = []
suggestion = "move SQL into a repository"
allowlist = []
exclude_patterns = []
```

`allowed_patterns` 匹配违规起始行，适合明确的 trait impl 或框架样板；不要用过宽正则隐藏真实违规。`exclude_patterns` 与 hard rule 一样匹配文件路径。任意审计违规都会阻止后续外部步骤。

每个规则的 `paths` 必须解析到项目根内已经存在的目录。路径中的 `..`、逃出项目的绝对路径或符号链接都会被拒绝；目录遍历或文件读取失败也会让审计失败，避免扫描缺失时误报通过。单个审计源文件超过 16 MiB 时同样会拒绝读取并使审计失败。审计报告统一记录仓库相对路径。

auditor 是确定性的整文件正则扫描器，不是语言 parser。正则默认启用 multi-line 模式，因此 `^`/`$` 仍按代码行匹配，`\s` 可以跨行；需要让 `.` 跨行时应在规则中显式使用 `(?s)`。报告定位到匹配起始行。`[engine.comment_syntax.<扩展名>]` 可配置行注释、块注释和字符串定界符；扫描器会跟踪这些词法状态，避免把字符串中的注释标记当成真实注释。需要抽象语法树级判断时，应使用项目语言自己的 lint 工具，并把该工具配置成一个 step。

## 15. 最小完整示例

```toml
version = 2

[project]
name = "example-api"
default_profile = "full"
hook_profile = "hook"

[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"

[paths.aliases.app]
path = "."

[policy]
required_steps = ["app.format", "app.tests"]

[scope]
unmatched = "all"

[[doctor.checks]]
id = "tool.cargo"
label = "cargo"
kind = "command"
program = "cargo"
args = ["--version"]

[parsers.rust]
kind = "regex"
patterns = ['(?m)^running ([0-9]+) tests?$']
capture = 1
minimum = 1

[[scope.rules]]
patterns = ["**"]
components = ["app"]

[[steps]]
id = "app.format"
label = "Rust format"
component = "app"
profiles = ["full", "hook"]
program = "cargo"
args = ["fmt", "--", "--check"]
cwd = "{app}"
log = "rust_fmt.log"
timeout_secs = 120

[[steps]]
id = "app.tests"
label = "Rust tests"
component = "app"
profiles = ["full"]
program = "cargo"
args = ["test", "--", "--nocapture"]
cwd = "{app}"
log = "rust_tests.log"
timeout_secs = 300
parser = "rust"
```

完成配置后依次执行：

```bash
harness-gate config check
harness-gate doctor
harness-gate scope --all
harness-gate verify --all
```

### 步骤日志唯一性

每个 `log` 都必须是唯一的单个 `.log` 文件名。即使两个步骤已有 `depends_on` 顺序，也不能复用
同一日志：串行复用同样会覆盖报告证据。错误诊断会同时指出两个 `steps[*].log` 字段。

### 报告模板、JUnit 和通知

HTML 模板和 JUnit 输出是可选的。每次验证的规范证据写入
`<reports>/invocations/<invocation_id>/`，包括 `invocation.json`、`scope.json`、步骤日志和报告；
为兼容现有消费者，`test_result.json` 与 `test_result.md` 仍会镜像到报告根目录：

```toml
[report_templates]
root = "templates/harness-gate"
template = "templates/harness-gate/verification.tera"
junit = "junit.xml"
```

`root` 和 `template` 必须同时存在；二者必须是仓库内路径，不能是绝对路径、Windows 前缀、
`..` 跳转或 NUL。模板根必须存在且为目录，模板必须存在且为 `.html` 或 `.tera` 普通文件；
解析符号链接后仍必须在仓库和模板根内。模板根不能等于、包含或被 `paths.reports` 包含。
模板路径相对于仓库根，模板支持 `{{ timestamp }}`、`{{ profile }}`、`{{ summary }}` 和
`{{ components }}` 替换，也可使用 Tera 的 `include`、`extends` 和 `block`。模板上下文同时提供
完整的 `report` 对象及其 `timestamp`、`profile`、`scope`、`steps` 和 `passed` 字段。输出固定为报告目录中的 `test_result.html`。`junit` 路径相对于报告目录，
必须是报告目录内的 `.xml` 文件名或子路径；运行时还会检查现有目录和符号链接不会把输出重定向到目录外。
模板输入仍是只读且受路径隔离校验。
规范报告和日志先写入临时同级文件，再通过原子 rename 发布；失败会使本次验证返回报告错误。

`test_result.json` 使用仓库中的 [machine-result schema](../schema/machine-result.schema.json)（当前版本 `1`）。
顶层和步骤状态使用 `PASS`、`FAIL`、`SKIPPED`、`CANCELLED` 等稳定值，并提供 attempts、结构化 failures、
invocation-relative artifacts 及 `evidence_complete`；集成方不应从 Markdown 或日志内容推断门禁状态。

每次 invocation 还会生成 `artifact-registry.json` 和 `manifest.json`。registry 使用
[artifact registry schema](../schema/artifact-registry.schema.json)，manifest 使用
[artifact manifest schema](../schema/artifact-manifest.schema.json)。两者都只接受当前
invocation 内的普通文件，并记录相对路径、kind、字节数和 SHA-256；步骤日志还绑定
invocation 和 step ID。报告声明、registry、manifest 和磁盘上的可发布文件必须组成同一闭集；
缺失、未声明、越界、符号链接、替换或摘要不一致都会使报告发布失败并保持
`evidence_complete = false`。控制文件先完成校验，最终 machine result 在 manifest 之后发布，
作为完整证据的提交标记。

日志、报告和通知中的文本会统一脱敏，包含授权/Cookie 请求头、Bearer/Basic 凭据、API key、密码、
私钥块及常见数据库连接串的值都不会导出。默认保留最新 50 个 invocation；仅清理超过 15 分钟租约窗口
且超出数量上限的旧目录，活动或最近修改的 invocation 不会被清理。

日志或 JSON lines 内的 JSON 凭据字符串也会完整脱敏，包含转义引号、连续反斜杠和
Unicode 转义。私钥 BEGIN 块缺少 END 时脱敏至文件尾。请求头可位于缩进或明确的日志前缀之后：
以空白分隔的方括号字段、ISO 日期时间、TRACE/DEBUG/INFO/WARN/WARNING/ERROR/FATAL
级别（可带冒号）以及 curl 的 `<`/`>` 方向标记。保留前缀，脱敏完整 header 值及 Cookie 属性；
任意正文中出现 `cookie:` 不构成 header 边界。现有证据大小限制、Core 生成 JSON 的内容身份校验
以及 Engineering Policy 的 required quality gate 语义保持不变。

发布流程先生成一个由 `tools/release/release_inventory.py` 管理的显式
`release-inventory.json`，其中列出每个平台二进制和 CycloneDX SBOM。checksum、Sigstore
签名/证书、GitHub provenance 验证和最终上传集合都从同一 inventory 派生；inventory、
`SHA256SUMS`、二进制、SBOM 以及对应 `.sig`/`.crt` 的集合必须完全一致，缺失或额外文件会在
创建 release 前失败。离线升级时应先运行 `sha256sum --check SHA256SUMS`，再按 README 中的
`cosign verify-blob` 命令校验二进制和 SBOM；任意字节被修改都会导致摘要或签名校验失败。
发布资产不覆盖旧文件，替换必须使用新的版本 tag。

### 容器运行时和 Webhook

Docker service 可显式选择 Docker 兼容的 Podman CLI，省略时默认为 Docker：

```toml
[services.postgres]
kind = "docker"
runtime = "podman" # docker | podman
```

Webhook 在报告写入成功后发送报告 JSON。仅启用的结果类型会发送；URL 主机必须列入精确的
`allowed_hosts`，不能包含用户名、密码或通配符：

```toml
[[notifications.webhooks]]
url = "https://hooks.example.test/events"
allowed_hosts = ["hooks.example.test"]
on_failure = true
on_success = false
```

Webhook 只支持 `http` 和 `https`，发送前会再次解析并拒绝 loopback、RFC1918/private、link-local、
unspecified、multicast、共享地址（CGNAT 100.64.0.0/10）、基准测试、文档及保留地址，并对内嵌
IPv4 的 IPv6 形式（IPv4-mapped/compatible、NAT64 `64:ff9b::/96`、6to4）按内嵌地址判定；重定向被禁用。非 2xx 响应、连接错误或目的地策略拒绝会使本次验证
返回报告错误（`E1404`），但不会改写已经生成的报告。`on_failure` 默认开启，`on_success` 默认关闭；
至少启用一个结果类型。多个 webhook 按配置文件中的顺序发送；第一个请求失败后立即停止，后续 endpoint
不会被调用。日志只保留 scheme 和规范化主机，不记录凭据或报告 body。

### v1 到 v2 配置迁移和安全修复

v1 配置使用迁移命令生成 v2 文件，源文件不会被删除：

```bash
harness-gate --project-root /path/to/project config migrate \
  --input legacy.flow.toml \
  --output .harness-gate/flow.toml
harness-gate --project-root /path/to/project config check
```

v2 版本号保持为 `2`，不会自动升级。若原本可加载的 v2 文件因为服务、注入变量或日志关系被
新预检拒绝，请根据 `config check` 的主字段、related 字段和 `help:` 修复：添加显式的
`depends_on`、使用不同的 `inject_env` 或 service、或为每个步骤指定独立日志。工具绝不会
静默重排、插入依赖或改写配置。

## 16. 进程外 Adapter 请求（P2，显式 opt-in）

签名 adapter 不是 `flow.toml` 中的步骤类型，也不会改变内置 runner 的行为。它通过一份
独立 JSON 请求交给 `harness-gate adapter run` 执行；只有调用方显式提供请求、受信任公钥和
能力 allowlist 时才会启动。协议和结果约束见
[ADR-0033](adr/0033-signed-out-of-process-adapter-protocol.md) 及
[Adapter OpenSpec](../openspec/changes/harness-gate-adapter-protocol/proposal.md)。
请求字段的机器校验见
[adapter-request.schema.json](../schema/adapter-request.schema.json)。

最小请求形状如下（协议 v2；`signature.value` 是对下方完整规范化请求的
Ed25519 签名，均使用 base64）：

```json
{
  "protocol_version": 2,
  "result_schema_version": "1",
  "adapter": {
    "name": "org-scanner",
    "version": "1.0.0",
    "executable": "adapters/org-scanner",
    "source_digest": "<64 lowercase hex SHA-256>",
    "signature": {
      "algorithm": "ed25519",
      "key_id": "release-2026",
      "value": "<base64 signature>"
    }
  },
  "invocation_id": "inv-2026-08-31-0001",
  "step_id": "security.org-scanner",
  "timeout_ms": 120000,
  "config_digest": "<configuration digest>",
  "artifact_root": ".harness-gate/reports/invocations/inv-2026-08-31-0001",
  "nonce": "<single-use random value>",
  "issued_at_ms": 1788134400000,
  "expires_at_ms": 1788134520000,
  "args": [],
  "environment": {},
  "capabilities": {
    "network": [],
    "resources": [],
    "environment": []
  },
  "input": {}
}
```

签名者必须对紧凑 UTF-8 JSON 载荷签名，而不是对请求文件中的
`signature.value` 签名。载荷键顺序固定为
`domain`、`protocol_version`、`result_schema_version`、`adapter`、
`invocation_id`、`step_id`、`timeout_ms`、`config_digest`、`artifact_root`、
`nonce`、`issued_at_ms`、`expires_at_ms`、`args`、`environment`、
`capabilities`、`input`；`domain` 固定为
`harness-gate/adapter-request/v2`。`adapter` 内顺序为
`name`、`version`、`executable`、`source_digest`、`signature`，其中
`signature` 只包含 `algorithm` 和 `key_id`，不包含签名值。集合按 JSON
规范化后的键序列化（环境变量使用字典序），再将 Ed25519 结果 base64
写入请求的 `signature.value`。

每个受信任公钥是一个独立 JSON 文件：

```json
{"key_id":"release-2026","public_key":"<base64 32-byte Ed25519 key>"}
```

执行命令和能力选项：

```bash
harness-gate adapter run \
  --request adapter-request.json \
  --trusted-key release-2026.json \
  --allow-network internal-api \
  --allow-resource test-database \
  --allow-environment ORG_SCANNER_MODE
```

`--trusted-key`、`--allow-network`、`--allow-resource` 和 `--allow-environment` 都可以重复。
请求中的能力必须分别出现在对应 allowlist 中；环境变量还必须同时列在
`capabilities.environment`，否则在启动前失败。host 启动 adapter 前会校验协议/结果版本、
可执行文件 SHA-256 和完整请求签名，清空继承环境，仅注入 Harness-Gate 的 invocation 元数据及请求中
声明的环境变量。`nonce` 必须在有效期内且在同一 host policy 下只使用一次；默认有效期最长
1 小时，并允许 30 秒时钟偏差。能力 allowlist 是协议级失败边界，不等同于操作系统级网络、
文件、资源或进程沙箱。

adapter 通过 stdin 接收一份 JSON，请在 stdout 只返回一份 JSON 对象，`schema_version` 必须为
`"1"`，`status` 必须是 `PASS`、`FAIL`、`CANCELLED`、`SKIPPED` 或 `WAIVED` 之一。产物路径必须
是相对于 `artifact_root` 的普通相对路径；绝对路径、Windows 前缀、`..` 越界路径、缺失文件和
符号链接逃逸都会失败。stdout/stderr 默认各限 16 MiB，adapter 产物默认合计限 64 MiB；超限会
保留已读取的部分长度并以 `truncated=true` 的结构化协议失败结束。超时、取消、崩溃、非零退出、
malformed 响应和上述校验错误统一映射为 `ADAPTER_PROTOCOL_FAILURE`，并尝试清理 adapter 的进程组。
reader 在独立 deadline 后不会阻塞调用方。该 host 当前是独立执行入口；若要把 adapter 纳入验证
DAG，应先在上层编排器中显式安排 invocation 和结果映射。
