# WSL bridge 架构

该边界依据 [Microsoft WSL 互操作说明](https://learn.microsoft.com/windows/wsl/filesystems) 与 [SQLite WAL 限制](https://sqlite.org/wal.html)：Windows 可以通过 WSL 文件共享通道浏览 Linux 文件，但 SQLite WAL 需要同机共享内存语义，不应把 UNC 访问当作安全的数据库运行方式。

## 运行时模型

`ManagedInstance.runtime` 是带标签的运行时：原生实例保存 `{ kind: "native" }`；WSL 实例保存发行版、Linux 用户、规范化 Codex 主目录、Windows 可打开路径和架构。实例同时以 `availability: "unknown" | "available" | "unavailable"` 表示探测状态；WSL 注册记录初始为 `unknown`，启动时不主动唤醒发行版。旧注册表行迁移后默认是原生运行时；成功探测到的旧 UNC 记录会原地升级并保留 ID 与别名。

## Profile 操作

桌面命令统一构造 `ProfileOperation`。原生运行时在 Tauri 的 blocking 线程池执行；WSL 运行时编码为 `BridgeRequest`，交给目标发行版内的 `codex-session-manager-wsl-bridge`。bridge 协议 v2 增加 `InstanceSyncBridgeTarget { instance_id, codex_home }` 和 `ProfileOperation::InstanceSync`，内部 action 覆盖源数据、配置差异、自动配置选择、预览、执行与同步组进程检测。响应必须以固定标记开头并携带 `protocol_version`，因此登录 shell 的额外输出不会污染 JSON 解码。

多实例同步先由 Tauri 的异步实例组解析器仅按注册表 ID 读取源和目标，校验 ID、运行时组合、WSL 发行版/用户身份与架构，不接受前端物理路径。同步核心分为注册表解析入口和已解析 profile 执行入口：Windows 同组在本进程复用后者；WSL 同组把 Linux 源路径与目标路径交给同一次 helper，在发行版内复用相同的冲突、备份、索引和 SQLite 更新逻辑。Windows↔WSL、跨发行版、跨 Linux 用户与跨机器同步不在支持范围内。

## helper 生命周期

- Windows 安装包携带两个静态 Linux helper 资源：`codex-session-manager-wsl-bridge-x86_64` 和 `codex-session-manager-wsl-bridge-aarch64`，分别对应 `x86_64-unknown-linux-musl` 与 `aarch64-unknown-linux-musl`。
- 首次连接或 identity/fingerprint 变化时部署到 `$HOME/.cache/codex-session-manager/<app-version>/<architecture>/codex-session-manager-wsl-bridge`。
- 写入临时文件、设置 `0700` 后原子替换；每次业务操作启动一个 helper，不驻留后台。
- 应用内按 `distribution + user + architecture` 串行调度；验证缓存键还包含 `app-version` 和 bundled helper SHA-256 fingerprint。命中前仍执行远端 identity 检查，缓存失效时强制重新部署。
- 启动、stdin 写入、stdin 关闭、输出读取和等待共用同一阶段时限；超时会终止子进程，并在错误中标明阶段、时限和终止结果。
- helper 通过从 passwd entry 取得的用户登录 shell 启动，以继承 `mise`、`nvm` 等初始化后的 `PATH` 和 provider 环境变量；helper 路径由外层脚本导出为环境变量，因此兼容 fish 的 `$argv` 规则，不把路径放进 `$1`；没有可执行登录 shell 时回退 `/bin/sh`。
- helper 内部负责 WSL 进程的最终写入保护；若 Codex home 位于 `/mnt/*`，Tauri 统一路由在发出写请求前还会拒绝 Windows Codex 进程占用。
- WSL 同组同步在 helper 入口及每个目标写入前检测当前 Linux 用户的 Codex 进程；执行动作使用长超时且遇到缺失响应标记、超时或解码失败时不重试，源数据、差异和预览等只读动作保持安全重试。
- 同步写入前使用发行版内规范化真实路径检查源和目标不相同、不互相包含；源失效终止操作，单个目标失效或路径重合只生成该目标失败结果，后续目标继续。
- bridge 对协议/部署拒绝、缺失响应标记、operation failure、响应解码失败和超时使用显式错误类型。执行前拒绝可重新部署并重试；缺失标记仅对只读、预览及 `apply: false` 重试，可能写入的操作只执行一次并报告结果未知；超时和响应解码失败不重试。

## 实时 probe

- 发现和手动登记时，probe 会启动或复用目标发行版，读取实际用户、passwd home、`uname -m`、Codex home 的 `config.toml` 和登录 shell 中的 `codex`。
- 每次受管 WSL profile 操作在解析注册表运行时后再次 probe，并校验发行版、用户、Codex home 和架构；发行版被删除、用户不存在、配置缺失或架构变化会在操作前失败。
- 未启用自动同步时，应用启动不执行受管 profile probe；实例管理页的显式刷新仅更新展示状态。启用“Codex 停止后自动同步”后，每 30 秒的后台检测是明确例外，会复用本轮已解析的 target，可能启动或唤醒目标发行版；检测、同步和结果刷新不占用全局 busy，也不弹模态等待窗口。
- probe 不执行 `wsl --terminate` 或 `wsl --shutdown`，也不修改注册表中的登记记录。

Windows Codex 不支持把 `CODEX_HOME` 指向 `\\wsl.localhost\\...`、`\\wsl$\\...` 或 WSL 的 `/mnt/...` 路径。此类路径必须绑定到 WSL 运行时登记记录，所有 SQLite/WAL 和 profile 操作在发行版内执行。

## 发布边界

`.github/workflows/release.yml` 使用分别对应 `linux/amd64` 和 `linux/arm64`、以 tag+immutable digest 固定的 musl-cross 镜像构建并验证两个 helper 的 ELF 架构、静态链接、`--protocol-version` 和 `--identity` 输出。协议校验使用同样固定 digest 的 Alpine 运行时。Windows Tauri job 下载两份资源并在打包前检查文件存在；`src-tauri/tauri.windows.conf.json` 控制 Windows 专属打包，macOS/Linux 安装包不包含 helper。
