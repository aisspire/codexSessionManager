# Codex Session Manager

Codex Session Manager 是一个用于查看和整理 OpenAI Codex 本地会话的桌面工具。
它把常用的会话查看、筛选、批量编辑、归档、设为活动、置顶和删除到回收区等操作放进一个简单的桌面界面里。

## 背景

Codex 的本地会话信息分散在多个位置：

```text
<CodexHome>/state_5.sqlite            会话状态、项目路径、归档状态、rollout_path 等索引信息
<CodexHome>/sessions/**/*.jsonl       会话正文和会话元信息
<CodexHome>/archived_sessions/*.jsonl 已归档会话文件
<CodexHome>/session_index.jsonl       最近会话标题和更新时间索引
```

所以会话在某个列表里“不见了”，通常不代表正文丢失，而可能是 SQLite、JSONL 文件、归档目录或 session_index 之间的状态不一致。
本工具的列表视图会把 SQLite 中的 threads 和 `sessions` / `archived_sessions` 中可解析的 rollout JSONL 合并展示，便于看清楚当前本地数据的实际分布。

## 功能

- 按项目分组查看会话，支持展开和折叠项目分组。
- 按项目、提供方、模型、来源、归档状态和关键字筛选会话。
- 查看会话详情，包括标题、项目、模型、状态、更新时间、会话文件和索引状态。
- 批量修改会话标题前缀、提供方和项目路径。
- 单个或批量归档会话。
- 将已归档会话设为活动。
- 置顶会话，通过刷新 rollout 文件时间让会话更容易出现在较新的位置。
- 删除会话到工具回收区，不直接永久删除。
- 自动保存输入框内容、筛选条件和项目分组展开状态。

当前版本暂时不提供备份和从备份恢复功能；相关功能会在重新设计后再接入。

## 运行环境

直接使用打包后的桌面应用时，通常只需要系统能运行 Tauri 应用。Windows 上一般需要 WebView2 Runtime，现代 Windows 通常已经自带。

从源码运行或打包需要：

- Rust stable 和 Cargo
- Node.js 和 npm
- Windows WebView2 Runtime
- Tauri 2 相关依赖

本地开发运行：

```bash
cd ui
npm install
npm run tauri -- dev
```

打包桌面应用：

```bash
cd ui
npm run tauri -- build
```

## 基本使用

### 1. 设置 Codex 主目录

打开应用后，先确认顶部的 Codex 主目录。默认值是 `~/.codex`。
如果你的 Codex 数据在其他位置，需要改成实际目录，例如：

```text
Windows: C:\Users\<用户名>\.codex
WSL:     /mnt/c/Users/<用户名>/.codex
Linux:   ~/.codex
```

目录不正确时，列表可能为空，或者操作的不是你想管理的那份会话数据。

### 2. 刷新会话列表

点击“刷新”后，应用会重新读取本地会话数据，并按项目分组展示。
列表不是单纯的 SQLite threads 表，而是会合并：

- `state_5.sqlite` 中的 threads
- `sessions/**/*.jsonl` 中带 `session_meta` 的会话
- `archived_sessions/*.jsonl` 中带 `session_meta` 的会话
- `session_index.jsonl` 中的标题和更新时间

因此软件显示数量可能大于直接查询 `state_5.sqlite` 的 threads 数量。

### 3. 筛选和搜索

顶部筛选栏可以按以下条件缩小范围：

- 项目路径
- 提供方
- 模型
- 来源
- 活动、已归档或全部
- 搜索关键字

筛选只影响当前列表展示，不会修改 Codex 数据。

### 4. 查看和编辑单个会话

点击会话行会打开详情面板。详情中可以查看标题、项目、提供方、模型、来源、会话文件和会话索引状态。

详情面板中可直接编辑标题、项目和提供方。保存后，应用会更新本地索引并刷新列表。

### 5. 批量选择

你可以勾选单个会话，也可以在项目分组上选择该项目下的所有可见会话。
选中多个会话后，可以统一执行归档、活动、置顶或删除等操作。

## 会话管理操作

### 归档

归档会把会话设为已归档，并尽量把对应 rollout 文件从 `sessions` 移到 `archived_sessions`。
归档适合把暂时不用、但仍想保留的会话从活动列表中收起来。

### 活动

“活动”会把已归档会话重新设为活动，并尽量把对应 rollout 文件从 `archived_sessions` 移回 `sessions`。
活动完成后，应用会 touch 一下 rollout 文件，刷新文件修改时间，帮助 Codex 或列表刷新逻辑感知变化。

### 置顶

置顶会刷新选中会话 rollout 文件的访问时间和修改时间。
它不重写会话正文，只用于让会话更容易出现在较新的位置。

### 删除

删除会把会话移动到工具自己的回收区，并把 SQLite 中对应会话标记为归档。
它不是直接永久删除。

## 路径兼容

Codex 数据里可能同时存在 Windows 路径和 WSL 路径，例如：

```text
C:\Users\14139\.codex\sessions\...
/mnt/c/Users/14139/.codex/sessions/...
```

Windows 版桌面应用在执行归档、活动、置顶等文件操作时，会把 `/mnt/<盘符>/...` 自动转换为对应 Windows 路径，例如 `/mnt/c/...` 转为 `C:\...`。
这样可以避免把文件误移动到当前盘下的 `\mnt\c\...` 目录。

## 写入安全

写入前建议关闭正在使用同一份数据的 Codex。
应用会在归档、活动、删除等写入操作前检查 Codex 是否正在运行；检测到可能占用同一份本地数据时，会拒绝写入，避免两个程序同时修改造成状态混乱。

当前版本不会自动创建备份。执行大批量整理或删除前，如果会话很重要，请先自行复制保存 `.codex` 中的关键数据。

## 原理简述

桌面端由三部分组成：

```text
ui/          Vite + TypeScript 前端界面
src-tauri/   Tauri 2 桌面壳和前后端桥接
src/         Rust 核心逻辑
```

前端负责展示列表、筛选、选择、详情面板和操作按钮；Tauri 桥接层把这些操作交给 Rust 核心执行。
Rust 侧负责读取 Codex 本地状态、解析 rollout JSONL、移动归档文件、更新 SQLite 状态、更新索引和执行安全检查。

写入时遵循几个原则：

- 只修改必要的索引字段或文件位置。
- 不全量重写会话正文。
- 删除进入工具回收区，不直接永久删除。
- Codex 正在运行时拒绝写入。
- 对 WSL 挂载路径做运行时归一化。

## 支持

如果这个项目帮到了你，star、反馈问题，或小额支持都很感谢：

[支持链接](https://aisspire.github.io/support/)
