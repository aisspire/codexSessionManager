# 项目文档索引

## 当前事实

- [项目简介](codex-session-manager-项目简介.md)：项目范围、主要能力和技术栈。
- [WSL Codex 支持](features/wsl-codex.md)：Windows 桌面版的 WSL 发现、操作边界和验收规则。
- [WSL bridge 架构](architecture/wsl-bridge.md)：运行时注册、双架构 helper 协议、部署和安全边界。
- [WSL 验证清单](tests/wsl-codex.md)：自动化与本机验收入口。
- [使用说明](../使用说明.md)：面向用户的完整操作指南。
- [README](../README.md)：安装、开发和发布入口。

## 权威来源

1. 当前用户规格与已确认计划。
2. `src/profile_operation.rs`、`src/instance_registry.rs`、`src-tauri/src/wsl.rs` 和 `src-tauri/src/main.rs`。
3. 本索引链接的当前事实文档。

## 已知边界

- Windows 安装包携带 `x86_64-unknown-linux-musl` 和 `aarch64-unknown-linux-musl` 两个静态 helper；其他 WSL 架构会返回明确错误。
- 每次受管 WSL profile 操作前都会实时 probe 发行版、用户、`config.toml` 和架构；实例列表使用 `unknown`、`available`、`unavailable` 三态，状态不是永久可用性承诺。未启用自动同步时启动不唤醒 WSL，`unknown` 仍可选择，显式刷新或实际操作才执行 probe；启用“Codex 停止后自动同步”后，每 30 秒后台检测属于明确例外，可能启动或复用选中的发行版。probe 不执行 `wsl --terminate` 或 `wsl --shutdown`。
- Windows Codex 不支持把 `CODEX_HOME` 指向 `\\wsl.localhost\\...`、`\\wsl$\\...` 或 WSL 的 `/mnt/...` 路径；必须通过 WSL 发现或手动登记实例，由发行版内 helper 访问数据。
- 多实例同步只允许 Windows↔Windows，或发行版名称忽略大小写后相同且 Linux 用户完全相同的 WSL↔WSL；Windows↔WSL、跨发行版和跨用户继续拒绝。WSL 同组同步由一次 helper 调用在发行版内完成，不经 UNC 访问 SQLite 或把会话文件传回 Windows。
- WSL 自动发现只在用户点击“发现 WSL”后执行，并只探测普通发行版默认用户的 `~/.codex`。
