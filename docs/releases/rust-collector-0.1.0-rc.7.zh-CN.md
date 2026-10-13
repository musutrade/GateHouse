# 原生 Rust/MIR collector 0.1.0-rc.7

RC7 更新大型 Rust 项目的依赖归属处理，并支持 monorepo 源路径绑定。它保持独立预发布版本，提供 Linux x86-64、macOS Intel/Apple Silicon、Windows x86-64 的原生可执行文件。

- 每次 LLVM export 内缓存已经验证成功的依赖源码路径归属，避免重复遍历不可变依赖根目录；每个原生符号、计数映射和 region 仍逐项验证。
- 在签名项目绑定中增加可选 `source_prefix`，将包内源码路径精确映射到 monorepo 路径；路径与源码哈希仍必须一致，不使用 basename 回退。
- 每个平台运行实际 Core/native 验收；公开产物包含 SBOM、SHA256SUMS、Sigstore 签名和 GitHub 构建来源证明。

从 Core 0.4.6 标签取得安装脚本后，可独立安装：

```sh
bash install.sh --rust-only --rust-version 0.1.0-rc.7
```

Rust 1.97.1、匹配的 LLVM 工具与 Python 3.12+ 仍由外部环境提供。MIR 指标包含编译器生成控制流，与独立 Rust source-risk 的源码指标是不同序列，不能直接混用。RC7 不更改质量阈值，也不自动批准项目绑定或基线。
