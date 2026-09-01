# Codex Session Manager 项目简介

项目地址：[https://github.com/aisspire/codexSessionManager](https://github.com/aisspire/codexSessionManager)

[![GitHub stars](https://img.shields.io/github/stars/aisspire/codexSessionManager?style=flat&logo=github&label=stars)](https://github.com/aisspire/codexSessionManager/stargazers)
[![Latest release](https://img.shields.io/github/v/release/aisspire/codexSessionManager?style=flat&label=release)](https://github.com/aisspire/codexSessionManager/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/aisspire/codexSessionManager/total?style=flat&label=downloads)](https://github.com/aisspire/codexSessionManager/releases)
[![License](https://img.shields.io/github/license/aisspire/codexSessionManager?style=flat&label=license)](https://github.com/aisspire/codexSessionManager/blob/main/LICENSE)

## 简介

Codex Session Manager 是一个面向 OpenAI Codex 桌面端本地会话的管理工具，用可视化、可筛选、可备份的方式解决 Codex 会话分散、难找、难整理和本地索引不一致的问题。

## 项目背景

在高频使用 Codex 桌面端时，会话数据会分散在 `state_5.sqlite`、`sessions/`、`archived_sessions/` 和 `session_index.jsonl` 等本地文件中。随着项目和会话数量增加，用户容易遇到旧会话难以定位、归档状态不一致、SQLite 索引与 JSONL 文件不同步、批量整理缺少安全入口等问题。

这个项目的目标不是替代 Codex，而是补齐桌面端在本地会话管理上的一些缺失和不便：先把本地数据整理成清晰的统一视图，再提供谨慎的批量操作、备份恢复和数据库修复能力，让用户能够更放心地维护自己的 Codex 会话资产。

## 核心功能及亮点

- **统一会话视图**：合并读取 SQLite、活动会话 JSONL、归档会话 JSONL 和 `session_index.jsonl`，把分散的本地会话整理成按项目分组的列表，适合快速找回跨项目、跨时间段的 Codex 工作记录。
- **高效筛选与批量整理**：支持按项目、模型、提供方、来源、归档状态、收藏状态和关键字过滤，并可批量归档、设为活动、置顶、删除或编辑元数据，减少重复手工整理会话的时间。
- **备份与恢复机制**：在删除、编辑会话信息、压缩上下文等关键操作前创建会话级备份，并提供备份预览、恢复和快照管理，降低误操作对本地会话数据的影响。
- **数据库修复与同步**：可以预览并保守修复 SQLite 与 JSONL 之间的不一致，例如补齐 JSONL-only 会话、修正无效 `rollout_path`、同步归档状态等，让 Codex 本地索引和实际文件重新对齐。
- **本地优先与安全边界**：工具直接处理本机 Codex 数据，不上传会话内容；写入操作会检测 Codex 是否正在运行，并在可能发生并发写入时阻断操作，更适合处理私人或敏感开发会话。
- **Windows 管理 WSL Codex**：可显式发现或手动登记 WSL 实例，并把全部 profile 操作交给发行版内的静态 helper，避免跨 UNC 直接打开 SQLite/WAL。

## 技术栈

- **Rust**：实现核心数据处理逻辑，包括扫描本地会话、解析 JSONL、读写 SQLite、管理备份、执行安全检查和提供 CLI 能力。
- **Tauri**：构建跨平台桌面应用外壳，并负责前端界面与 Rust 后端能力之间的桥接。
- **TypeScript + Vite**：实现桌面端前端界面，包括会话列表、筛选、详情面板、批量操作、备份恢复和错误提示等交互。
- **rusqlite**：访问和修复 Codex 本地 `state_5.sqlite`，处理会话索引、归档状态和路径信息。

## 项目成果

- 项目已开源，仓库地址为 [aisspire/codexSessionManager](https://github.com/aisspire/codexSessionManager)，采用 MIT License。
- 已接入 GitHub Releases 发布流程，推送 `v*` tag 后会通过 GitHub Actions 构建 Windows、Linux 和 macOS 安装包。
- 已接入 Tauri updater，应用可以通过 GitHub Releases 的 `latest.json` 检查更新。
- 已提供较完整的 README、使用说明、截图和自动化测试，覆盖会话扫描、备份恢复、数据库修复、批量操作、版本脚本和 Tauri 配置等关键路径。

## 适合的用户

- 经常使用 OpenAI Codex 桌面端，并积累了大量本地会话的开发者。
- 同时维护多个项目，希望按项目快速查找、筛选和整理 Codex 会话的人。
- 需要批量归档、恢复、删除或修正会话元数据，但又担心误操作损坏本地数据的用户。
- 遇到 Codex 本地会话列表、归档状态、SQLite 索引或 JSONL 文件不一致，希望先预览再谨慎修复的人。
- 想了解 Tauri + Rust + TypeScript 如何构建本地数据管理桌面工具的开发者。
