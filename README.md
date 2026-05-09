# Codex Session Manager

一个用于管理 OpenAI Codex 本地会话数据的 Rust CLI。

它的目标不是“全文替换路径”，而是安全地扫描、备份、校验和迁移 Codex 的会话索引数据。典型场景包括：

- 从 Windows Codex 数据迁移到 WSL 使用。
- 从 ChatGPT/OpenAI 默认 provider 切换到自定义 API provider。
- 修改模型名后，让旧会话继续出现在 Codex Desktop/CLI 的会话列表里。
- 修复 `state_5.sqlite`、`sessions/*.jsonl`、`session_index.jsonl` 之间的不一致。
- 删除、归档、恢复等功能的后续扩展。

## 为什么需要这个工具

Codex 的聊天显示通常涉及多层状态：

```text
<CodexHome>/sessions/**/*.jsonl       聊天正文
<CodexHome>/state_5.sqlite            线程索引和过滤字段
<CodexHome>/session_index.jsonl       轻量最近会话索引
<CodexHome>/config.toml               当前模型和 provider 配置
<CodexHome>/.codex-global-state.json  前端工作区和显示状态
```

聊天记录“不显示”不代表正文丢失。常见原因是：

- `threads.rollout_path` 仍是 Windows 路径，WSL 后端读不到。
- `threads.cwd` 仍是 `E:\code\xxx`，当前 WSL 环境按 `/mnt/e/code/xxx` 分组。
- `threads.model_provider` 是旧的 `openai`，当前配置切到了 `cm`。
- 普通 `cli/vscode` 会话的 `has_user_event = 0`。
- `session_index.jsonl` 缺少普通会话索引记录。
- `.codex-global-state.json` 里的前端工作区状态和后端路径环境混杂。

## 当前功能

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
```

## 安全规范

请把这些规则当作硬规范：

- 先 `scan`，再 `dry-run`，最后才 `--apply`。
- 所有迁移命令默认是 dry-run，不写文件。
- 写入必须显式加 `--apply`。
- 写入默认自动备份关键文件。
- 不要在 Codex Desktop、Codex CLI 或 app-server 正在运行时写入。
- 工具会在写入前检测 Codex Desktop、Codex CLI 和 Codex app-server；检测到运行中会拒绝写入。
- `delete` 不做永久删除，只移动到 `<CodexHome>/trash/codex-session-manager/` 并写入 `manifest.json`。
- 不要迁移 `guardian` 或 `subagent` 为普通聊天。
- 不要全文替换 JSONL。工具只修改第一行 `session_meta`。
- 不修改 `logs_2.sqlite`。
- 不修改 `_sqlx_migrations` checksum。
- 不备份或打印 `auth.json`。

## 构建

```bash
cargo build
```

运行测试：

```bash
cargo test
```

如果你刚在 WSL 安装 Rust，但当前 shell 找不到 `cargo`，通常需要执行：

```bash
source "$HOME/.cargo/env"
cargo --version
```

如果希望永久生效，把下面这行加入 `~/.bashrc`：

```bash
. "$HOME/.cargo/env"
```

## 基本用法

所有命令都可以指定 Codex home：

```bash
cargo run -- scan --codex-home /mnt/c/Users/14139/.codex
```

如果使用已构建二进制：

```bash
./target/debug/codex-session-manager scan --codex-home /mnt/c/Users/14139/.codex
```

注意：`--codex-home`、`--provider`、`--model`、`--path-map` 是全局参数，写在子命令前或后都可以。

## 推荐修复流程

### 1. 只读扫描

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  --provider cm \
  --model gpt-5.5 \
  --path-map 'C:\Users\14139\.codex\sessions=/mnt/c/Users/14139/.codex/sessions' \
  --path-map 'E:\code=/mnt/e/code' \
  scan
```

重点看这些字段：

```text
threads with non-active provider
rollout_path values matching path maps
cwd values matching path maps
missing rollout files
JSONL/SQLite id mismatches
visible user threads missing from session_index
```

### 2. 手动备份

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  backup
```

备份目录会包含 `manifest.json`，用于后续精确恢复：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  restore --manifest /mnt/c/Users/14139/.codex/backups/codex-session-manager-1770000000/manifest.json --apply
```

如果也要备份完整 `sessions/`，加：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  backup --include-sessions
```

`sessions/` 可能很大，所以默认不复制。

### 3. dry-run 迁移 provider

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  migrate-provider --from openai --to cm
```

确认输出中的 `sqlite rows` 和 `jsonl files` 符合预期后再执行：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  migrate-provider --from openai --to cm --apply
```

这会修改：

- `state_5.sqlite` 的 `threads.model_provider`
- `sessions/**/*.jsonl` 第一行 `session_meta.payload.model_provider`

不会修改历史消息正文。

### 4. dry-run 迁移路径

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  --path-map 'C:\Users\14139\.codex\sessions=/mnt/c/Users/14139/.codex/sessions' \
  --path-map 'E:\code=/mnt/e/code' \
  migrate-paths
```

确认后应用：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  --path-map 'C:\Users\14139\.codex\sessions=/mnt/c/Users/14139/.codex/sessions' \
  --path-map 'E:\code=/mnt/e/code' \
  migrate-paths --apply
```

这会修改：

- `state_5.sqlite` 的 `threads.rollout_path`
- `state_5.sqlite` 的 `threads.cwd`
- `sessions/**/*.jsonl` 第一行 `session_meta.payload.cwd`

### 5. 修复普通会话入口

先 dry-run：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  repair-has-user-event
```

应用：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  repair-has-user-event --apply
```

这个命令只处理：

```text
source in ('cli', 'vscode')
archived = 0
has_user_event = 0
title 或 first_user_message 存在
```

不会把 `guardian` 或 `subagent` 当成普通聊天。

### 6. 修复 session_index

先 dry-run：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  repair-session-index
```

应用：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  repair-session-index --apply
```

`session_index.jsonl` 是 append-only 风格，工具会追加缺失记录，而不是默认重写整个文件。

### 7. 校验

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  validate
```

如果发现问题，命令会以非零退出码结束，方便脚本捕获。

## 会话管理

### 列出会话

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  list --project /mnt/e/code/codexSessionManager --provider cm --model gpt-5.5 --archived all
```

`list` 支持：

- `--project`
- `--provider`
- `--model`
- `--source`
- `--archived active|archived|all`
- `--search`

### 归档与恢复会话

先 dry-run：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  archive --id <thread-id>
```

应用：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  archive --id <thread-id> --apply
```

恢复归档状态：

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  restore --id <thread-id> --apply
```

### 删除到工具 trash

```bash
cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  delete --id <thread-id> --apply
```

这会把会话 rollout JSONL 移到：

```text
<CodexHome>/trash/codex-session-manager/<timestamp>/
```

并写入 `manifest.json`。SQLite 中对应线程会被标记为 archived，不会直接永久删除数据库行。

## 桌面软件

仓库包含一个 Tauri 桌面壳：

```text
src-tauri/   Tauri command bridge
ui/          Vite + TypeScript 前端
```

桌面端第一屏就是会话管理器，包含筛选栏、会话表格、详情面板和批量工具栏。单个会话和批量会话都可以执行归档、恢复、删除到 trash 和备份。

桌面端调用同一套 Rust 核心库，所以写入前同样会检测 Codex 是否正在运行；运行中会拒绝执行归档、恢复、删除、manifest restore 等写操作。

本地运行桌面端通常需要先安装前端依赖：

```bash
cd ui
npm install
npm run tauri dev
```

## 你的 WSL + 自定义 provider 示例

你的目标配置大致是：

```toml
model = "gpt-5.5"
model_provider = "cm"
model_reasoning_effort = "high"

[model_providers.cm]
name = "OpenAI"
base_url = "http://localhost:48760/v1"
```

推荐扫描命令：

```bash
cargo run -- \
  --profile wsl-local-cm \
  --codex-home /mnt/c/Users/14139/.codex \
  --provider cm \
  --model gpt-5.5 \
  --path-map 'C:\Users\14139\.codex\sessions=/mnt/c/Users/14139/.codex/sessions' \
  --path-map '/home/ais/.codex/sessions=/mnt/c/Users/14139/.codex/sessions' \
  --path-map 'E:\code=/mnt/e/code' \
  --path-map '\\?\E:\code=/mnt/e/code' \
  scan
```

推荐迁移顺序：

```bash
cargo run -- --codex-home /mnt/c/Users/14139/.codex migrate-provider --from openai --to cm
cargo run -- --codex-home /mnt/c/Users/14139/.codex migrate-provider --from openai --to cm --apply

cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  --path-map 'C:\Users\14139\.codex\sessions=/mnt/c/Users/14139/.codex/sessions' \
  --path-map '/home/ais/.codex/sessions=/mnt/c/Users/14139/.codex/sessions' \
  --path-map 'E:\code=/mnt/e/code' \
  --path-map '\\?\E:\code=/mnt/e/code' \
  migrate-paths

cargo run -- \
  --codex-home /mnt/c/Users/14139/.codex \
  --path-map 'C:\Users\14139\.codex\sessions=/mnt/c/Users/14139/.codex/sessions' \
  --path-map '/home/ais/.codex/sessions=/mnt/c/Users/14139/.codex/sessions' \
  --path-map 'E:\code=/mnt/e/code' \
  --path-map '\\?\E:\code=/mnt/e/code' \
  migrate-paths --apply

cargo run -- --codex-home /mnt/c/Users/14139/.codex repair-has-user-event
cargo run -- --codex-home /mnt/c/Users/14139/.codex repair-has-user-event --apply

cargo run -- --codex-home /mnt/c/Users/14139/.codex repair-session-index
cargo run -- --codex-home /mnt/c/Users/14139/.codex repair-session-index --apply

cargo run -- --codex-home /mnt/c/Users/14139/.codex validate
```

## 代码分层

```text
src/main.rs            程序入口，只负责打印错误和退出码
src/cli.rs             命令行参数解析和命令分发
src/profile.rs         Codex home、provider、model、path-map 的运行上下文
src/path_map.rs        Windows/WSL 路径前缀迁移规则
src/state_db.rs        state_5.sqlite 的 threads 读写和 integrity_check
src/rollout.rs         sessions JSONL 第一行 session_meta 读写
src/session_index.rs   session_index.jsonl 解析和追加修复
src/backup.rs          关键文件备份
src/scan.rs            只读扫描报告
src/migrate.rs         provider/path/index/user-event 迁移动作
src/validate.rs        迁移后校验
```

代码中保留了必要注释，重点解释“为什么这么做”，例如为什么只改 JSONL 第一行、为什么 path-map 必须显式配置、为什么迁移命令默认 dry-run。

## 后续功能建议

第一版稳定后，可以继续加：

- `list`：按项目、provider、模型、归档状态列出会话。
- `archive`：通过 SQLite 归档会话。
- `restore`：从 backup manifest 恢复。
- `delete`：移动到工具自己的 trash，而不是直接永久删除。
- `tui`：用终端界面预览迁移计划。
- `tauri`：把核心库包装成桌面软件。

## 支持

如果这个项目帮到了你，star、反馈问题，或小额支持都很感谢：  

[支持链接](https://aisspire.github.io/support/)
