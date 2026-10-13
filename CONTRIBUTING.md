# 贡献指南

感谢你对 Harness-Gate 的关注！我们欢迎各种形式的贡献。

## 规范性工程政策

所有贡献（包括人类、Codex、Symphony、其他 Agent 或自动化产生的变更）都必须遵守 [Harness-Gate Engineering Policy](docs/engineering-policy.md)。该文档是仓库的规范性工程契约；ADR 解释历史决策，OpenSpec 描述具体变更，但没有在 Issue/OpenSpec 中重复某条长期规则并不表示该规则失效。

如果变更涉及 quality、coverage、complexity、CRAP、collector、evidence、policy、baseline、ratchet/debt、exception、CI profile、preset、project aggregation 或 release authority，PR/OpenSpec 必须明确说明相关 Engineering Policy 语义保持不变，或提出显式的 normative policy delta。不得以降低阈值、跳过 required gate、吞掉 measurement failure、重置 baseline 或扩大 waiver 的方式把 CI “修绿”。

## 开发环境设置

### 前置要求

- Rust 1.88 或更高版本（以 `tools/harness-gate/Cargo.toml` 的 `rust-version` 为准）
- Git
- Docker（用于测试服务集成）

### 克隆和构建

```bash
git clone https://github.com/musutrade/GateHouse.git
cd GateHouse
cargo build --manifest-path tools/harness-gate/Cargo.toml
python3 tools/maintenance/test_data.py
cargo test --manifest-path tools/harness-gate/Cargo.toml
```

### 运行测试

```bash
# 准备完整回归数据（固定摘要、缓存复用）
python3 tools/maintenance/test_data.py

# 运行单元测试
cargo test --manifest-path tools/harness-gate/Cargo.toml

# 运行集成测试
cargo test --manifest-path tools/harness-gate/Cargo.toml --test integration

# 格式检查
cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check

# Clippy 检查
cargo clippy --manifest-path tools/harness-gate/Cargo.toml -- -D warnings
```

以上只是基础开发检查。涉及质量语义的变更还必须满足 Engineering Policy 和仓库 Required Quality Aggregate 中适用的 coverage、risk/CRAP、critical-path、evidence、policy/ratchet 等 required gates；不能用单独的 `cargo test` 代替这些门禁。

CI 执行模型、工具版本、缓存与制品信任边界、故障诊断及托管性能对比见
[CI operating guide](docs/quality/ci-topology/README.md)。

## 提交代码

### 提交信息格式

我们遵循 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

```
<type>(<scope>): <subject>

<body>

<footer>
```

类型：
- `feat`: 新功能
- `fix`: 修复 bug
- `docs`: 文档更新
- `style`: 代码格式（不影响代码运行）
- `refactor`: 重构
- `test`: 测试相关
- `chore`: 构建过程或辅助工具的变动

示例：
```
feat(doctor): add Node.js version check

Add doctor check to verify Node.js version is 18 or higher for
frontend projects.

Closes #123
```

### Pull Request 流程

1. Fork 本仓库
2. 创建你的特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交你的修改 (`git commit -m 'feat: add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

### PR 检查清单

- [ ] 变更遵守 `docs/engineering-policy.md`；若修改长期工程政策，已提交显式 normative policy delta 与证据
- [ ] 代码通过 `cargo fmt` 格式化
- [ ] 代码通过 `cargo clippy` 检查
- [ ] 所有适用的 required gates 和测试通过
- [ ] 添加了必要的测试和 fail-closed/negative regression coverage
- [ ] 更新了相关文档
- [ ] 更新了 CHANGELOG.md（适用时）

## 代码规范

### Rust 代码风格

- 遵循 [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- 使用 `rustfmt` 默认配置
- 使用 `clippy` 进行 lint 检查
- 避免不必要的 `unsafe` 代码
- 添加适当的文档注释

### 测试要求

- 新功能必须包含单元测试
- 修复 bug 应包含回归测试
- Coverage、CRAP、complexity、risk 和 debt/ratchet 的权威要求以 Engineering Policy、已接受 measurement series 和机器门禁为准；不得用此文档中的简化数字覆盖它们
- 集成测试应该是独立的，不依赖外部状态

## 报告问题

### Bug 报告

请包含以下信息：
- Harness-Gate 版本
- 操作系统和版本
- Rust 版本
- 重现步骤
- 预期行为
- 实际行为
- 相关日志或错误信息

### 功能请求

请描述：
- 功能的使用场景
- 预期的行为
- 可能的实现方案
- 是否愿意贡献代码

## 文档贡献

文档改进同样重要！包括：
- 修正错误
- 改进说明
- 添加示例
- 翻译

## 行为准则

请遵守独立的 [行为准则](CODE_OF_CONDUCT.md)，保持尊重、聚焦工作，并通过其中的
私密渠道报告行为问题。

## 许可

贡献的代码将在 MIT 许可下发布。


## 发布与文档同步

用户入口以中英文 README、[文档目录](docs/README.md)和[当前交付状态](docs/release-status.md)为准。
修改安装方式或发布版本时，同步两种语言的版本号、命令参数、验证方式、下载成本和支持边界；检查发布治理与插件安装指南，避免仅改英文 README。
发布完成后将公开运行、签名/下载验证和兼容性凭据写入交付状态，并按各自验收项更新 issue/OpenSpec。历史 ADR 和验收报告保留原有日期与失败记录，不批量替换历史版本。
新增兼容性声明必须有对应真实验收；安装成功不能代替采集认证，插件发布不能代替使用项目接入完成。
运行 `python3 tools/quality/docs_consistency.py` 检查本地链接、配置示例与 schema 同步，并按正常 PR/CI 流程合并。

完整回归数据按需获取，基础构建无需下载。见 [数据准备与离线缓存](test-data/README.md)。
