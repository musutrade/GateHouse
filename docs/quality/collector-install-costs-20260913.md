# GH-255：轻量安装、升级与迁移实测（2026-09-13）

本报告对应 [OpenSpec change](../../openspec/changes/lightweight-collector-install-and-independent-upgrades/proposal.md)、[任务状态](../../openspec/changes/lightweight-collector-install-and-independent-upgrades/tasks.md) 和 [ADR 0051](../adr/0051-lightweight-collector-storage.md)。这是未发布实现的本地验证，**不是正式发行验收或干净主机认证**。使用说明见[轻量安装指南](lightweight-collector-installation.md)。

## 方法与边界

全部构建、采集、安装和迁移均在 GH-255 工作区内新建目录完成；没有读取用户已有安装或其他工作区回执。使用 operator 提供的官方 rustc-dev 1.97.1 archive（SHA-256 `0109304e1995cce9e3362208f5d4ec0944e52a2ddfbc0a85d4ce5bea5d3081ab`），新建 driver/sysroot/private runtime，重新构建 Core 0.4.2。编译器 commit 为 `8bab26f4f68e0e26f0bb7960be334d5b520ea452`，LLVM 为 22.1.6；准确工具字节身份同时保存在本次 runtime inventory 和原生捕获中。

大小诊断用临时 RSA 密钥和测试 eligibility 签署**本次真实运行时字节**，每次激活执行实际编译、运行和覆盖率导出。它不模拟编译，但不覆盖生产 Sigstore 身份认证。本地标签 rc.1–rc.4 只是诊断版本；不表示发布了这些版本。升级仅修改 launcher 注释及其 runtime inventory，工具链字节保持一致。

获取量按压缩对象 `st_size` 精确计算，离线介质读取量和网络量分别报告。运行空间按安装树、共享对象和 staging 内普通文件的 `(device,inode)` 去重后累计 `st_blocks * 512`；目录、根锁和 journal 不计入。逻辑大小会重复计入硬链接，不能当作真实分配量。缓存列为完整压缩对象的逻辑字节，不包括测试发布源、离线介质、开发构建目录或验证器。空间数值取决于文件系统，不能外推到所有平台。

历史问题基线来自 issue：用户 rc.3 安装为 2,129,596,416 字节，压缩插件与工具链对象合计 282,243,324 字节，历史首次下载约 444 MB。**这些不是本次新实现结果**。本次是含调试 native driver 的新构建，原始 tar 为 1,688,197,120 字节，不能与正式 rc.3 的压缩大小直接比较。

## 本次测量

同一真实运行时的旧格式安装在本地重建后，占用从 **3,382,693,888** 降至 **1,315,278,848** 字节，迁移释放 **2,067,415,040** 字节。迁移后重放校验原始签名 tar 摘要成功，选择回滚版本和重建完整离线归档均成功。减少量包括删除原始 tar 和按内容共享重复文件；并非签名元数据占用了数 GB。

自动模式从本机探测精确匹配的 Rust/C/Python 文件，实际只从离线介质读取 **166,726,298** 字节、360 个缺失对象；运行空间 **1,315,344,384** 字节，压缩缓存 **166,726,298** 字节，原生自检成功。固定工具链模式首次读取 **432,174,999** 字节、2,891 个对象。显式提供完整兼容运行时，实际读取组件 **0** 字节，原生自检成功，运行空间 **1,315,336,192** 字节。外部工具仅作为只读来源，复制进私有存储，不创建指向可变用户工具的硬链接。

上述三种实际安装的网络量均为 **0**，因为采用本地离线介质；不能声称测出了互联网下载耗时或生产全流程首次下载量。本次另外构建的私有安装 bootstrap 为 15,056,047 压缩字节、55,080,960 展开字节（后续源代码变化会改变此数值）。每次诊断的五份签名控制文件合计 1,830,806 字节；生产还需发行者固定的 cosign、认证 shell 入口及 Sigstore 材料，其总量由发布构建自动生成，这次没有生产材料，未填造总数。

| 本地诊断版本 | 获取对象 | 增量组件字节 | 运行分配字节（清理后） | 压缩缓存字节 | 已展开临时组件分配字节 |
| --- | ---: | ---: | ---: | ---: | ---: |
| rc.1，首次 pinned | 2,891 | 432,174,999 | 1,315,344,384 | 432,174,999 | 1,309,999,104 |
| rc.2 | 2 | 141,545 | 1,321,177,088 | 432,316,544 | 495,616 |
| rc.3 | 2 | 141,544 | 1,327,009,792 | 432,458,088 | 495,616 |
| rc.4 | 2 | 141,543 | 1,327,009,792 | 432,599,631 | 495,616 |

三次增量均只有 launcher 和 runtime inventory 两个对象；没有获取工具链对象。清理后保留 rc.2、rc.3、rc.4，实际回滚至 rc.3。复查三个保留版本的 rustc/cargo 具有相同 device/inode，且所有版本均没有完整安装 tar。逐次耗时为 338.150、102.043、107.153、153.035 秒；首次包含迁移/导出诊断，不能作为用户安装延迟比较。

本次记录了展开临时组件空间及安装阶段检查点，**没有连续采样完整临时空间峰值**。诊断字段 `peak_preparation_allocated_upper_bound` 的值依次为 3,024,331,872、1,720,661,088、1,726,493,792、1,732,326,496 字节，计算为最大存储检查点加临时展开组件及两倍最大 payload。它只用于估计 payload 准备空间，不包含缓存、签名控制文件临时副本和编译自检输出，不应当称为整个安装的实测峰值。完整峰值采样仍是发布 runner 验收待补项。

[机器可读摘要](collector-install-costs-20260913.json) 保留逐次结果、变化对象准确名称/摘要/字节、回滚后 inode 检查和本地完整报告的 SHA-256。

完整对象名称、压缩/展开字节、摘要、复用原因及检查点留在本次 `target/gh-255/costs-run2/{report.json,auto-report.json,transport-*/transport.json}`，可用下述脚本重新生成。签名控制文件、许可证和 SPDX/provenance 始终保留；已安装版本没有 `.delivery/collector.tar`。发布端完整 tar 和诊断目录是测量输入，不是安装器自动保留的版本数据。

## 验收场景与证据强度

| 场景 | 实际证据及限制 |
| --- | --- |
| 1 兼容环境 | 上述 auto 和显式复用的真实安装、自检和字节计数通过；没有获取整套工具链。 |
| 2 缺失环境、独立与离线 | 空安装根、空组件缓存的 pinned/offline 原生自检通过；12 项私有运行时测试包括真实 Core 读取认证证据。不是干净主机认证。 |
| 3 连续三次插件升级 | rc.1→rc.2→rc.3→rc.4 本地实际工具字节、增量获取、共享存储与自检；保留 current 加两个回滚版本。 |
| 4 工具链升级 | 自动化测试实际改动工具对象，验证只获取该对象、其他 inode 复用、自检失败不切换及旧版回滚；对象为测试 fixture。本机仅供应一个官方 compiler，未执行第二套真实 LLVM 的升级验收。 |
| 5 业务依赖变化 | 通用 Rust 项目的本地 path dependency 0.1.1→0.1.2，两次真实编译和 `certify` 成功；工具身份相同，Cargo inputs 不同。原生/Core 负例验证错误 anchor、nonce/replay 和身份拒绝，既有 reviewed series transition 不变；未采用新基线。 |
| 6 故障和并发 | 83 项发布测试包含断点传输、签名/内容/receipt 篡改、准备与自检失败、注入 ENOSPC、磁盘预检和真实多进程安装/清理锁。未执行物理断电或把磁盘真正填满。 |
| 7 rc 迁移清理 | 新建旧格式树的实际迁移、重验、回滚、导出以及回收字节计数；receipt 写入中断恢复、未知文件/链接拒绝和引用保护由自动化测试覆盖。 |
| 8 仓库检查 | 下表记录命令、通过结果和不适用项；不是生产发行验收。 |

## 可复现命令

所有输出目录必须为本工作区内的新目录。不要使用用户安装作为 `runtime` 输入。先运行：

```bash
export CARGO_TARGET_DIR="$PWD/target/gh-255/cargo"
python3 tools/quality/rust-native-driver/bootstrap.py \
  --archive target/symphony-inputs/rustc-dev-1.97.1-x86_64-unknown-linux-gnu.tar.xz \
  --sysroot "$(rustc --print sysroot)" --output target/gh-255/native-driver
python3 tools/quality/build_rust_collector.py --inventory \
  --driver target/gh-255/native-driver/build/debug/harness-gate-rust-native-driver \
  --sysroot target/gh-255/native-driver/sysroot \
  --crate-cache /home/gem/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f \
  --rustc-dev target/symphony-inputs/rustc-dev-1.97.1-x86_64-unknown-linux-gnu.tar.xz \
  --build-record target/gh-255/native-driver/bootstrap.json --lock target/gh-255/runtime-lock.json
python3 tools/quality/build_rust_collector.py --lock target/gh-255/runtime-lock.json --output target/gh-255/runtime
cargo build --manifest-path tools/harness-gate/Cargo.toml --locked
cargo vendor --locked --manifest-path tools/quality/fixtures/rust-native/file-classification/Cargo.toml target/gh-255/vendor
RUST_COLLECTOR_RUNTIME="$PWD/target/gh-255/runtime" \
RUST_COLLECTOR_TEST_OUTPUT="$PWD/target/gh-255/native-tests" \
RUST_COLLECTOR_TEST_VENDOR="$PWD/target/gh-255/vendor" \
HARNESS_GATE_NATIVE_POLICY_BINARY="$PWD/target/gh-255/cargo/debug/harness-gate" \
  python3 -m unittest discover -s tools/quality/tests -p test_rust_collector_runtime.py -v
```

`--crate-cache` 指向本机 Cargo registry 的 cache 子目录，是原始构建输入，路径需按环境调整。从本次生成的 `native-tests/tmp*/configured-core/binding.json` 选用新 binding；脚本只复制并调整诊断包装版本，不把它作为 capture authority：

```bash
python3 tools/release/tests/measure_collector_components.py \
  --runtime target/gh-255/runtime --core target/gh-255/cargo/debug/harness-gate \
  --binding target/gh-255/native-tests/tmpc34sam8y/configured-core/binding.json \
  --output target/gh-255/costs-fresh
LD_LIBRARY_PATH="$PWD/target/gh-255/runtime/lib:$PWD/target/gh-255/runtime/rust/lib" \
PATH="$PWD/target/gh-255/runtime/bin:/usr/bin:/bin" CARGO_NET_OFFLINE=true \
  target/gh-255/runtime/python/bin/python3 -I -S -B \
  tools/release/tests/measure_business_dependency_upgrade.py \
  target/gh-255/runtime target/gh-255/dependency-upgrade-fresh
```

## 仓库验证记录

以下 Rust 命令均使用工作区本地 `CARGO_TARGET_DIR=$PWD/target/gh-255/cargo`。完整日志在 `target/gh-255/`。

| 命令 | 结果 / 日志 |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked --no-fail-fast` | 397 passed、0 skipped；`nextest-retry.log`。设置 `NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost`。 |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | 通过；`fmt.log`。 |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | 通过；`clippy.log`。 |
| `python3 -m unittest discover -s tools/quality/tests -v` | 415 tests OK、3 个环境条件 skip；`quality-tests.log`。 |
| 上述 private runtime 定向测试 | 12 tests OK、无 skip；`native-tests.log`，补上全量运行中缺少 `RUST_COLLECTOR_RUNTIME` 的分组。 |
| `python3 -m unittest discover -s tools/quality/tests -p 'test_rust_native_*.py' -v` | 设置本次 `NATIVE_DRIVER`、`NATIVE_DRIVER_SYSROOT`、`HARNESS_GATE_NATIVE_POLICY_BINARY` 后 17 tests OK、无 skip；`native-driver-tests.log`，补上另外两个环境 skip 分组。 |
| `python3 -m unittest discover -s tools/release/tests -v` | 83 tests OK；`release-tests-final.log`。 |
| `bash tools/release/tests/test_install.sh` | installer integrity tests pass；`install-shell-tests.log`。 |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | 通过；`docs-consistency-final.log` 与生成 JSON。初次检查发现尚未创建本报告及继承的只读 Cargo target，随后修正。 |
| `openspec validate lightweight-collector-install-and-independent-upgrades --strict` | 通过；`openspec.log`。 |
| `harness-gate config check` / `harness-gate verify --profile ci --all` | **不适用，未运行**：本仓库没有 `.harness-gate/flow.toml`，没有人为补造项目配置。 |

初次 nextest 的 localhost webhook 测试返回 `io: Connection refused`（359 passed、1 failed、37 未运行）；上列重跑全部通过。最初 Core 构建继承 `/home/gem/cargo-target`，创建 `.cargo-build-lock` 返回 `Read-only file system (os error 30)`；改为工作区 target 后构建和检查通过。首次 inventory 错用了 cache 父目录，报缺少 `block-buffer`，纠正为上述 cache 子目录后通过。大小诊断初次被 binding 包装版本检查阻止，随后被 host library symlink 检查阻止；采用新建诊断 binding 副本和已验证 host library 的真实路径后重新运行成功，没有绕过文件验证。

`unshare -Ur true` 返回 `unshare: write failed /proc/self/uid_map: Operation not permitted`。因此干净主机隔离、生产 RSA+Sigstore 在线安装与真实第二套 compiler 升级仍需发布流水线在适合的 runner 上执行。本次没有修改生产 release pin、签名策略、测量认证或用户质量基线，也没有发布生产版本。不能把本地 RSA fixture 或已有历史验收替代这些尚未执行的场景。

发布实现按每组最多 900 个对象拆分组件 release，签名 catalog 固定各组下载地址；逐页核对全部附件摘要，所有 draft 验证后先公开组件、最后公开安装入口。该数量低于 [GitHub 每 release 1,000 个附件限制](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases)。自动化测试覆盖跨页上传校验和分片篡改阻止公开；没有实际执行生产发布。
