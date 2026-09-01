# WSL Codex 支持

## 目标

Windows 桌面版可以发现、登记和选择 WSL 内的 Codex 主目录，并通过发行版内部的 helper 完成会话浏览、整理、收藏、设置、备份恢复、数据库修复/同步和上下文压缩。

## 规则

- `WSL-R01`：不得从 Windows 进程通过 `\\wsl.localhost` 或 `\\wsl$` 直接打开 WSL 的 `state_5.sqlite`；所有 profile 操作在目标发行版内执行。
- `WSL-R02`：自动发现必须由用户点击触发，只检查普通发行版默认用户的 `$HOME/.codex/config.toml`；自定义用户或 `CODEX_HOME` 走手动登记。
- `WSL-R03`：选中受管实例时，后端以 `managed_instance_id` 对应的注册表运行时为准，忽略前端提交的物理主目录。
- `WSL-R04`：WSL helper 通过固定 shell 脚本启动，不接受前端提供的任意 shell；helper 路径通过环境变量传入目标用户的登录 shell，不依赖 fish 不支持的 `$1`；profile 操作使用版本化 JSON 协议。
- `WSL-R05`：写操作由前端预检和 helper 内最终进程检测共同保护；`/mnt/*` 共享主目录在 Tauri 业务路由中还会检查 Windows 运行时，检测失败按 Codex 仍在运行处理。
- `WSL-R06`：多实例同步允许 Windows 原生同组，或发行版名称忽略大小写后相同、Linux 用户完全相同且 helper 架构一致的 WSL 同组；Windows↔WSL、跨发行版、跨用户和架构不一致的组合在保存、预览和执行阶段拒绝。
- `WSL-R07`：由 Windows 选择器取得的项目目录或 CLI 文件，在写入 WSL profile 前使用目标发行版的 `wslpath` 转换。
- `WSL-R08`：每次解析受管 WSL profile 都实时 probe 发行版、用户、`config.toml` 和规范化架构；probe 允许发行版自动启动，但不主动终止或关闭发行版。
- `WSL-R09`：helper 资源和部署缓存按 `x86_64`/`aarch64` 区分，并通过 `protocol_version`、`app_version`、目标架构和 bundled fingerprint 校验身份；identity 或资源不匹配时重新部署。
- `WSL-R10`：启用“Codex 停止后自动同步”后，桌面端每 30 秒在后台检测 Codex 状态；该模式允许周期性 probe 并启动或唤醒选中的 WSL 发行版。检测、同步和结果刷新保持异步、无模态，单轮未结束时跳过后续 tick，失败只更新非阻塞状态提示；未启用时仍保持启动不唤醒、仅显式刷新或实际操作 probe。

## 用户流程

1. 进入“多实例管理”，点击“发现 WSL”。
2. 应用列出 WSL 普通发行版并在各发行版内探测默认用户的 `~/.codex`。
3. 非默认用户或自定义主目录填写发行版、Linux 用户和绝对 Linux 路径后手动登记。
4. 在任一会话页面的“已登记实例”中选择带 `WSL · <发行版>` 标识的实例。
5. Linux 主目录保持只读；切换“手动输入目录”后才恢复 Windows 原生路径编辑。
6. 在“本机同步工作区”选择 WSL 源时，只显示同发行版、同 Linux 用户的其他 WSL Codex 主目录；源和全部目标通过一次 helper 调用在发行版内完成会话、配置、索引和 SQLite 同步。

## 失败路径

- 没有 `wsl.exe`、发行版启动失败、用户不存在或缺少 `config.toml`：逐项显示错误，不把 UNC 路径当原生目录回退。
- helper 缺失或协议不匹配：从安装包资源原子部署一次并重试；仍失败时保留退出码、stdout 和 stderr。
- 架构不是 `x86_64`/`amd64` 或 `aarch64`/`arm64`：明确报告不支持，不尝试执行错误架构的 helper。
- 实例列表标记为 `available` 但实时 probe 失败：立即拒绝本次 profile 操作，返回发行版、用户、配置或架构错误，不依赖旧的列表状态。
- 目标用户的登录 shell 不在继承的 `$SHELL` 中猜测；probe 从 passwd entry 读取 shell，必要时回退 `/bin/sh`，以便加载 `mise`/`nvm` 环境查找 `codex`。
- 未绑定运行时的 UNC 或 `/mnt/...` 手工路径：拒绝直接访问，提示重新发现或手动登记。Windows Codex 通过 `CODEX_HOME=\\wsl.localhost\\...` 或 `CODEX_HOME=\\wsl$\\...` 使用 WSL 原生 home 同样不受支持。
- 同步实例组包含 Windows↔WSL、不同发行版、不同 Linux 用户或不同 helper 架构：保存方案、预览和执行均拒绝；目标目录失效或与源的真实路径重合时记录该目标失败并继续其他目标，源目录失效则终止整次操作。
- bridge 的协议/部署拒绝只在执行 profile operation 前发生时允许重新部署并重试；缺失响应标记仅对只读、预览和 `apply: false` 重试。写操作遇到缺失标记时返回“执行结果未知”，超时、响应解码失败和 operation failure 均不重试，以避免重复写入。
