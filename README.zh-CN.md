# GateHouse

> 开发仓库已迁移到 **GateHouse**，命令和 crate 名仍为 `harness-gate`。v0.4.8 及以前的发行版保留旧仓库下载与签名身份。见[迁移与测试数据说明](docs/gatehouse-migration.md)。

[![CI](https://github.com/musutrade/GateHouse/actions/workflows/ci.yml/badge.svg)](https://github.com/musutrade/GateHouse/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/harness-gate.svg)](https://crates.io/crates/harness-gate)
[![Documentation](https://docs.rs/harness-gate/badge.svg)](https://docs.rs/harness-gate)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[English](https://github.com/musutrade/GateHouse/blob/main/README.md) | [简体中文](https://github.com/musutrade/GateHouse/blob/main/README.zh-CN.md)

`Harness-Gate` 是用 Rust 实现的配置驱动项目验证 CLI。主流程是 **`init -> verify -> one project decision`**：声明执行与质量意图，让安全、架构、命令和证据质量门禁形成一个阻断式项目结论。

它统一负责 changed paths、secret scan、architecture audit、环境体检、外部命令编排、测试结果计数、超时与中断处理，以及临时服务生命周期。Git hook 只保留启动器，流程判断不依赖 Shell 脚本。


普通命令结果复用需要显式启用；输入绑定、Unix 宿主信任、命中解释与安全清理见[确定性步骤本地缓存](docs/step-cache.md)。

## 仓库与发行边界

| 名称 | 职责 |
| --- | --- |
| **GateHouse** | Core 的当前开发仓库，维护配置、schema、执行编排与质量决策。 |
| **`harness-gate`** | 保持不变的命令与 crate 名称，项目配置仍位于 `.harness-gate/`。 |
| **[GateProbes](https://github.com/musutrade/GateProbes)** | 独立交付的采集器与生态预设。 |
| **[Harness-Gate](https://github.com/musutrade/Harness-Gate)** | 历史源码、Issue，以及 Core 0.4.8 及以前的发行资产和签名身份。 |

源码迁移没有发布新的 Core 版本。当前源码与 Core 0.4.9 均支持
外部预设加载；请按下方对应版本的快速开始操作。普通源码构建无需下载
历史证据，完整回归与文档检查使用[固定摘要的可选数据](test-data/README.md)。
完整入口见[文档目录](docs/README.md)与[迁移验收](docs/gatehouse-migration-acceptance.md)。

## 发布状态

Postgres 外置已由正式0.5.0/0.1.1 产物组合验收，见[固定产物回执](docs/releases/postgres-service-0.1.1-acceptance.md)。

**当前 Core 0.5.0 已在 GitHub 与 crates.io 发布并验证。** Postgres 检查和迁移由 GateProbes 承接；旧配置需先迁移。见[升级指南](docs/releases/0.5.0.zh-CN.md)与[公开交付回执](docs/releases/0.5.0-public-verification.md)。

以下保留首个拆分发行版的历史记录。

**Core 0.4.9 已在 GitHub 与 crates.io 发布。** 这是 GateHouse 拆分后的首个核心版本：生态
预设外置，核心发行清单仅包含 Core 二进制与元数据，采集器由 GateProbes 独立
交付。安装见[0.4.9 发布指南](docs/releases/0.4.9.zh-CN.md)，公开验证见
[交付回执](docs/releases/0.4.9-public-verification.md)。

公开 crate 与原 `v0.4.9` 源码一致，并已通过全新 Linux registry 安装验证。
详见[crate 验证回执](docs/releases/0.4.9-crate-public-verification.md)。

本次保留[已接受的 Phase 1 基线](docs/benchmarks/phase-1/README.md)，旧 #346
候选已在证据审查后拒绝采纳。历史 0.4.8 与插件资产保留原有身份；当前安装器
独立选择 GateProbes native **0.1.0-rc.8**。

## 阅读导航

- 快速开始：看[安装](#安装)和[快速开始](#安装与快速开始)；
- 接入新项目：看[安装与快速开始](#安装与快速开始)和[内置预设](#内置预设)；
- 增加命令、组件或 CI profile：看[选择模型](#选择模型)、[schema v2 配置参考（中文）](https://github.com/musutrade/GateHouse/blob/main/docs/configuration.zh-CN.md)和 [JSON Schema 目录（英文）](https://github.com/musutrade/GateHouse/blob/main/schema/README.md)；
- 处理失败：看[验证与报告](#验证与报告)和[故障排查](#故障排查)；
- 扩展 Rust 引擎：看[无需改代码的范围](#无需改代码的范围)、[需要改-rust-的边界](#需要改-rust-的边界)和[项目自有验证边界](#项目自有验证边界)。

## 工作模型

`init` 生成配置，`verify` 按 profile 和 scope 执行门禁与采集器，由 Rust 核心评估质量策略，再合并为一个项目结论。执行配置放在 `flow.toml`，质量意图放在 `quality.toml`。详见[质量工作流与架构](docs/quality-workflow.md)。

生态、语言和框架名称是配置/pack 数据。新增语言名称不需要修改通用 schema、compiler、orchestrator、baseline、profile、verify、report 或 CI 层；扩展路径为 **collector + capability/policy pack + certification**。新的度量语义仍需单独评审，不能由名称或预设自动获得认证。

执行流程使用四类数据：

1. **scope rule**：把 Git 变更路径映射成 component；
2. **profile**：从同一 component 中选择不同强度的步骤，例如 `hook`、`full`、`ci`；
3. **step**：声明一个 `program + args[]` 外部命令、超时、日志、parser 和 service 依赖；
4. **gate**：固定先运行 secret scan 和 audit，成功后才允许外部步骤执行。

运行 `verify` 时的数据流：

```text
Git 变更文件
  -> scope.rules
  -> components
  -> component + profile 匹配的 steps
  -> secret scan
  -> architecture audit
  -> 按配置顺序执行 steps
  -> 配置的质量采集 + 可信 baseline + Rust 策略评估
  -> 一个项目决策 + JSON / Markdown / 可选 HTML/JUnit 报告
  -> 可选 HTTP(S) Webhook 通知
```

component、profile、命令、路径、parser 和 service 都来自 TOML。常规项目迁移不需要在 Rust 中增加枚举或修改匹配分支。

## 安装

### 从 crates.io 安装

需要 Rust 1.88 或更新版本：

```bash
cargo install harness-gate --version 0.5.0 --locked
```

[发布与安装验证](docs/releases/0.5.0-public-verification.md)记录了
registry 摘要、原 tag 源码和全新 Linux 安装验证。

### 从 GitHub Release 安装（预编译二进制）

从不可变的 [GitHub Release tag](https://github.com/musutrade/GateHouse/releases/tag/v0.5.0) 下载适合你平台的二进制文件：

- **Linux (x86_64)**: `harness-gate-linux-amd64`
- **macOS (Intel)**: `harness-gate-macos-amd64`
- **macOS (Apple Silicon)**: `harness-gate-macos-arm64`
- **Windows (x86_64)**: `harness-gate-windows-amd64.exe`

安装脚本会先校验 checksum 清单和 Sigstore 证书，再原子替换目标文件。请从下面固定的源码
提交下载当前安装脚本，并显式选择 Core 版本：

```bash
curl --fail --show-error --location --proto '=https' --tlsv1.2 \
  -o /tmp/harness-gate-install.sh \
  https://raw.githubusercontent.com/musutrade/GateHouse/v0.5.0/install.sh
bash /tmp/harness-gate-install.sh --version v0.5.0
harness-gate --version
```

默认安装到 `~/.local/bin`，也可以用 `--install-dir` 指定私有目录。脚本不会调用可变的
`releases/latest` API，也不再推荐执行 `raw/main` 安装命令。
缺少 `cosign` 时，安装器会自动下载并校验固定版本的验证工具，也会复用已有缓存；源码安装还需要 `git` 和 Rust `cargo`。

### 可选 Rust 插件

Core 仅安装 Core。GateProbes 负责 native 插件的独立安装、升级与回滚，
使用显式版本和经过摘要校验的固定提交安装器。当前源码安装器拒绝旧插件参数，
并给出迁移诊断。可执行命令及两仓库信任边界见[独立插件指南](docs/quality/standalone-installation.md)。
缺少插件依赖时仅诊断，Rust/LLVM、Python 和 Cosign 由用户自行提供。
安装插件不采纳测量系列或基线，历史标签内的安装器保留原有 pin 与签名身份。

### 从源码安装

```bash
git clone https://github.com/musutrade/GateHouse.git
cd GateHouse
cargo install --locked --path tools/harness-gate
```

## 开发命令

下列命令需要 Rust 1.88+（以 crate 的 `rust-version` 为准）、Git、Python 3
和 `cargo-nextest`。完整测试前先准备固定摘要的回归数据；构建或安装 Core
本身不需要这一步。

```bash
python3 tools/maintenance/test_data.py --list
python3 tools/maintenance/test_data.py
```

仓库使用 `cargo-nextest` 执行快速、隔离的测试：

```bash
cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked
cargo clippy --manifest-path tools/harness-gate/Cargo.toml --locked --all-targets --all-features -- -D warnings
cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check
```

`release` 保留默认的 panic unwind 行为以支持诊断；需要最小分发二进制时使用
`release-small`：

```bash
cargo build --manifest-path tools/harness-gate/Cargo.toml --release
cargo build --manifest-path tools/harness-gate/Cargo.toml --profile release-small
```

worker 在 scheduler 边界由 `catch_unwind` 转换为可发布的失败结果；service 或 lease
锁被 poison 时也会 fail closed，不能被当作可用资源。`release-small` 使用
`panic = "abort"`，调用方不能依赖该二进制的 unwind。稳定的 machine failure 注册表见
[docs/failure-codes.md](docs/failure-codes.md)。

Release 资产同时提供 `SHA256SUMS`、CycloneDX SBOM，以及 Sigstore 签名和证书。
手工验证时下载清单中列出的全部资产及对应的 `.sig`/`.crt` 文件，先校验摘要，再校验
二进制和 SBOM 的签名：

```bash
sha256sum --check SHA256SUMS
cosign verify-blob --signature harness-gate-linux-amd64.sig \
  --certificate harness-gate-linux-amd64.crt \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity 'https://github.com/musutrade/GateHouse/.github/workflows/release.yml@refs/tags/v0.5.0' \
  harness-gate-linux-amd64
cosign verify-blob --signature harness-gate.sbom.cdx.json.sig \
  --certificate harness-gate.sbom.cdx.json.crt \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity 'https://github.com/musutrade/GateHouse/.github/workflows/release.yml@refs/tags/v0.5.0' \
  harness-gate.sbom.cdx.json
```

SBOM 记录源码提交、`Cargo.lock` 摘要和构建工具链。发布流程先生成一个显式的
`release-inventory.json`；checksum、Sigstore 签名/证书、GitHub provenance、校验和上传
都从同一 inventory 派生，并完整覆盖 SBOM。缺失、额外、未签名或未 attested 的资产会在
创建 release 前失败。发布资产不可覆盖；任何替换都必须使用新的版本 tag 和新的 provenance 证明。

## 安装与快速开始

### Core 0.5.0 与当前源码构建

Core 0.5.0 和当前源码构建使用外部生态预设目录。历史 Core 0.4.8 内置预设，
使用该版本时省略 `--presets-dir`。

从源码安装后，选择审阅过的 GateProbes checkout 中的 `presets/` 目录。
Core 仅内置 `generic`，详见[外部预设接入](docs/quality-presets.md)。
验证安装并查看预设：

```bash
harness-gate --version
harness-gate presets --presets-dir /path/to/GateProbes/presets
```

在已有 Git 项目中初始化（新目录先运行 `git init`）：

```bash
harness-gate --project-root /path/to/new-project init --preset rust-api --presets-dir /path/to/GateProbes/presets
```

### 检查配置并准备质量输入

```bash
harness-gate --project-root /path/to/new-project config check
harness-gate --project-root /path/to/new-project doctor
# 阅读生成的 QUALITY.md；由可信 host 配置 state、签名请求和密钥。
harness-gate --project-root /path/to/new-project verify --profile ci --all
```

参考预设生成 flow、quality 和策略配置，不安装采集器、不生成密钥或签名运行时输入。缺少这些输入时 `verify` 会 fail closed；完整接入步骤见[质量快速开始](docs/quality-workflow.md#quick-start)。`generic` 和旧 flow-only 仓库保持显式选择启用。

当前 Rust/Angular 预设是有边界的参考实现，Angular CRAP 仍不支持。未来 Vue + Go + PostgreSQL 应组合可复用 pack；registry 安装和新的生态认证不在本次范围内。详见[认证边界](docs/quality-workflow.md#capabilities-and-certification)和[未知生态端到端验收证据](docs/quality/gh-187/validation.md)。`quality evaluate` / `adapter run` 是高级重放、调试和集成接口；普通项目使用 `verify`。

推荐接入顺序：

1. 选择最接近技术栈的预设；
2. 审阅 `.harness-gate/flow.toml` 执行配置与 `.harness-gate/quality.toml` 的质量、策略和 profile；
3. 在 `.harness-gate/audit.toml` 增加项目自己的架构规则；
4. 在 `.harness-gate/secrets.toml` 增加业务或供应商特有的凭据规则；
5. 执行 `config check`，先解决引用和 schema 错误；
6. 执行 `doctor`，补齐本机工具、依赖、镜像或环境变量；
7. 按生成的 `QUALITY.md` 配置可信主机输入，然后执行 `verify --profile ci --all`，得到一个项目决策；
8. 在 CI 中使用相同命令，并按需安装只负责调用 `harness-gate hook` 的薄 hook。

`init` 会创建目标目录，但 `scope` 和 `verify` 需要目标目录是 Git worktree。项目已有配置时默认拒绝覆盖；只有确认目标内容可替换时才使用 `--force`。

<a id="内置预设"></a>
## 项目预设

Core 0.5.0 的 Postgres 预设必须先通过已安装的 GateProbes 插件生成绑定目录；冻结旧目录不能直接用于新 Core。见[迁移步骤](docs/quality-presets.md)。

Core 仅内置 `generic`。下列三个生态预设由 GateProbes 维护，需通过
`--presets-dir PATH` 或 `HARNESS_GATE_PRESETS_DIR` 指向审阅过的预设目录。
详见[外部预设接入](docs/quality-presets.md)。此加载方式需要包含本次迁移的
Core 构建；已发布的 0.4.8 仍使用旧版内置预设。

| 预设                    | 用途                        | 初始步骤                                |
| ----------------------- | --------------------------- | --------------------------------------- |
| `generic`               | 任意 Git 项目               | working tree 和 staged whitespace check |
| `rust-api`              | 单 Rust crate               | fmt、Clippy、check、test                |
| `angular-only`          | Angular/npm                 | lint、format check、test、build         |
| `angular-rust-postgres` | Angular + Rust + PostgreSQL | 双端检查、测试、构建和临时数据库        |

`init` 以同目录临时文件和原子重命名写入 `.harness-gate/flow.toml`、`.harness-gate/audit.toml`、`.harness-gate/secrets.toml` 和忽略报告目录的 `.harness-gate/.gitignore`，不会留下半写文件，也不会覆盖已有配置，除非显式传入 `--force`。`config migrate` 对目标配置使用相同的原子写入策略，并在缺失时生成 Secret Scan v2 默认规则。新建 audit v2 文件预置 Rust、TypeScript、JavaScript、SQL、TOML 和 YAML 的词法配置，可直接追加第一条规则。

预设是起点，不是运行时分支。初始化完成后，所有行为都由项目内 TOML 决定：可以重命名 component、增加 `ci` profile、换成 MySQL/Redis、调整目录或替换任意步骤，无需保留预设原有名称。

## 命令总览

| 命令                                                         | 用途                                  |
| ------------------------------------------------------------ | ------------------------------------- |
| `harness-gate presets`                                       | 列出 generic 及显式指定目录中的生态预设                      |
| `harness-gate init --preset <name>`                          | 生成执行配置及参考质量 pack；generic 仅生成执行配置 |
| `harness-gate doctor [--strict] [--json]`                    | 执行配置声明的环境检查                |
| `harness-gate cleanup [--dry-run] [--json]`                 | 检查或回收过期的 Harness-Gate 资源租约 |
| `harness-gate scope [--staged\|--base REF\|--all] [--json]`  | 列出变更和选择的 components           |
| `harness-gate secrets [--staged] [--json]`                   | 扫描已追踪文件内容中的高置信凭据模式，只输出文件名  |
| `harness-gate audit [--json]`                                | 执行正则架构规则并生成审计报告        |
| `harness-gate verify`                                        | 按默认 profile 合并执行和质量门禁，输出一个项目决策 |
| `harness-gate verify --plan [--json]` | [只读解释选择、排除、依赖和所需输入](docs/verification-plan.zh-CN.md) |
| `harness-gate plugins --lock PATH --json` | [预检离线锁定插件与宿主指定的信任密钥](docs/plugin-lock.zh-CN.md) |
| `harness-gate verify --profile ci --all`                     | 对全部 components 执行指定 profile    |
| `harness-gate hook`                                          | 对暂存快照执行 hook profile           |
| `harness-gate step <id>`                                     | 通过 secrets/audit 后单独运行一个步骤 |
| `harness-gate config check`                                  | 校验 schema、引用、路径、环境覆盖和资源安全 |
| `harness-gate config check --format json`                    | 为编辑器和 CI 输出稳定的字段路径诊断    |
| `harness-gate config print --resolved`                       | 输出最终生效配置                      |
| `harness-gate config migrate`                                | 将 schema v1 转换成 v2                |
| `harness-gate schema export`                                 | 生成 flow.toml 的 JSON Schema          |
| `harness-gate adapter run --request <PATH> --trusted-key <PATH>` | 高级接口：校验并执行签名 adapter 请求；采集器没有策略裁决权 |
| `harness-gate quality evaluate --help` | 高级接口：重放显式可信输入；普通项目使用 `verify` |
| `harness-gate parse-logs`                                    | 提取 JSON Lines ERROR trace 上下文    |

所有命令都支持全局 `--project-root <PATH>` 和 `--config <PATH>`。命令成功返回 0；配置错误、门禁失败、步骤失败、超时或中断返回非 0，适合直接用于 CI。

`cleanup --dry-run` 可以在重试前或共享 CI 主机上安全执行，只列出带有
`harness-gate` owner marker 的租约，并把结构化观察写入
`<reports>/cleanup.json`。lease schema `2` 记录规范化项目身份、逻辑资源、invocation、
完整 runtime labels、容器名和不可变 runtime object ID。过期只表示可以尝试回收；停止前
会重新 inspect，并比较租约文件名、项目、资源、invocation、labels 和不可变 ID。未知、
损坏、改名、活动或无法证明 ownership 的租约会保留，停止失败也会保留租约并返回非零状态。

交互式终端中的 `verify` 会显示进度条，并以颜色区分通过、警告和失败。重定向输出或 CI 保持纯文本；可使用 `--color auto`（默认）、`--color always` 或 `--color never` 控制颜色，`NO_COLOR` 会关闭自动颜色。

本仓库使用等价的 Cargo 别名：

```bash
harness-gate scope
harness-gate verify --components backend,frontend
harness-gate verify --profile full --all
```

## 典型工作流

### 日常开发

```bash
# 编码前确认范围
harness-gate scope

# 编码后只验证命中的组件
harness-gate verify

# 定向重复运行一个步骤；仍会先过 secrets 和 audit
harness-gate step frontend.tests
```

工作区没有变更时，默认 scope 不选择 component，`verify` 只运行固定门禁。需要确认整个仓库时必须显式传 `--all`。

### 提交前

```bash
git add <明确的文件清单>
harness-gate scope --staged
harness-gate hook
git commit -m "..."
```

项目可配置 pre-commit 执行 `harness-gate hook`。hook profile 应选择适合提交前运行的检查，具体内容由项目配置决定，不代替完整测试。

### PR 或发布前

```bash
harness-gate verify --all
```

该命令忽略变更路径，选择配置中的所有 component，并执行默认 `full` profile。

### 比较某个基线

```bash
harness-gate scope --base origin/main
harness-gate verify --base origin/main
```

`--base REF` 使用 `REF...HEAD` 的已提交变更，不包含未提交工作区内容，适合 CI 或 PR 分支验证。

### 手动指定范围

```bash
harness-gate verify --components backend
harness-gate verify --components backend,frontend --profile full
```

显式 components 会覆盖自动 scope，并且不能与 `--staged`、`--base`、`--all` 同时使用。未知 component 或 profile 会立即失败。

## 错误码

运行失败会输出 `ERROR [E####]: message`。错误码可用于 CI 日志与问题定位，
消息会保留文件路径或 Git 命令等上下文。

| 错误码 | 分类 |
| ------ | ---- |
| `E1000` | 通用命令、项目或配置失败 |
| `E1101`-`E1103` | 审计配置、执行或日志解析 |
| `E1201`-`E1202` | 秘密扫描配置或执行 |
| `E1301`-`E1304` | Git scope、scope 配置、未匹配路径或报告 |
| `E1401`-`E1404` | 验证选择、取消、执行或报告 |

### 常见修复路径

新项目可以按以下顺序接入：

```bash
harness-gate init --preset generic
harness-gate config check
harness-gate doctor
harness-gate verify --all
```

`config check` 失败时，人类可读输出会包含稳定错误码、字段路径、修复
提示和最小 schema v2 `flow.toml` 骨架。按提示修改，或执行
`harness-gate init --preset generic` 重新生成配置，然后再次运行
`harness-gate config check`。编辑器和 CI 需要结构化结果时使用
`config check --format json`。

日常开发先运行 `harness-gate scope`，再运行 `harness-gate verify`；提交前
暂存明确文件，执行 `harness-gate scope --staged` 和 `harness-gate hook`；
PR 或发布前执行 `harness-gate verify --all`，失败时上传 reports 目录供定位。

## Schema v2 概览

项目根由 `.harness-gate/flow.toml` 标识，不要求固定的 `backend/`、`frontend/` 或工具目录。component 和 profile 都是配置中的小写字符串 ID。

```toml
version = 2

[project]
name = "example"
default_profile = "full"
hook_profile = "hook"

[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"

[paths.aliases.api]
path = "services/api"
env = "API_DIR"

[[scope.rules]]
patterns = ["services/api/**"]
components = ["api"]

[[steps]]
id = "api.clippy"
label = "API Clippy"
component = "api"
profiles = ["full", "hook", "ci"]
program = "cargo"
args = ["clippy", "--manifest-path", "{api}/Cargo.toml"]
cwd = "{root}"
log = "api_clippy.log"
timeout_secs = 180
```

可用占位符包括 `{root}`、`{reports}`、`{audit_config}` 和任意 `[paths.aliases.*]`。命令使用 `program + args[]`，不经过 Shell 字符串解析。

`REPORT_DIR` / `HARNESS_GATE_REPORTS`、`HARNESS_GATE_SECRETS_CONFIG` 和步骤或服务声明的 `*_env` 字段可覆盖相应配置值。审核配置始终由当前项目 `flow.toml` 中字面量、仓库相对的 `paths.audit_config` 决定，不接受进程级审核配置覆写或环境插值；切换项目或工作流文件时显式传入 `--project-root` 和 `--config`。

完整字段、默认值、限制和示例见 [schema v2 配置参考](https://github.com/musutrade/GateHouse/blob/main/docs/configuration.zh-CN.md)。英文参考和全部 JSON Schema 见 [English configuration reference](https://github.com/musutrade/GateHouse/blob/main/docs/configuration.md) 与 [schema catalog](https://github.com/musutrade/GateHouse/blob/main/schema/README.md)。修改配置后先运行：

```bash
harness-gate config check
harness-gate config print --resolved
```

## 主要扩展点

### Doctor

`[[doctor.checks]]` 支持 `command`、`path`、`glob`、`env`、`env-or-file`、`git-config`、`git-remotes`、`version` 和 `service`。`required = false` 的失败表现为 warning，其余为 failure。

```bash
harness-gate doctor            # warning 不影响退出码
harness-gate doctor --strict   # warning 也导致非 0
harness-gate doctor --json     # 给 CI 或其他工具消费
```

### 测试解析器

`[parsers.<id>]` 支持 `regex`、JUnit、TRX 和 JSON 标准结果；步骤通过 `parser = "<id>"` 引用。
解析器会把 malformed、零结果和部分结果记录为不同失败码，避免命令虽退出 0 却没有真正执行测试。

### 临时服务

`[services.<id>]` 支持两种 provider：

- `kind = "environment"`：从已有环境变量读取连接值并注入步骤；
- `kind = "docker"`：声明镜像、端口、环境、健康检查、连接串和目标环境变量。

Docker provider 可用于 PostgreSQL、MySQL、Redis 等服务，也可通过 `runtime = "podman"` 使用 Docker
兼容的 Podman CLI。容器使用随机宿主端口，验证结束或异常退出时自动删除；省略 `runtime` 时默认使用 Docker。
本次计划选中的服务会在 secret scan 和 architecture audit 门禁运行时预热，减少首个步骤的等待时间。

步骤可用 `services = ["test-postgres", "test-redis"]` 组合多个服务，并以 `remove_env = ["DATABASE_URL"]` 删除继承的运行时变量。每个 service 必须注入不同变量，避免静默覆盖。

### 项目策略

`[policy].required_steps` 声明项目不可缺失的基础步骤。策略属于项目配置，不再编译进通用引擎；新增 component、profile、普通命令、路径别名、regex parser 或 Docker service 都不需要修改 Rust 源码。

## 选择模型

### Working tree

默认 `scope` 合并以下文件集合：

- 未暂存修改：`git diff --name-only`；
- 已暂存修改：`git diff --cached --name-only`；
- 未忽略的未跟踪文件：`git ls-files --others --exclude-standard`。

这些路径按 `[[scope.rules]]` 匹配，命中的 component 去重后用于选择步骤。`[scope].unmatched` 控制未命中路径：`fail`（默认）立即失败并列出文件，`all` 选择全部 component，`ignore` 仅在明确接受漏测风险时忽略。

### Staged

`scope --staged` 和 `hook` 只读取暂存快照。secret scan 也通过 Git index 读取文件内容，而不是读取可能不同的工作区版本。

### Step input

配置步骤默认使用 `input = "snapshot"`，因此 `{root}`、path alias 和参数都相对于本次 invocation
的输入根解析。确实需要原始 checkout 或直接读取 Git 元数据的步骤，可以显式设置
`input = "repository"`；这是一项需要审阅的兼容能力，不会改变 scope、secret scan 或 architecture audit
的输入语义。invocation 报告会记录输入模式、source identity、execution root 和 configuration digest。

### All

`--all` 不依赖 Git diff，直接选择配置步骤中出现的全部 component。适合交付门禁和干净 checkout。

### Profile

profile 由步骤的 `profiles = [...]` 隐式声明。`verify` 使用 `[project].default_profile`，`hook` 使用 `[project].hook_profile`，`verify --profile <id>` 可选择其他 profile。

## 固定安全门禁

以下行为不由项目配置关闭：

1. 外部步骤前必须依次通过 secret scan 和 audit；
2. 配置、报告和路径别名不得逃出项目根；
3. 禁止 `sh -c`、`bash -lc` 等 Shell 命令字符串；
4. 未知引用、重复 ID、非法 glob/regex/占位符和越界超时直接失败；
5. service 容器必须声明健康检查并在结束时清理；
6. 多个 service 不得向同一步骤注入同名环境变量。
7. 审计扫描根必须存在且位于项目内，`..` 和逃出项目的符号链接会被拒绝；
8. Doctor、Git 探测和 Docker 生命周期命令都有硬超时，超时时终止整个子进程组。

secret scan 检查 Git 已追踪文件和未忽略的未跟踪文件内容。具体规则由 `[paths].secrets_config` 指向的 TOML 文件提供，默认覆盖 GitHub/GitLab/npm Token、AWS access key、JWT、命名签名密钥、PostgreSQL 凭据 URL、Webhook、企业微信/钉钉密钥、HTTP Basic Auth 和 PEM 私钥头。捕获值会经过占位符与低信息值过滤；报告只记录文件名和规则 ID，不把凭据内容复制到终端或 JSON。这是快速前置检查，不替代 gitleaks、TruffleHog 等专用扫描器。

为限制扫描过程的内存使用，secret scan 和 architecture audit 都拒绝超过 16 MiB 的单个输入文件，并将该错误作为门禁失败返回。

## v1 迁移

```bash
harness-gate --project-root /path/to/project config migrate \
  --input harness-gate/.codex/flow.toml \
  --output .harness-gate/flow.toml
```

迁移器保留源文件，把原有 backend/frontend、doctor、PostgreSQL、parser、scope 和 steps 转成 v2。目标文件已存在时需要显式 `--force`。

迁移后必须执行：

```bash
harness-gate config check
harness-gate doctor
harness-gate verify --all
```

确认新配置通过后，再由人工决定何时删除 v1 文件。迁移命令本身不会删除源文件。

## 验证与报告

`verify` 固定按以下顺序执行：

1. working tree 或 staged snapshot secret scan；
2. auditor 全量架构扫描；
3. 按配置顺序执行选中 component/profile 的步骤。

报告目录由 `[paths].reports` 决定。当前项目使用 `harness-gate/.codex/reports/`。

| 文件                  | 内容                            | 常见消费者         |
| --------------------- | ------------------------------- | ------------------ |
| `changed_files.txt`   | 当前 scope 的变更文件，每行一个 | Reviewer           |
| `scope.json`          | scope mode、文件和 components   | CI、自动化工具     |
| `secret_scan.json`    | 扫描模式和命中文件名            | 安全门禁           |
| `review_context.json` | 完整审计结果、规则、文件和行号  | 修复代理、Reviewer |
| `review_context.md`   | 截断的人类可读审计摘要          | 终端或 LLM 上下文  |
| `test_result.json`    | schema v1、scope、步骤状态/attempts、失败和 artifact 引用 | CI、统计 |
| `test_result.md`      | 简洁验证摘要和 `TEST_SUMMARY`   | 人工查看           |
| `test_result.html`    | 按仓库内模板渲染的可选 HTML      | CI、人工查看       |
| `<junit>.xml`         | 报告目录内配置的可选 JUnit XML   | CI 测试平台        |
| `logs/<step>.log`     | 外部命令完整 stdout/stderr      | 失败诊断           |

终端只展示摘要。步骤失败时先看 `test_result.md` 中的日志路径，再打开对应日志；不要只根据最后一行猜测根因。

`test_result.json` 遵循[版本化 machine-result schema](https://github.com/musutrade/GateHouse/blob/main/schema/machine-result.schema.json)。为兼容旧消费者仍保留
`passed`，新消费者应使用稳定的 `status`、步骤 `attempts`、结构化 `failures`、invocation-relative 的
`artifacts` 和 `evidence_complete`，不要解析 Markdown 或日志文本来判断结果。

机器结果还记录 parser 模式/版本和完整性。优先使用 JUnit、TRX 或 JSON
标准结果；malformed、零结果和部分结果分别映射为 `RESULT_PARSE_FAILURE`、
`RESULT_ZERO` 和 `RESULT_PARTIAL`。JUnit 只接受单个 `testsuite` 或 `testsuites`
根元素，TRX 只接受单个 `TestRun` 根元素（支持命名空间前缀）；缺根、多根或根元素外
非空内容均 fail closed。有界重试记录 `retry_count` 与 `flaky`，
分片记录 merge identity 并拒绝缺失或重复测试身份。有效的到期 waiver 使用
机器可区分的 `WAIVED`，并包含审批和补偿控制证据。

JSON producer 如果输出嵌套数组或数字摘要，应显式声明 `count_path`：

```toml
[parsers.json-results]
kind = "json"
count_path = "summary.total"
minimum = 1
```

未配置时只自动发现裸顶层数组，或对象包装层中的一个受支持结果数组
（`testcases`、`testCases`、`test_results`、`testResults`、`results`）。
`duration_ms` 等任意数字、metadata/attachments 等无关数组、非数组字段和多个
候选（即使长度相同）都会以 `RESULT_PARSE_FAILURE` 拒绝；显式路径无效时也不会
回退自动发现。

迁移时可重放串行请求、比较归一化结果并记录金丝雀/回滚：

```bash
harness-gate compat run --input request.json --output result.json --old-result frozen.json
harness-gate compat compare --old frozen.json --new result.json --output comparison.json
harness-gate compat canary --state migration-canary.json --slice team-a
harness-gate compat rollback --state migration-canary.json
```

这些命令保留原始摘要和 invocation 证据，不会删除已有报告。
P2 进程外签名 adapter 边界见 [ADR-0033](docs/adr/0033-signed-out-of-process-adapter-protocol.md)。可使用 host
执行单个签名请求：

```bash
harness-gate adapter run \
  --request adapter-request.json \
  --trusted-key adapter-key.json \
  --allow-resource test-database
```

host 使用协议 v2，在启动前校验覆盖完整请求的 Ed25519 签名（adapter 身份、invocation/step、
参数、输入、环境、能力、超时、配置摘要、产物根目录、nonce 和有效期）以及可执行文件
SHA-256；清空继承环境，执行协议级能力白名单，限制 stdout/stderr 和产物预算，并拒绝
malformed 结果或越过 invocation 根目录的产物。nonce 只允许使用一次；`verify` 与
`quality collect` 共用持久化 ledger。只读快照或不可信测试环境应通过全局
`--replay-state-dir PATH` 指定挂载之外的可信宿主状态，`adapter run` 也可共用该路径。
[作用域与生命周期](docs/quality-collectors.md#durable-replay-protection)包含隔离和清理要求。
超限、超时、取消、重放和其他协议错误统一记录为
`ADAPTER_PROTOCOL_FAILURE`。

请求是一个独立的 JSON 文档，不从 `flow.toml` 读取，也不会自动启用内置步骤的
adapter。每个受信任的 Ed25519 公钥重复传入一次 `--trusted-key`；只传入本次调用
允许的 `--allow-network`、`--allow-resource` 和 `--allow-environment` 能力。执行前应
确认请求、可执行文件、产物根目录位于项目或明确受管控的部署目录内，并审核签名者。
能力白名单只是协议层声明检查，不是操作系统级的网络、文件系统、资源或进程 sandbox；
进程组清理是有界的 best-effort 尝试，也不等于完整的 descendant containment 证明。每个
输出流和单个证据文件默认最多保留 16 MiB、adapter 产物合计最多 64 MiB，单次 invocation
证据合计最多 256 MiB，reader 有独立 deadline。

JSON Lines 应用日志可提取同一 trace 的上下文：

```bash
harness-gate parse-logs \
  --input path/to/application.jsonl \
  --output /tmp/error-context.txt
```

解析器优先选择第一条 `level = ERROR` 所在的 `trace_id`，支持从事件字段、`data`、当前 `span` 和 `spans` 中提取，再收集相同 trace 的结构化记录；没有 trace 时退化为原始日志最后 30 行。

解析采用有界流式读取并在输出前脱敏：错误前最多保留 20 条上下文，输出最多 30 条记录，不会把整份无限增长的 JSON Lines 日志载入内存。已启动 service 的清理失败也会使验证失败，并写入验证报告，避免把容器泄漏隐藏为成功。

## Git Hook

在已配置的使用项目中，将以下内容保存为可执行的 `.githooks/pre-commit`，然后执行 `git config core.hooksPath .githooks`。hook 运行哪些测试由项目 profile 决定，不存在全局排除数据库测试或构建的规则。

```sh
#!/bin/sh
set -eu
root="$(git rev-parse --show-toplevel)"
cd "$root"
exec harness-gate hook
```

hook 只负责定位根目录和启动二进制，所有选择、门禁和步骤仍在 Rust 与 TOML 中。

## CI 集成

CI 推荐执行全量 profile，而不是依赖 runner 上的工作区 diff：

```bash
harness-gate config check
harness-gate doctor --strict
harness-gate verify --all
```

如需复用外部测试服务，向 job 注入 service 配置的 `external_env`；否则预拉取配置镜像并允许 runner 访问 Docker daemon。缓存 Cargo、npm 和构建目录只影响性能，不应跳过 `verify --all`。

无论成功或失败，都建议上传 `[paths].reports` 目录作为 artifact。这样可以保留审计行号、步骤耗时和完整日志。

可选的 `[[notifications.webhooks]]` 会在报告文件写入后发送 JSON。每个 URL 必须配置精确匹配的
`allowed_hosts`；凭据、通配符、解析后的 loopback/private/link-local/unspecified/multicast 地址和重定向都会
在配置或连接时拒绝。`on_failure` 默认开启，`on_success` 默认关闭；非 2xx 响应、连接错误或策略拒绝返回
报告错误 `E1404`，但不会删除已生成的报告。目的地证据只保留 scheme 和规范化主机。完整字段、模板路径
约束和 JUnit 示例见[配置参考](https://github.com/musutrade/GateHouse/blob/main/docs/configuration.zh-CN.md#报告模板junit-和通知)。多个 webhook 按配置顺序发送，首个失败会停止后续通知。

## 故障排查

### 没有 component 被选中

工作区没有变更时不会选择 component。变更路径没有命中 scope rule 时，默认 `unmatched = "fail"` 会列出遗漏文件并失败；修正 `[[scope.rules]]`，或为希望触发全量验证的项目设置 `unmatched = "all"`。只有明确接受未匹配文件不触发验证时才使用 `ignore`。

### `unknown component` / `unknown profile`

component 来自 `[[steps]].component`，profile 来自 `[[steps]].profiles`。运行 `harness-gate config check` 查看引用错误，或 `config print --resolved` 确认最终配置。

### Docker 或 Podman daemon 不可用

可以启动配置中选择的 Docker/Podman runtime，或者设置 service 的 `external_env`，例如 `TEST_DATABASE_URL`。
`doctor` 中 `required = false` 的 service 检查只产生 warning，但真正依赖该 service 的步骤仍会失败。

### Docker/Podman image 不存在

容器 provider 使用 `--pull=never`，不会在验证中隐式访问网络。按 Doctor 提示执行 `<runtime> pull <image>`，
或通过 `image_env` 指向本机已有镜像。

### 测试命令成功但 parser 失败

查看步骤日志，确认测试框架实际输出与 `patterns` 一致，并且 capture group 是整数。不要把 `minimum` 改为 0；应修正规则或测试命令，确保零测试不会被误判为成功。

### 步骤超时

先看日志判断是死锁、网络等待还是超时过短。需要项目或 CI 差异化时设置步骤或 service 的 `timeout_env`，不要复制两套配置。步骤允许 1 到 3600 秒，service 整个启动过程允许 1 到 300 秒；Doctor 单项检查用 `timeout_secs` 控制，默认 15 秒、范围 1 到 300 秒。

### 配置路径被拒绝

配置、报告、audit 文件和 path alias 必须位于项目根内。不要用 `..`、仓库外绝对路径或逃出仓库的符号链接；把需要的输入放到仓库内，或通过受控环境变量向步骤传值。

## 无需改代码的范围

以下变化只修改 `.harness-gate/flow.toml` 或 audit TOML：

- 新增或重命名 component、profile、step；
- 接入 Go、Java、Python、Node 或其他 CLI 工具；
- 调整 monorepo 路径和 scope；
- 增加 Doctor 检查；
- 增加 regex 测试结果解析器；
- 增加 Docker 或环境变量 service；
- 增加硬性规则、分层规则和逐行允许规则；
- 为 CI 增加独立超时、镜像或目录覆盖变量。

## 需要改 Rust 的边界

只有引擎出现新的行为类别时才需要修改源码，例如：

- 新增非 Docker 的服务生命周期 provider；
- 新增无法用 regex 表达的测试报告格式；
- 新增现有 Doctor kind 无法表达的检查协议；
- 修改凭据识别算法、进程取消机制或固定安全策略；
- 增加远程执行、并行调度或新的报告格式。

这类改动应同时增加单元测试、内置预设验证和配置兼容性说明，并提升版本号。

## 项目自有验证边界

应用与仓库特有的验证逻辑属于被验证项目本身。Harness-Gate 在通用边界上编排、接入和裁决，不会成为每个 API、E2E、集成、冒烟、迁移、负载或框架测试系统的实现归属。项目自有验证由三层扩展覆盖：

- **命令 hook / 执行门禁**运行项目自有命令。Harness-Gate 负责 scope/profile 选择、依赖、服务、环境、超时、重试、必需性与阻断组合；项目负责测试代码、fixture、领域断言和工具语义。
- **结构化结果适配器**接入 JUnit、SARIF 或声明式 JSON 契约等可复用机器格式，改善诊断与报告。解析结果不会把必需性、阈值、ratchet 或发布权限转移给工具或解析器。
- **质量采集器插件**度量供通用策略评估的规范化事实。采集器始终只做度量；发布版 Rust 核心保留必需性、阈值、基线、聚合和最终裁决。

一个新的应用验证工具只要能用命令 hook 契约表达，就不应强制修改通用核心代码。原生产品支持只在可复用协议、结果格式、服务原语、生态/能力 pack、认证边界或通用策略语义上成立。迁移成熟项目时，其现有必需验证在达到对等且具备 fail-closed 替代行为之前，始终是保证基线。详见 [ADR-0049](docs/adr/0049-project-owned-validation-extension-boundary.md)。

## 当前审计规则

`.harness-gate/audit.toml` 保存项目自己的 SQL、分层和模板约束。audit 配置当前 schema 为 v2，必须显式声明 `version = 2` 和 `[engine]`；规则扩展名没有对应 `comment_syntax` 时会 fail closed。旧版字符串 allowlist、缺失 engine 和版本升级方法见[配置迁移参考](https://github.com/musutrade/GateHouse/blob/main/docs/configuration.zh-CN.md#audit-v2-migration)。`arch_rules.allowed_patterns` 可声明逐行例外，不存在写死的 model trait 放行逻辑。

auditor 以整文件为单位执行正则检查并把命中映射回起始代码行：支持跨行规则、扩展名过滤、路径排除、显式类型的路径 allowlist 和起始行 allowed pattern。行注释、块注释及字符串定界符按扩展名配置，扫描时跟踪词法状态；正则默认启用 multi-line 模式。需要抽象语法树级判断时，应把 Clippy、ESLint 或其他语言 lint 工具配置为 step。

## 开发 harness-gate 本身

```bash
cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check
cargo clippy --manifest-path tools/harness-gate/Cargo.toml \
  --locked --all-targets --all-features -- -D warnings
cargo test --manifest-path tools/harness-gate/Cargo.toml --locked
python3 tools/quality/docs_consistency.py
```

修改配置模型时还应验证全部内置预设和 v1 migration 测试；修改 service provider 时须运行相应的服务集成测试，确认成功和失败路径都会清理一次性容器。本工具源码仓库没有项目级 `flow.toml`，应使用仓库 CI 和测试夹具；上面的 `verify` 接入示例用于已配置的使用项目。

### 质量门禁开发

本地质量脚本测试和 CI 使用的精确采集命令见 [tools/quality/README.md](tools/quality/README.md)。覆盖率、函数风险和隔离的关键路径证据共同构成 PR 和 push 上的 `Required Quality Aggregate`。关键路径采集器现在每次采集只构建一次插桩二进制，默认最多并发运行两个隔离测试；`--jobs 1` 选择串行执行，`--jobs N` 接受 1 到 8 个 worker。基准数据见[采集基准](docs/benchmarks/critical-path-collection/README.md)。生成的 baseline 仍只是待审候选；采纳策略见 [ADR-0039](docs/adr/0039-required-risk-and-traceability-gates.md)。

完整回归数据按需下载并按摘要缓存；基础源码构建无需下载证据。见[测试数据准备](test-data/README.md)。

### 离线 HTML 的债务类别分布

[现有 HTML 模板](tools/harness-gate/templates/verification.html)按组件、策略、测量系列、
指标、值类型和单位，统计报告提供的 `ratchet.debt`：`none`、`new`、`improved`、
`regressed`、`unchanged`、`resolved`。计数单位是报告门数，同一函数的不同策略分别计数。
测量与证据/身份必须一致；缺失标注、测量不可用或证据不一致计入 **Not available**，
未知标注计入 **Unrecognized annotation**，原始标注仍以转义、脱敏后的明细保留。
不重新计算 ratchet、债务金额或现存债务总数；`none` 不代表门禁通过，`resolved` 与
剩余债务分开。选择债务 panel 和类别可与搜索、状态、组件、复杂度范围及 CRAP 区间
组合筛选；Reset 清除全部筛选。原始类别计数和权威 blockers 不随筛选变化。
禁用 JavaScript 仍可读取所有计数与明细，工程政策及 Core 决策语义保持不变。

### 精确提交上的源码跳转

在 `[report_templates]` 中显式设置
`source_repository = "https://github.com/owner/repository"` 后，内置 HTML 模板可显示
源码链接；默认仅保留源码文本。仓库映射由项目所有者信任和配置，不读取 Git remote 或报告
携带的 URL。仅支持规范 GitHub HTTPS 仓库地址，不允许凭据、query、fragment、`.git` 后缀
或末尾 `/`；其他平台不在本增量范围。

链接要求 invocation 与质量证据的完整 40/64 位提交一致、subject/series/evidence 身份一致、
安全仓库相对路径及提交文件内的有效行范围。Core 读取本地 Git tree/blob，核对文件 SHA-256
与 subject.source_sha256；未提交或暂存源码与提交不同、对象缺失、符号链接/子模块、身份
不一致、质量不可用时均保留文本，不回退到 HEAD 或分支。路径按字节编码，URL 绑定精确提交
和行号并禁止 referrer。离线检查不证明所配远端的所有权或可访问性。

生成过程无网络请求或源码上传；禁用 Git lazy fetch、replacement objects、可选锁和 fsmonitor。
每次渲染最多检查 128 个不同文件，共享 2 秒执行预算，单命令最多 500 ms，UTF-8 文件最多
1 MiB，输出读取有界；进程回收可能增加耗时。预算耗尽后仍复用已有缓存，其余来源保持文本。
Git 缺失/不支持或读取失败只省略链接，取消仍返回错误。不改变门禁、完整性、原始行和 Core
政策。点击源码链接需要网络，报告本身仍可离线阅读。自定义模板可使用
`quality_view.groups[].rows[].source_url`，仍属于受信任项目代码；不宣称已发布版本。

### 已完成 HTML 报告的 Actions 下载

源码中的 `report-export --input <单条v1索引> --output <新目录> --commit <精确提交>`
复用已完成报告的完整 manifest 校验，导出已有 `test_result.html` 和精简
`provenance.json`；不重新渲染，不导出原始日志、证据或 machine report。
摘要钉定、已完成证据和精确 commit 必须匹配，HTML 上限 2 MiB，输出不得覆盖现有目录。
导出成功不改变 Core 的 PASS/FAIL。

[可选 Action](.github/actions/report-artifact/action.yml)与 #69 共用这一路径，默认
`upload: 'false'`，开启时必须传入可信 producer 独立提供的 provenance 摘要和 commit。
远程 Action 引用应固定审查过的完整提交；这属于源码能力，不表示已有发行版支持。
Action 仅上传两个展示文件，无额外评论或 Checks 写权限，保留期默认 7 天，可选 1–30 天，
受仓库政策限制。可信交付 job 不执行不可信 PR 代码；摘要不能替代来源认证，任意自定义
HTML 也不会被导出器自动净化。跨运行来源校验、故障处理及完整接入示例见
[英文接入说明](README.md#export-a-completed-html-report-for-actions-download)。

Linux Test job 的下载样例是经真实发布器生成的 **1000 个 subject 合成 UI 验收报告**，
故意保留 FAIL，不代表 GateHouse 自身的生产质量测量。进入 Actions 的已完成 CI run，
在 Artifacts 下载 `gatehouse-html-COMMIT-RUN-ATTEMPT`，解压后离线打开
`test_result.html`。Action 另返回 `artifact-id` 与 `artifact-url`；需要仓库读取权限，
链接随保留期或删除失效，不是在线托管网页。上传/校验失败会使该集成步骤失败，
Core 原有 required gates 与退出状态仍保持权威；移除显式上传步骤即可关闭交付。

### 自动归档已完成的本地报告

在 `flow.toml` 中配置即可启用，默认关闭：

```toml
[report_history]
archive = ".harness-gate/history"
```

目录必须是规范仓库相对路径，与 reports/template 根互不包含。已完成的 PASS/FAIL publication
先完整校验、私下复制、再次校验，再保存为不可变快照；自动生成现有
`harness-report-history/v2` 格式的 `index.json`，可通过
`report-history --input .harness-gate/history/index.json` 配合既有 policy/subject/metric 与
时间窗口参数查询。不完整证据
不归档；副本与原 invocation 文件独立，正常报告清理后仍可查询。不增加 JSONL 格式、新评分、
推断债务变化或 Core baseline/ratchet 重算。

归档含 project 标记、`snapshots/` 中的完整报告与 pin 记录、自动索引，以及排除全部归档内容的
本地 `.gitignore`。它保存本地原始证据，不是 HTML artifact 上传目录；不会自动上传。
删除配置只关闭后续归档，不删除已保存快照、不改变原报告保留策略。

每次更新验证所有已有快照和旧索引 pin，再原子替换索引；同一 publication 重试幂等。
快照完成、索引更新前发生中断时，下次更新会恢复该完整快照。已索引内容被改动/删除、重复
invocation、坏索引、外来文件、符号链接或项目身份混合均拒绝，不重新计算 pin 掩盖问题。
协作锁繁忙或崩溃后遗留锁会明确报错，不抢锁；请核实进程与 `.pending-*` 后再人工清理
遗留状态，未知/未完成内容保留供诊断。不要并发手工编辑归档；这不是对抗外部写入者的隔离边界。

最多 300 个快照、快照文件共 1 GiB、machine reports 共 64 MiB；单快照最多 256 MiB、8192 个
文件系统条目、32 层目录，并保留现有报告/控制文件/索引上限。满额提示换新目录，不自动删除历史。
更新读取既有归档证据，IO 成本随有界数据量增长，不宣称满额归档也满足 5 秒目标。
归档失败返回报告交付错误，保留已完成的原始报告和 Core 决策；不会把失败门禁变成功。
Engineering Policy 和 required gates 不变。

### 热点列表

内置离线报告在覆盖率图、复杂度 Top 10 和债务类别旁增加最高 CRAP 与最低行覆盖率列表。
每组保留原始报告中最多 10 个 gate，按 component、policy、测量系列、metric、value type
分开。使用精确小数/分数比较；相同值按源码路径、名称、subject 和 gate ID 排序。
链接可用键盘跳转到详情，过滤只隐藏原列表条目，不补入低位项。

列表不新增风险阈值，也不把可能重叠的函数覆盖率汇总成模块覆盖率；同一文件可能重复出现。
缺失/不一致证据保留为 unavailable，不算零。CRAP 排名每个小数/分数字段最多 128 字符，
限制成对比较成本；覆盖率沿用原图的非零分母及安全整数范围。超限值仍在详情和单独有界的
CRAP 分布中展示。空组/全部不可用组及无 JavaScript 场景仍可读。
Core 结果、requiredness、baseline 与 ratchet/debt 语义不变。

### 综合离线看板

`report-dashboard` 复用已验证的 v1/v2 历史索引，把最新选中报告的概览/热点、最多 12 组
policy/subject/metric 趋势、相邻选中提交的观测变化放到一个离线页面。全部索引项都验证，
包括时间窗口外的数据。没有时间参数时选最近 30 个已索引提交；时间、去重与不可比断线
规则沿用 `report-history`，不声称覆盖完整 Git 历史或证明祖先关系。

```bash
harness-gate report-dashboard --input archive/index.json --output dashboard \
  --subject SUBJECT_ID --metric coverage.line --metric risk.crap \
  --last-days 30 --last-commits 30
```

本地打开 `dashboard/dashboard.html`，展开各系列查看图、精确表与 Core 债务注解；无
JavaScript 也可操作原生展开项，当前报告的搜索过滤仅作用于当前行。policy/subject/metric
按精确 ID 匹配，可重复指定：同字段取并集、不同字段取交集。不指定则纳入所选报告中支持的
观测，包括最新报告中已缺失的 subject；超过 12 组明确要求缩小过滤，不静默截断。
支持 coverage.line、coverage.region、complexity.cyclomatic、risk.crap；ID 可从报告详情复制。

相邻观测身份完全兼容且可用时，精确比较数值增/减/相同；不是新的门禁判定、债务差额或
提交因果归因。缺失、身份变化和不可用样本保持不可比较。成对比较字段最多 128 字符，
超限保留原图/表与明确说明。当前详情使用已验证文档和内置模板，不执行自定义模板。

输出新目录只含 dashboard.html 与 provenance.json，绑定索引、选中报告、HTML 摘要、
查询条件、选择项及原始最新 Core 状态；不覆盖已有目录。历史部分最多 4 MiB，整页最多
6 MiB。生成成功不等于项目 PASS；不采集、不联网、不更新历史，不采用 baseline 或重算
Core debt/ratchet，Engineering Policy 与 required gates 保持不变。
