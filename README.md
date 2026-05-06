# Codex Session Manager

Codex Session Manager 是一个用于管理 OpenAI Codex 本地会话数据的工具，包含 Rust CLI 和 Tauri 桌面界面。

这个项目的起点很朴素：我在切换 Codex 配置、provider、模型和 Windows/WSL 环境时，遇到过会话丢失显示、路径不匹配、provider 不一致、会话索引缺失等问题。聊天正文其实还在，但 Codex Desktop/CLI 依赖多份本地状态文件共同决定“会话是否出现、归到哪个项目、用哪个 provider 过滤”。这个工具就是为了解决这些迁移和整理问题写的。

它的目标不是粗暴全文替换，而是安全地扫描、备份、校验、迁移和编辑 Codex 会话索引数据。

## 典型场景

- 在 Windows、WSL、不同磁盘路径之间切换 Codex 使用环境。
- 从默认 `openai` provider 切到自定义 API provider，例如 `cm`。
- 修改模型或 provider 后，让旧会话继续出现在 Codex Desktop/CLI 列表里。
- 修复 `state_5.sqlite`、`sessions/**/*.jsonl`、`session_index.jsonl` 之间的不一致。
- 批量修改会话 provider、项目路径、会话名称。
- 归档、恢复、删除到工具 trash，而不是直接永久删除。

## Codex 会话数据结构

Codex 的会话显示通常涉及这些文件：

```text
<CodexHome>/sessions/**/*.jsonl       聊天正文和第一行 session_meta
<CodexHome>/state_5.sqlite            threads 表，保存索引、项目、provider、标题等字段
<CodexHome>/session_index.jsonl       轻量最近会话索引，包含 thread_name
<CodexHome>/config.toml               当前模型和 provider 配置
<CodexHome>/.codex-global-state.json  前端工作区和显示状态
```

聊天记录“不显示”通常不代表正文丢失，常见原因包括：

- `threads.rollout_path` 仍是 Windows 路径，当前 WSL 后端读不到。
- `threads.cwd` 仍是 `E:\code\xxx`，当前环境按 `/mnt/e/code/xxx` 分组。
- `threads.model_provider` 是旧 provider，当前配置切到了另一个 provider。
- `session_index.jsonl` 缺少普通用户会话记录。
- `threads.title` 与 `session_index.jsonl.thread_name` 不一致。
- 普通 `cli/vscode` 会话的 `has_user_event = 0`。

## 功能概览

CLI 当前支持：

```text
scan                  只读扫描 Codex 会话状态
list                  按项目、provider、模型、归档状态列出会话
backup                备份关键 Codex 状态文件
validate              校验 SQLite 和 JSONL 一致性
migrate-provider      迁移 provider，例如 openai -> cm
migrate-paths         按 path-map 迁移 rollout_path/cwd
repair-session-index  补齐普通用户会话的 session_index 记录
repair-has-user-event 修复普通 cli/vscode 会话的 has_user_event
archive               归档选定会话
restore               恢复选定会话，或从 backup manifest 恢复文件
delete                移动选定会话到本工具 trash，并标记为归档
app-server-probe      调用 Codex app-server thread/list 和 thread/read 做黑盒验证
```

桌面端当前支持：

- 按项目、provider、模型、来源、关键词和归档状态筛选会话。
- 表格列宽拖拽、单选、多选、全选当前列表。
- 批量归档、恢复、删除到工具 trash。
- 批量修改已选会话的 provider、项目路径和会话名前缀。
- 多选重命名时按时间从早到晚生成 `前缀(1)`、`前缀(2)`。
- 右侧详情面板单独修改会话名称、项目路径和 provider。
- 右侧字段编辑支持小笔进入编辑、回车或失焦确认草稿、底部保存按钮统一保存。
- 写入前复用 Rust 核心安全检查和备份逻辑。

## 安全原则

请把这些规则当作硬规范：

- 先扫描，再 dry-run，最后才写入。
- CLI 迁移类命令默认 dry-run，写入必须显式加 `--apply`。
- 写入默认自动备份关键文件。
- 不要在 Codex Desktop、Codex CLI 或 Codex app-server 正在运行时写入。
- 工具会在写入前检测 Codex Desktop、Codex CLI 和 Codex app-server；检测到运行中会拒绝写入。
- `delete` 不做永久删除，只移动到 `<CodexHome>/trash/codex-session-manager/` 并写入 manifest。
- 不要把 `guardian` 或 `subagent` 当作普通聊天迁移。
- 不要全文替换 JSONL；工具只修改第一行 `session_meta`。
- 不修改 `logs_2.sqlite`。
- 不修改 `_sqlx_migrations` checksum。
- 不备份或打印 `auth.json`。

## 构建与测试

Rust CLI：

```bash
cargo build
cargo test
```

前端：

```bash
cd ui
npm install
npm run build
```

桌面端开发：

```bash
cd ui
npm run tauri dev
```

本仓库是在 WSL/bash 环境里开发的；如果当前 WSL Node 不可用，可以从 Windows 侧运行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "Set-Location 'E:\code\codexSessionManager\ui'; npm run build"
```

## 快速开始

只读扫描：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  scan
```

列出会话：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  list --archived all
```

备份：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  backup
```

provider dry-run：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  migrate-provider --from openai --to cm
```

确认输出后应用：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  migrate-provider --from openai --to cm --apply
```

路径 dry-run：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  --path-map 'E:\code=/mnt/e/code' \
  migrate-paths
```

确认输出后应用：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  --path-map 'E:\code=/mnt/e/code' \
  migrate-paths --apply
```

更多桌面端和 CLI 操作见 [使用说明.md](使用说明.md)。

## 代码分层

```text
src/main.rs            程序入口，只负责打印错误和退出码
src/cli.rs             命令行参数解析和命令分发
src/profile.rs         Codex home、provider、model、path-map 的运行上下文
src/path_map.rs        Windows/WSL 路径前缀迁移规则
src/state_db.rs        state_5.sqlite 的 threads 读写和 integrity_check
src/rollout.rs         sessions JSONL 第一行 session_meta 读写
src/session_index.rs   session_index.jsonl 解析、追加和重命名更新
src/backup.rs          关键文件备份
src/scan.rs            只读扫描报告
src/migrate.rs         provider/path/index/user-event/会话字段迁移动作
src/session_ops.rs     归档、恢复、删除到 trash
src/validate.rs        迁移后校验
src-tauri/             Tauri command bridge
ui/                    Vite + TypeScript 前端
```
