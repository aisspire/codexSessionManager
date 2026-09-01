# WSL 验证清单

## 自动化

- `cargo test --all-targets`：协议序列化、注册表迁移/去重、旧 UNC 升级、WSL 同步拒绝、UTF-16 发行版解析及 TOML 缺键不 panic。
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`：响应标记抗登录输出污染、显式 bridge 错误分类、只读缺失标记重试两次、写操作缺失标记只调用一次、协议拒绝重试、超时/解码/operation failure 不重试、双架构资源名、identity/cache key、probe 响应解析、passwd shell 路径选择和 stdin 写入超时。
- `npm --prefix ui run test:instance-management`：运行时展示及 WSL 同步排除。
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

Windows Codex 通过 `CODEX_HOME=\\wsl.localhost\\...`、`\\wsl$\\...` 或 `/mnt/...` 使用 WSL 原生 home 不在支持范围内；请使用发现或手动 WSL 登记流程。
