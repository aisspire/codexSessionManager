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
- 预览并保守修复数据库与 JSONL 文件之间的不一致，包括 JSONL-only 会话、不可用的 `rollout_path` 和归档状态偏差。
- 自动保存输入框内容、筛选条件和项目分组展开状态。

当前版本暂时不提供通用的备份恢复界面。数据库修复在应用写入前会自动备份关键文件，但其他批量整理或删除操作前，仍建议自行保存重要数据。

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

## 发布流程

本项目通过 GitHub Actions 和官方 `tauri-apps/tauri-action` 在推送版本标签时自动构建桌面安装包。
发布 workflow 位于 `.github/workflows/release.yml`，触发条件是推送 `v*` 格式的 Git tag，例如 `v0.2.0`。
不要从 GitHub Releases 页面手动创建正式发布作为第一步；手动创建的 Release 不会自动带上安装包。正确入口是在本地推送 tag，让 Actions 先构建并生成草稿 Release。
`src-tauri/tauri.conf.json` 中必须启用 `bundle.active`，否则 Tauri 只会生成裸可执行文件，`tauri-action` 上传阶段会提示 `No artifacts were found`。

当前发布会构建：

- Windows：`windows-latest`
- Linux：`ubuntu-22.04`
- macOS Apple Silicon：`aarch64-apple-darwin`
- macOS Intel：`x86_64-apple-darwin`

发布前先用版本脚本同步项目版本号。脚本参数只写纯版本号，不带 `v`：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\set-version.ps1 0.2.0
```

脚本会同步更新：

- `Cargo.toml`
- `Cargo.lock`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `ui/package.json`
- `ui/package-lock.json`

脚本还会移除 `src-tauri/tauri.conf.json` 中重复的 `version` 字段，让 Tauri 以 `src-tauri/Cargo.toml` 作为桌面应用版本来源。

发布前建议至少执行下面两项本地检查：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tests\set-version.tests.ps1
npm --prefix ui run build
```

如果要在本机额外验证 Tauri 打包，可运行：

```powershell
npm --prefix ui run tauri -- build
```

Windows 本地打包时，如果出现无法删除 `src-tauri\target\...\codex-session-manager-desktop.exe` 或 `拒绝访问`，通常是旧应用进程、杀毒软件或系统仍占用该 exe。关闭正在运行的桌面应用后再重试。

确认版本文件和构建检查没问题后，先创建一个干净的发布提交，再给这个提交打 tag。
如果版本脚本实际修改了文件，提交这些版本变更：

```powershell
git status
git add Cargo.toml Cargo.lock src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json ui/package.json ui/package-lock.json
git commit -m "chore(release): 准备 v0.2.0"
git push origin main
```

如果当前版本号本来就是目标版本，`git status` 没有任何版本文件变化，也仍然建议创建一个空的发布提交，让 tag 指向清晰的 release commit：

```powershell
git commit --allow-empty -m "chore(release): 准备 v0.2.0"
git push origin main
```

然后创建并推送 tag：

```powershell
git tag v0.2.0
git push origin v0.2.0
```

推送 tag 后，GitHub Actions 会自动执行 Release workflow，并把各平台构建产物上传到同一个 GitHub Release。
当前 workflow 设置为 `releaseDraft: true`，因此生成的是草稿 Release。Actions 全部通过后，到 GitHub Releases 页面检查标题、说明和附件，确认无误后手动发布。
如果 Release 页面没有安装包，先检查 Actions 页面里这个 tag 对应的 Release workflow 是否已经跑完并通过；没有 workflow 运行时，通常是 tag 没有推送到远端，或 tag 是在 workflow 提交之前创建的。

早期内部测试可以先不配置代码签名。未签名包仍可构建和上传，但 Windows 可能显示未知发布者或 SmartScreen 提示，macOS 也可能要求用户绕过系统安全限制。面向公开用户稳定分发前，建议再补 Windows 代码签名证书、Apple Developer ID 签名和 macOS notarization。

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

## 数据库修复

“数据库修复”位于左侧导航栏。它用于处理 Codex 本地数据中 SQLite、JSONL 文件和索引之间的常见不一致，默认只做预览，不会立即写入。

点击“预览修复项”后，应用会扫描：

- `sessions/**/*.jsonl` 中首行为 `session_meta` 且带 `id` 的会话文件
- `archived_sessions/*.jsonl` 中首行为 `session_meta` 且带 `id` 的会话文件
- `state_5.sqlite` 的 `threads` 表
- `session_index.jsonl`

预览结果会按行展示修复类型、会话 ID、当前值、目标值和状态。可以勾选单个项目，也可以点击“全选可修复”选择所有可保守处理的项目，再点击“应用已选修复”。

当前会自动应用的保守修复包括：

- JSONL-only 会话：JSONL 文件存在但 SQLite `threads` 中缺少对应行时，补充缺失行。
- `rollout_path` 修复：SQLite 中的 `rollout_path` 为空、不存在或不可用时，改为当前真实 JSONL 路径。
- 路径归一化：将 SQLite 中不可用的 `/mnt/<盘符>/...` 类 `rollout_path` 修为当前系统下可用的真实 JSONL 路径。
- 归档状态同步：唯一 JSONL 位于 `archived_sessions` 时同步 `archived = 1`，位于 `sessions` 时同步 `archived = 0`。

以下情况只报告，不自动修改：

- SQLite-only row：SQLite 中有会话行，但没有找到唯一对应 JSONL 文件。
- 重复 JSONL：同一会话 ID 对应多个 JSONL 文件，需人工确认。

应用修复前，软件会先检查 Codex 是否正在运行；检测到 Codex 可能正在使用同一份数据时会拒绝写入。写入前还会在 `<CodexHome>/backups/codex-session-manager-<timestamp>` 下备份关键文件，至少包括 `state_5.sqlite` 和 `session_index.jsonl`，并尽量包含 SQLite 的 WAL/SHM 文件。

数据库修复不会重写会话正文 JSONL，也不会删除 SQLite-only 行。

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
应用会在归档、活动、删除和数据库修复等写入操作前检查 Codex 是否正在运行；检测到可能占用同一份本地数据时，会拒绝写入，避免两个程序同时修改造成状态混乱。

数据库修复在写入前会自动备份 `state_5.sqlite`、`session_index.jsonl` 等关键文件。其他大批量整理或删除操作前，如果会话很重要，请先自行复制保存 `.codex` 中的关键数据。

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
- 数据库修复只应用可确认的保守修复，无法唯一确认的项目只报告。

## 支持

如果这个项目帮到了你，star、反馈问题，或小额支持都很感谢：

[支持链接](https://aisspire.github.io/support/)
