# WSL 验证清单

## 自动化

- `cargo test --all-targets`：协议 v2 序列化、注册表迁移/去重、旧 UNC 升级、同组 WSL 方案放行、跨运行时/发行版/用户/架构拒绝、未绑定 WSL 路径的 Native 记录拒绝、真实路径重合拒绝、已解析 profile 同步与原生冲突/备份/索引逻辑一致、UTF-16 发行版解析及 TOML 缺键不 panic。
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`：响应标记抗登录输出污染、显式 bridge 错误分类、只读缺失标记重试两次、写操作缺失标记只调用一次、协议拒绝重试、超时/解码/operation failure 不重试、同步执行长超时且不可不确定重试、双架构资源名、identity/cache key、probe 响应解析、passwd shell 路径选择和 stdin 写入超时、运行时检测已确认时跳过 Windows 进程检测。
- `npm --prefix ui run test:instance-management`：源候选、Windows/WSL 目标兼容矩阵、发行版大小写匹配、Linux 用户严格匹配、完整 WSL 路径展示、切源选择清理、方案失效目标剔除及失效源方案清空全部源依赖选择。
- `npm --prefix ui run test:path-picker`：WSL 主目录只读和原生选择器状态。
- `npm --prefix ui run test:input-cache`：当前受管实例 ID 的持久化与恢复。
- `npm --prefix ui run test:auto-sync`：后台轮询互斥、跳过重叠 tick、停止后同步及失败状态更新。
- `npm --prefix ui test`：汇总实例管理、路径选择、输入缓存和后台自动同步测试。
- `npm --prefix ui run build`、`cargo check --manifest-path src-tauri/Cargo.toml`、`cargo fmt --all -- --check`、`git diff --check`：构建与格式回归。

## 规则回归

- stop-guard 矩阵覆盖设置保存、收藏切换、备份删除、恢复、数据库修复/同步和所有 `apply: true` mutation；`apply: false`、preview、list、load、detect 不要求停止 Codex。
- identity 校验覆盖协议版本、应用版本、`x86_64`/`aarch64` 架构和资源 fingerprint；旧版本、错误架构或资源变化必须重新部署。
- 受管 profile 操作覆盖每次实时 probe、缺失发行版/用户/配置、架构不匹配、登录 shell 查找 `codex` 和 `/mnt/*` Windows 进程保护。
- `detect_codex_running` 对 WSL target 只解析/probe 一次，并在 `/mnt/*` 共享主目录下补充 Windows 进程检测；自动轮询在检测、同步和结果刷新全周期内互斥。
- `detect_instance_sync_codex_running` 只解析选中的源/目标实例组；WSL 同组只启动一次 helper，任一成员使用 `/mnt/*` 时补充 Windows Codex 检测。
- 两条 Codex 运行检测共用同一合并逻辑：运行时检测已确认运行时直接返回 `true`，不触发 Windows 进程检测；Windows 检测失败仅在尚未确认任何运行中进程时才作为错误返回。
- `validate_instance_sync_compatibility` 拒绝 `\\wsl.localhost`、`\\wsl$`、扩展 UNC 和 `/mnt/*` 的未绑定 Native 记录作为源或目标参与同步；普通 Native/Native 与合法 WSL 同组行为不变。
- 实例列表刷新发现源实例失效时，前端原子清空源、目标、会话、项目、配置选择、加载状态与预览/执行结果，并提示重新选择；仅目标失效时保留其余选择并提示剔除数量。
- release workflow 断言 macOS/Linux build 不依赖 WSL helper，独立 Windows build 依赖 helper 并安装、校验 `x86_64` 和 `aarch64` 两份资源。
- `/proc` 进程扫描按 `/proc/self` 的 UID 过滤，覆盖同 UID、其他 UID、进程消失和同 UID `PermissionDenied` 的 fail-closed 行为；WSL helper 登录脚本不使用 `exec "$1"`。

## Windows + WSL 2 本机验收

1. 点击发现后能登记 Ubuntu 默认用户的 `~/.codex`，且不会自动登记系统发行版。
2. 选择 WSL 实例后能加载会话与设置；不出现从 UNC 打开 SQLite 引起的 `database is locked`。
3. x86_64 与 ARM64 主机均能选择匹配 helper；登录 shell 可找到由 `mise`/`nvm` 管理的 `codex`；选择 Windows CLI 文件后保存的是 `wslpath` 转换结果。
4. Codex 运行时只执行只读加载；归档、恢复、数据库同步等受保护写入被后端拒绝。
5. Codex 停止后只在临时 profile 验证写操作与 compact，不对真实重要会话做破坏性验收。
6. 安装包首次连接会自动部署匹配架构的 helper 到 `$HOME/.cache/codex-session-manager/<app-version>/<architecture>/`，目标权限为 `0700`，无需安装本项目 CLI。
7. 删除或修改发行版后直接执行 profile 操作会先 probe 并返回不可用错误；probe 不会主动执行 `wsl --terminate` 或 `wsl --shutdown`。
8. 在同一发行版、同一 Linux 用户下登记两个不同 Codex 主目录，确认可加载源数据、预览并同步新增/归档会话、配置、备份、`session_index.jsonl` 与 SQLite；其他发行版、其他用户和 Windows 实例不出现在目标列表。
9. 分别在 WSL 用户内运行 Codex，以及让任一同步成员位于 `/mnt/*` 并运行 Windows Codex，确认同步写入均被阻止；停止后重新预览并手动执行，不发生自动同步。

Windows Codex 通过 `CODEX_HOME=\\wsl.localhost\\...`、`\\wsl$\\...` 或 `/mnt/...` 使用 WSL 原生 home 不在支持范围内；请使用发现或手动 WSL 登记流程。
