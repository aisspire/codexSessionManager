import { invoke } from "@tauri-apps/api/core";
import { singleSelectionForCodexAction } from "./codexExitConfirm";
import { loadInputCache, saveInputCache } from "./inputCache";
import { loadProjectExpansionCache, saveProjectExpansionCache } from "./projectExpansionCache";
import {
  buildSessionMetaItems,
  sessionStateDisplay,
  sessionTitle as displaySessionTitle,
} from "./sessionDisplay";
import { buildProjectGroups, type ProjectGroup } from "./sessionGroups";
import "./styles.css";

type AppPage = "batch-edit" | "session-management" | "restore-backups" | "database-repair";
type SessionScope = "active" | "archived" | "favorite" | "all";
type SessionCommand =
  | "archive_sessions"
  | "active_sessions"
  | "delete_sessions"
  | "refresh_session_updated_at";

interface ProfileInput {
  codex_home: string;
  profile?: string;
  provider?: string;
  model?: string;
  path_maps: string[];
}

interface SessionSummary {
  id: string;
  title?: string;
  first_user_message?: string;
  project?: string;
  provider?: string;
  model?: string;
  source?: string;
  archived: boolean;
  favorite: boolean;
  updated_at?: string;
  sort_updated_at_ms?: number;
  rollout_path?: string;
  in_session_index: boolean;
}

interface SessionListFilter {
  project?: string;
  provider?: string;
  model?: string;
  source?: string;
  archived: SessionScope;
  search?: string;
}

interface MutationReport {
  action: string;
  applied: boolean;
  sqlite_rows: number;
  jsonl_files: number;
  index_entries: number;
}

interface SessionOperationReport {
  action: string;
  applied: boolean;
  sqlite_rows: number;
  index_entries: number;
  backup_manifests?: string[];
  warnings?: string[];
  trash_manifest_path?: string;
}

interface CompactReport {
  action: string;
  applied: boolean;
  session_id: string;
  backup_manifest: string;
  command: string[];
  exit_code?: number;
  stdout: string;
  stderr: string;
}

type DatabaseRepairKind =
  | "missing-thread-row"
  | "repair-rollout-path"
  | "normalize-rollout-path"
  | "sync-archived-state"
  | "sqlite-only-thread"
  | "duplicate-jsonl";

interface DatabaseRepairItem {
  id: string;
  kind: DatabaseRepairKind;
  session_id: string;
  summary: string;
  before?: string;
  after?: string;
  rollout_path?: string;
  applicable: boolean;
  skip_reason?: string;
}

interface DatabaseRepairPreview {
  items: DatabaseRepairItem[];
  backup_note: string;
}

interface DatabaseRepairApplyReport {
  applied_items: number;
  sqlite_rows: number;
  backup_dir?: string;
  backup_files: string[];
  skipped_items: DatabaseRepairItem[];
}

type DatabaseSyncMode = "never" | "auto-when-codex-stops";

interface AppSettings {
  backup: BackupSettings;
  database_sync: DatabaseSyncSettings;
  codex_cli: CodexCliSettings;
}

interface BackupSettings {
  max_bytes?: number | null;
  max_age_days?: number | null;
  max_count?: number | null;
  skip_unique_archive_on_auto_prune: boolean;
  minimum_free_bytes: number;
}

interface DatabaseSyncSettings {
  mode: DatabaseSyncMode;
}

interface CodexCliSettings {
  command_path?: string | null;
}

interface SessionBackupSummary {
  session_id: string;
  title?: string;
  project?: string;
  group?: string | null;
  latest_created_at_unix: number;
  local_exists: boolean;
  snapshots: SessionBackupSnapshot[];
}

interface SessionBackupSnapshot {
  backup_id: string;
  created_at_unix: number;
  trigger: BackupTrigger;
  manifest_path: string;
  size_bytes: number;
}

type BackupTrigger = "delete" | "edit" | "manual" | "database-repair" | "restore-preflight" | "compact";

interface RestorePreview {
  backup_id: string;
  session_id: string;
  restore_session_path?: string;
  overwrites_existing: boolean;
  index_entries: number;
  favorite: boolean;
}

interface RestoreReport {
  applied: boolean;
  files_restored: number;
  restored_session_path?: string;
  index_entries: number;
  sqlite_rows: number;
  preflight_backup_manifest?: string;
  favorite_restored: boolean;
}

interface BackupGroupDeleteReport {
  session_ids: string[];
  deleted_backup_ids: string[];
  deleted_dirs: string[];
}

type DetailEditField = "title" | "project" | "provider";

interface DetailEditState {
  editingField: DetailEditField | "";
  draft: string;
  pendingId: string;
  pendingTitle: string;
  pendingProject: string;
  pendingProvider: string;
}

type TaskItemStatus = "pending" | "running" | "done" | "failed";

interface TaskProgressItem {
  id: string;
  label: string;
  status: TaskItemStatus;
  detail?: string;
}

interface BusyState {
  active: boolean;
  label: string;
  completed: number;
  total: number;
  items: TaskProgressItem[];
  error?: string;
}

interface AppDialog {
  kind: "codex-running";
  title: string;
  message: string;
  primaryLabel: string;
}

const pageLabels: Record<AppPage, string> = {
  "batch-edit": "批量编辑",
  "session-management": "会话管理",
  "restore-backups": "恢复备份",
  "database-repair": "数据库修复",
};

const GITHUB_REPOSITORY_URL = "https://github.com/aisspire/codexSessionManager";

const blankDetailEdit = (): DetailEditState => ({
  editingField: "",
  draft: "",
  pendingId: "",
  pendingTitle: "",
  pendingProject: "",
  pendingProvider: "",
});

const idleBusyState = (): BusyState => ({
  active: false,
  label: "",
  completed: 0,
  total: 0,
  items: [],
});

const cachedInput = loadInputCache();
const cachedExpandedProjects = loadProjectExpansionCache();

const state = {
  // 页面只改变“可用操作”，列表、筛选和选择状态在两个页面之间共享。
  activePage: "batch-edit" as AppPage,
  profile: {
    codex_home: cachedInput?.codexHome || "~/.codex",
    path_maps: [],
  } satisfies ProfileInput,
  filter: {
    archived: "all",
    project: emptyToUndefined(cachedInput?.filter?.project ?? ""),
    provider: emptyToUndefined(cachedInput?.filter?.provider ?? ""),
    model: emptyToUndefined(cachedInput?.filter?.model ?? ""),
    source: emptyToUndefined(cachedInput?.filter?.source ?? ""),
    search: emptyToUndefined(cachedInput?.filter?.search ?? ""),
  } as SessionListFilter,
  selectedEdit: {
    provider: cachedInput?.selectedEdit?.provider ?? "",
    project: cachedInput?.selectedEdit?.project ?? "",
    titlePrefix: cachedInput?.selectedEdit?.titlePrefix ?? "",
  },
  detailEdit: blankDetailEdit(),
  sessions: [] as SessionSummary[],
  selectedIds: new Set<string>(),
  repairItems: [] as DatabaseRepairItem[],
  selectedRepairIds: new Set<string>(),
  repairBackupNote: "",
  settings: null as AppSettings | null,
  settingsDraft: null as AppSettings | null,
  settingsOpen: false,
  backupSummary: null as { sessions: number; snapshots: number; bytes: number } | null,
  backupRows: [] as SessionBackupSummary[],
  selectedSnapshotBySession: {} as Record<string, number>,
  selectedBackupSessionIds: new Set<string>(),
  restorePreview: null as RestorePreview | null,
  syncStatus: "",
  codexWasRunning: null as boolean | null,
  autoSyncInFlight: false,
  activeId: "",
  detailOpen: false,
  status: "就绪",
  dialog: null as AppDialog | null,
  busy: {
    active: false,
    label: "",
    completed: 0,
    total: 0,
    items: [],
  } as BusyState,
  // 展开状态按项目 key 保存。首次加载时会自动展开全部项目，用户操作后保持本地状态。
  expandedProjects: cachedExpandedProjects ?? new Set<string>(),
};

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("missing app root");
const appRoot = app;

interface RenderOptions {
  preserveTableScroll?: boolean;
}

function render(options: RenderOptions = {}) {
  const tableScroll = options.preserveTableScroll ? readTableScroll() : undefined;
  const groups = buildProjectGroups(state.sessions);
  const active = state.sessions.find((session) => session.id === state.activeId);
  const mainContent =
    state.activePage === "database-repair"
      ? repairTable()
      : state.activePage === "restore-backups"
        ? backupTable()
        : groupedTable(groups);

  appRoot.innerHTML = `
    <main class="shell">
      ${navigation()}
      <section class="workbench" aria-label="${escapeHtml(pageLabels[state.activePage])}">
        ${pageHeader()}
        ${filterBar()}
        ${actionBar()}
        ${mainContent}
        <div class="status">${escapeHtml(state.status)}</div>
      </section>
      ${active && state.detailOpen ? detailDrawer(active) : ""}
      ${state.settingsOpen ? settingsDrawer() : ""}
      ${state.dialog ? appDialog(state.dialog) : ""}
      ${state.busy.active ? taskProgressDialog() : ""}
    </main>
  `;
  bindEvents(groups);
  if (tableScroll) {
    restoreTableScroll(tableScroll);
  }
}

function taskProgressDialog() {
  const total = Math.max(state.busy.total, state.busy.items.length, 1);
  const completed = Math.min(state.busy.completed, total);
  const percent = Math.round((completed / total) * 100);
  const visibleItems = state.busy.items.slice(0, 10);
  const hiddenCount = Math.max(0, state.busy.items.length - visibleItems.length);
  return `
    <div class="task-backdrop" aria-hidden="true"></div>
    <section class="task-dialog" role="dialog" aria-modal="true" aria-labelledby="task-dialog-title">
      <div class="task-dialog-top">
        <div>
          <h2 id="task-dialog-title">${escapeHtml(state.busy.label)}</h2>
          <p>${completed} / ${total}</p>
        </div>
        ${state.busy.error ? `<button class="icon-button" data-close-task title="关闭任务进度">×</button>` : ""}
      </div>
      <div class="task-meter" role="progressbar" aria-valuemin="0" aria-valuemax="${total}" aria-valuenow="${completed}">
        <span style="width:${percent}%"></span>
      </div>
      <div class="task-items">
        ${visibleItems.map(taskProgressRow).join("")}
        ${hiddenCount ? `<div class="task-overflow">还有 ${hiddenCount} 项正在队列中</div>` : ""}
      </div>
      ${state.busy.error ? `<div class="task-error">${escapeHtml(state.busy.error)}</div>` : ""}
    </section>
  `;
}

function taskProgressRow(item: TaskProgressItem) {
  const label = item.status === "pending" ? "等待" : item.status === "running" ? "处理中" : item.status === "done" ? "完成" : "失败";
  return `
    <div class="task-item ${item.status}">
      <span class="task-dot" aria-hidden="true"></span>
      <div>
        <strong>${escapeHtml(item.label)}</strong>
        ${item.detail ? `<small>${escapeHtml(item.detail)}</small>` : ""}
      </div>
      <span>${label}</span>
    </div>
  `;
}

function appDialog(dialog: AppDialog) {
  return `
    <div class="dialog-backdrop" data-close-dialog></div>
    <section class="app-dialog" role="dialog" aria-modal="true" aria-labelledby="app-dialog-title">
      <h2 id="app-dialog-title">${escapeHtml(dialog.title)}</h2>
      <p>${escapeHtml(dialog.message)}</p>
      <div class="dialog-actions">
        <button class="primary" data-close-dialog>${escapeHtml(dialog.primaryLabel)}</button>
      </div>
    </section>
  `;
}

function disabledWhenBusy(disabled = false) {
  return state.busy.active || disabled ? "disabled" : "";
}

function navigation() {
  return `
    <aside class="nav">
      <div class="brand">
        <span class="brand-mark">CSM</span>
        <span>Codex 会话管理</span>
      </div>
      <nav class="page-nav" aria-label="功能页面">
        ${pageNavButton("batch-edit")}
        ${pageNavButton("session-management")}
        ${pageNavButton("restore-backups")}
        ${pageNavButton("database-repair")}
      </nav>
      ${settingsPanel()}
    </aside>
  `;
}

function pageNavButton(page: AppPage) {
  return `
    <button class="page-nav-button ${state.activePage === page ? "selected" : ""}" data-page="${page}">
      ${escapeHtml(pageLabels[page])}
    </button>
  `;
}

function settingsPanel() {
  return `
    <section class="settings-panel" aria-label="设置">
      <span class="settings-title">设置</span>
      <button class="settings-open-button" data-open-settings>备份与同步</button>
      <a class="github-link" data-open-github href="${GITHUB_REPOSITORY_URL}" title="打开 GitHub 仓库">
        <svg class="github-icon" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
          <path d="M8 0.2C3.7 0.2 0.2 3.7 0.2 8c0 3.4 2.2 6.3 5.3 7.4 0.4 0.1 0.5-0.2 0.5-0.4v-1.4c-2.2 0.5-2.6-0.9-2.6-0.9-0.4-0.9-0.9-1.1-0.9-1.1-0.7-0.5 0.1-0.5 0.1-0.5 0.8 0.1 1.2 0.8 1.2 0.8 0.7 1.2 1.9 0.9 2.3 0.7 0.1-0.5 0.3-0.9 0.5-1.1-1.7-0.2-3.5-0.9-3.5-3.9 0-0.9 0.3-1.6 0.8-2.1-0.1-0.2-0.4-1 0.1-2.1 0 0 0.7-0.2 2.2 0.8 0.6-0.2 1.3-0.3 2-0.3s1.4 0.1 2 0.3c1.5-1 2.2-0.8 2.2-0.8 0.4 1.1 0.2 1.9 0.1 2.1 0.5 0.6 0.8 1.3 0.8 2.1 0 3-1.8 3.6-3.5 3.8 0.3 0.2 0.5 0.7 0.5 1.5V15c0 0.2 0.1 0.5 0.5 0.4 3.1-1 5.3-3.9 5.3-7.4C15.8 3.7 12.3 0.2 8 0.2z" />
        </svg>
        <span>GitHub 仓库</span>
      </a>
    </section>
  `;
}

function pageHeader() {
  const description =
    state.activePage === "batch-edit"
      ? "批量修改已选会话的名称前缀、提供方和项目路径。"
      : state.activePage === "session-management"
        ? "归档、活动、置顶、压缩上下文或删除已选会话。"
        : state.activePage === "restore-backups"
          ? "按会话查看备份快照，恢复缺失或覆盖前自动创建预检备份。"
          : "预览 Codex 数据库与 JSONL 文件之间的不一致项，勾选后执行保守修复。";
  const total =
    state.activePage === "database-repair"
      ? state.repairItems.length
      : state.activePage === "restore-backups"
        ? state.backupRows.length
        : state.sessions.length;
  const selected =
    state.activePage === "database-repair"
      ? state.selectedRepairIds.size
      : state.activePage === "restore-backups"
        ? state.selectedBackupSessionIds.size
        : state.selectedIds.size;
  const totalLabel = state.activePage === "database-repair" ? "项目" : "会话";
  return `
    <header class="page-header">
      <div>
        <h1>${escapeHtml(pageLabels[state.activePage])}</h1>
        <p>${escapeHtml(description)}</p>
      </div>
      <div class="page-count">
        <strong>${total}</strong>
        <span>${totalLabel}</span>
        <strong>${selected}</strong>
        <span>已选</span>
      </div>
    </header>
  `;
}

function settingsDrawer() {
  const draft = state.settingsDraft || defaultSettings();
  const summary = state.backupSummary;
  return `
    <div class="drawer-backdrop" data-close-settings></div>
    <aside class="settings-drawer" aria-label="设置">
      <div class="drawer-top">
        <span>备份与同步设置</span>
        <button class="icon-button" data-close-settings title="关闭设置">×</button>
      </div>
      <div class="settings-form">
        <label>最大备份空间（MB）
          <input id="setting-max-mb" type="number" min="0" step="1" value="${optionalBytesToMb(draft.backup.max_bytes)}" />
        </label>
        <label>保留天数
          <input id="setting-max-age" type="number" min="0" step="1" value="${optionalNumber(draft.backup.max_age_days)}" />
        </label>
        <label>最大快照数
          <input id="setting-max-count" type="number" min="0" step="1" value="${optionalNumber(draft.backup.max_count)}" />
        </label>
        <label>最小空闲空间（MB）
          <input id="setting-min-free-mb" type="number" min="0" step="1" value="${bytesToMb(draft.backup.minimum_free_bytes)}" />
        </label>
        <label class="check-row">
          <input id="setting-skip-unique" type="checkbox" ${draft.backup.skip_unique_archive_on_auto_prune ? "checked" : ""} />
          <span>自动清理时保留缺失本地会话的唯一备份</span>
        </label>
        <label>数据库同步
          <select id="setting-sync-mode">
            <option value="never" ${draft.database_sync.mode === "never" ? "selected" : ""}>从不自动同步</option>
            <option value="auto-when-codex-stops" ${draft.database_sync.mode === "auto-when-codex-stops" ? "selected" : ""}>Codex 停止后自动同步</option>
          </select>
        </label>
        <label>Codex CLI 命令
          <input id="setting-codex-cli" placeholder="留空自动查找；Windows 优先 where.exe codex 的 codex.cmd" value="${escapeHtml(draft.codex_cli.command_path ?? "")}" />
        </label>
      </div>
      <div class="settings-summary">
        ${summary ? `${summary.sessions} 个会话备份 · ${summary.snapshots} 个快照 · ${formatBytes(summary.bytes)}` : "备份统计未加载"}
      </div>
      <div class="settings-actions">
        <button id="reload-settings" ${disabledWhenBusy()}>重新加载</button>
        <button id="save-settings" class="primary" ${disabledWhenBusy()}>保存设置</button>
      </div>
    </aside>
  `;
}

function filterBar() {
  if (state.activePage === "database-repair") {
    return repairFilterBar();
  }
  if (state.activePage === "restore-backups") {
    return backupFilterBar();
  }

  return `
    <section class="toolbar filter-toolbar" aria-label="搜索筛选">
      <div class="filter-path-row">
        <label>Codex 主目录<input id="codex-home" value="${escapeHtml(state.profile.codex_home)}" /></label>
        <button id="refresh" class="primary" ${disabledWhenBusy()}>开始扫描</button>
      </div>
      <div class="filter-grid">
        <div class="filter-status">
          <span>范围</span>
          <div class="segmented" role="group" aria-label="会话范围">
            ${archivedButton("all", "全部")}
            ${archivedButton("active", "活动")}
            ${archivedButton("archived", "已归档")}
            ${archivedButton("favorite", "收藏")}
          </div>
        </div>
        <label>项目<input id="project" value="${escapeHtml(state.filter.project ?? "")}" /></label>
        <label>提供方<input id="provider" value="${escapeHtml(state.filter.provider ?? "")}" /></label>
        <label>模型<input id="model" value="${escapeHtml(state.filter.model ?? "")}" /></label>
        <label>来源<input id="source" value="${escapeHtml(state.filter.source ?? "")}" /></label>
        <label>搜索<input id="search" value="${escapeHtml(state.filter.search ?? "")}" /></label>
      </div>
    </section>
  `;
}

function repairFilterBar() {
  return `
    <section class="toolbar repair-filter-toolbar" aria-label="数据库修复范围">
      <label>Codex 主目录<input id="codex-home" value="${escapeHtml(state.profile.codex_home)}" /></label>
      <button id="refresh" class="primary" ${disabledWhenBusy()}>扫描修复项</button>
    </section>
  `;
}

function backupFilterBar() {
  return `
    <section class="toolbar repair-filter-toolbar" aria-label="备份范围">
      <label>Codex 主目录<input id="codex-home" value="${escapeHtml(state.profile.codex_home)}" /></label>
      <button id="refresh" class="primary" ${disabledWhenBusy()}>扫描备份</button>
    </section>
  `;
}

function actionBar() {
  if (state.activePage === "database-repair") {
    return repairActionBar();
  }
  if (state.activePage === "restore-backups") {
    return backupActionBar();
  }
  return state.activePage === "batch-edit" ? batchEditBar() : sessionManagementBar();
}

function repairActionBar() {
  const applicable = state.repairItems.filter((item) => item.applicable).length;
  return `
    <section class="toolbar action-toolbar repair-action-toolbar" aria-label="数据库修复操作">
      <div class="repair-note">${escapeHtml(state.repairBackupNote || "默认仅预览。应用前会检查 Codex 是否仍在运行，并创建数据库与索引备份。")}</div>
      <div class="action-buttons">
        <button id="select-all-repairs" ${applicable === 0 ? "disabled" : ""}>全选可修复</button>
        <button id="apply-repairs" class="primary" ${disabledWhenBusy(state.selectedRepairIds.size === 0)}>应用已选修复</button>
        <button id="sync-database-local" ${disabledWhenBusy()}>按本地文件同步数据库</button>
      </div>
      <div class="sync-note">${escapeHtml(state.syncStatus)}</div>
    </section>
  `;
}

function backupActionBar() {
  const selectedCount = state.selectedBackupSessionIds.size;
  return `
    <section class="toolbar action-toolbar management-toolbar" aria-label="备份操作">
      <label class="global-select">
        <input id="select-all-backups" type="checkbox" aria-label="全选备份条目" ${allBackupRowsSelected() ? "checked" : ""} />
        <span>全选备份条目</span>
      </label>
      <button id="delete-selected-backup-groups" class="danger" ${disabledWhenBusy(selectedCount === 0)}>
        删除所选条目的所有备份
      </button>
      <span class="repair-note">红色行表示本地 JSONL 已缺失。删除这种会话最后一个快照时会要求确认。</span>
    </section>
  `;
}

function batchEditBar() {
  return `
    <section class="toolbar action-toolbar" aria-label="批量编辑操作">
      <label>会话名前缀<input id="edit-title-prefix" placeholder="多选时生成 前缀(1)" value="${escapeHtml(state.selectedEdit.titlePrefix)}" /></label>
      <label>提供方<input id="edit-provider" placeholder="留空则不改" value="${escapeHtml(state.selectedEdit.provider)}" /></label>
      <label>项目路径<input id="edit-project" placeholder="留空则不改" value="${escapeHtml(state.selectedEdit.project)}" /></label>
      <div class="action-buttons">
        <button id="preview-selected-edit" ${disabledWhenBusy()}>预览</button>
        <button id="apply-selected-edit" class="primary" ${disabledWhenBusy()}>应用</button>
      </div>
    </section>
  `;
}

function sessionManagementBar() {
  return `
    <section class="toolbar action-toolbar management-toolbar" aria-label="会话管理操作">
      <button id="archive" ${disabledWhenBusy()}>归档</button>
      <button id="active" ${disabledWhenBusy()}>活动</button>
      <button id="refresh-time" ${disabledWhenBusy()}>置顶</button>
      <button id="compact-context" ${disabledWhenBusy()}>压缩上下文</button>
      <button id="delete" class="danger" ${disabledWhenBusy()}>删除</button>
    </section>
  `;
}

function archivedButton(value: SessionScope, label: string) {
  return `<button data-archived="${value}" class="${state.filter.archived === value ? "selected" : ""}" ${disabledWhenBusy()}>${label}</button>`;
}

function groupedTable(groups: ProjectGroup<SessionSummary>[]) {
  return `
    <section class="table-shell" aria-label="会话列表">
      <div class="table session-list">
        <div class="session-list-toolbar">
          <label class="global-select">
            <input id="select-all" type="checkbox" aria-label="全选当前列表" ${allVisibleSessionsSelected() ? "checked" : ""} />
            <span>全选当前列表</span>
          </label>
          <span class="session-list-summary">按项目分组 · ${state.sessions.length} 个会话</span>
        </div>
        ${
          groups.length
            ? groups.map((group) => projectGroup(group)).join("")
            : `<div class="empty-list">没有匹配的会话</div>`
        }
      </div>
    </section>
  `;
}

function projectGroup(group: ProjectGroup<SessionSummary>) {
  const expanded = state.expandedProjects.has(group.key);
  const selectedCount = group.sessions.filter((session) => state.selectedIds.has(session.id)).length;
  const allSelected = group.sessions.length > 0 && selectedCount === group.sessions.length;

  return `
    <section class="project-group" data-project-group="${escapeHtml(group.key)}">
      <div class="project-group-header">
        <button class="project-toggle" data-toggle-project="${escapeHtml(group.key)}" aria-expanded="${expanded}">
          ${folderIcon(expanded)}
          <span class="project-title">${escapeHtml(group.project)}</span>
          <span class="project-meta">${group.sessions.length} 个会话 · 已选 ${selectedCount}</span>
        </button>
        <label class="group-select">
          <input type="checkbox" data-select-project="${escapeHtml(group.key)}" ${allSelected ? "checked" : ""} />
          组内全选
        </label>
      </div>
      ${expanded ? `<div class="session-card-grid">${group.sessions.map(sessionRow).join("")}</div>` : ""}
    </section>
  `;
}

function repairTable() {
  return `
    <section class="table-shell repair-table-shell" aria-label="数据库修复预览">
      <div class="repair-table">
        ${repairTableHeader()}
        ${
          state.repairItems.length
            ? state.repairItems.map(repairRow).join("")
            : `<div class="empty-list">暂无预览结果</div>`
        }
      </div>
    </section>
  `;
}

function backupTable() {
  return `
    <section class="table-shell repair-table-shell" aria-label="备份列表">
      <div class="backup-table">
        ${
          state.backupRows.length
            ? backupRowsWithGroups()
            : `<div class="empty-list">暂无备份快照</div>`
        }
      </div>
    </section>
  `;
}

function backupRowsWithGroups() {
  let currentGroup = "";
  return state.backupRows
    .map((row) => {
      const group = row.group || "";
      const header =
        group && group !== currentGroup
          ? `<div class="backup-group-header">${escapeHtml(group)}</div>`
          : "";
      currentGroup = group;
      return header + backupRow(row);
    })
    .join("");
}

function backupRow(row: SessionBackupSummary) {
  const snapshotIndex = normalizedSnapshotIndex(row);
  const snapshot = row.snapshots[snapshotIndex];
  const missing = !row.local_exists;
  return `
    <div class="backup-row ${missing ? "missing" : ""}">
      <label class="backup-select">
        <input type="checkbox" data-select-backup-session="${escapeHtml(row.session_id)}" ${state.selectedBackupSessionIds.has(row.session_id) ? "checked" : ""} />
      </label>
      <div class="backup-main">
        <strong title="${escapeHtml(row.title || row.session_id)}">${escapeHtml(row.title || row.session_id)}</strong>
        <span title="${escapeHtml(row.project || "")}">${escapeHtml(row.project || "")}</span>
        <code title="${escapeHtml(row.session_id)}">${escapeHtml(row.session_id)}</code>
      </div>
      <div class="backup-facts">
        <span>${missing ? "本地缺失" : "本地存在"}</span>
        <span>${row.snapshots.length} 个快照</span>
        <span>${snapshot ? formatUnixTime(snapshot.created_at_unix) : ""}</span>
        <span>${snapshot ? backupTriggerLabel(snapshot.trigger) : ""}</span>
      </div>
      <div class="backup-controls">
        <button data-backup-prev="${escapeHtml(row.session_id)}" ${snapshotIndex <= 0 ? "disabled" : ""}>上一个</button>
        <span>${row.snapshots.length ? `${snapshotIndex + 1} / ${row.snapshots.length}` : "0 / 0"}</span>
        <button data-backup-next="${escapeHtml(row.session_id)}" ${snapshotIndex >= row.snapshots.length - 1 ? "disabled" : ""}>下一个</button>
        <button data-backup-restore="${escapeHtml(row.session_id)}" ${disabledWhenBusy(!snapshot)} class="primary">恢复</button>
        <button data-backup-delete="${escapeHtml(row.session_id)}" ${disabledWhenBusy(!snapshot)} class="danger">删除快照</button>
      </div>
    </div>
  `;
}

function repairTableHeader() {
  const allSelected = allApplicableRepairsSelected();
  return `
    <div class="repair-row repair-header">
      <span class="select-header-cell">
        <input id="select-all-repair-checkbox" type="checkbox" aria-label="全选可修复项目" ${allSelected ? "checked" : ""} />
      </span>
      <span>类型</span>
      <span>会话</span>
      <span>当前值</span>
      <span>目标值</span>
      <span>状态</span>
    </div>
  `;
}

function repairRow(item: DatabaseRepairItem) {
  const selected = state.selectedRepairIds.has(item.id);
  const status = item.applicable ? "可修复" : item.skip_reason || "仅报告";
  return `
    <div class="repair-row ${item.applicable ? "" : "muted"}">
      <span>
        <input type="checkbox" data-select-repair="${escapeHtml(item.id)}" ${selected ? "checked" : ""} ${item.applicable ? "" : "disabled"} />
      </span>
      <span>${escapeHtml(repairKindLabel(item.kind))}</span>
      <span title="${escapeHtml(item.session_id)}">${escapeHtml(item.session_id)}</span>
      <span title="${escapeHtml(item.before || "")}">${escapeHtml(item.before || "")}</span>
      <span title="${escapeHtml(item.after || item.rollout_path || "")}">${escapeHtml(item.after || item.rollout_path || "")}</span>
      <span title="${escapeHtml(status)}">${escapeHtml(status)}</span>
    </div>
  `;
}

function folderIcon(expanded: boolean) {
  return expanded
    ? `<svg class="folder-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M3 8.5C3 7.7 3.7 7 4.5 7h4.4l1.8 2H20c.8 0 1.5.7 1.5 1.5l-2 7c-.2.9-.8 1.5-1.7 1.5H4.4c-.9 0-1.4-.7-1.2-1.5l1.8-7h16.1V10.5H10L8.2 8.5H3z" />
      </svg>`
    : `<svg class="folder-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M3 6.5C3 5.7 3.7 5 4.5 5h5.2l1.8 2H20c.8 0 1.5.7 1.5 1.5v9c0 .8-.7 1.5-1.5 1.5H4.5C3.7 19 3 18.3 3 17.5v-11zm1.5 0v11H20v-9h-9.2L9 6.5H4.5z" />
      </svg>`;
}

function sessionRow(session: SessionSummary) {
  const selected = state.selectedIds.has(session.id);
  const active = state.activeId === session.id && state.detailOpen;
  const stateDisplay = sessionStateDisplay(session);
  const metaItems = buildSessionMetaItems(session);
  return `
    <div class="session-card ${selected ? "selected" : ""} ${active ? "active" : ""}" role="button" tabindex="0" data-open="${escapeHtml(session.id)}">
      <input class="session-card-check" type="checkbox" data-select="${escapeHtml(session.id)}" aria-label="选择${escapeHtml(sessionTitle(session))}" ${selected ? "checked" : ""} />
      <span class="session-card-body">
        <span class="session-card-top">
          <span class="session-title" title="${escapeHtml(sessionTitle(session))}">${escapeHtml(sessionTitle(session))}</span>
          <span class="session-card-tools">
            <button class="favorite-button ${session.favorite ? "active" : ""}" data-toggle-favorite="${escapeHtml(session.id)}" title="${session.favorite ? "取消收藏" : "收藏"}" aria-label="${session.favorite ? "取消收藏" : "收藏"}" ${disabledWhenBusy()}>${session.favorite ? "★" : "☆"}</button>
            <span class="session-state session-state-${stateDisplay.tone}">${escapeHtml(stateDisplay.label)}</span>
          </span>
        </span>
        <span class="session-meta">
          ${metaItems.map((item) => `<span>${escapeHtml(item)}</span>`).join("")}
        </span>
      </span>
    </div>
  `;
}

function detailDrawer(session: SessionSummary) {
  const currentTitle = sessionTitle(session);
  const pendingTitle = detailPendingValue(session, "title");
  const dirty = detailEditDirty(session);
  return `
    <div class="drawer-backdrop" data-close-detail></div>
    <aside class="detail-drawer" aria-label="会话详情">
      <div class="drawer-top">
        <span>会话详情</span>
        <button class="icon-button" data-close-detail title="关闭详情">×</button>
      </div>
      <div class="detail-title-row">
        ${
          state.detailEdit.editingField === "title" && state.detailEdit.pendingId === session.id
            ? `<input id="detail-edit-input" class="detail-title-input" value="${escapeHtml(state.detailEdit.draft)}" />`
            : `<h2>${escapeHtml(pendingTitle || currentTitle)}</h2><button data-detail-edit="title" class="icon-button" title="重命名会话">✎</button>`
        }
      </div>
      <dl>
        <dt>ID</dt><dd>${escapeHtml(session.id)}</dd>
        ${detailEditableRow(session, "项目", "project")}
        ${detailEditableRow(session, "提供方", "provider")}
        <dt>模型</dt><dd>${escapeHtml(session.model || "")}</dd>
        <dt>来源</dt><dd>${escapeHtml(session.source || "")}</dd>
        <dt>会话文件</dt><dd>${escapeHtml(session.rollout_path || "")}</dd>
        <dt>会话索引</dt><dd>${session.in_session_index ? "存在" : "缺失"}</dd>
      </dl>
      <div class="detail-actions">
        <button id="save-detail-title" class="primary" ${disabledWhenBusy(!dirty)}>保存</button>
        <button data-toggle-favorite="${escapeHtml(session.id)}" ${disabledWhenBusy()}>${session.favorite ? "取消收藏" : "收藏"}</button>
        <button data-single-command="refresh_session_updated_at" ${disabledWhenBusy()}>置顶</button>
        <button data-compact-single ${disabledWhenBusy()}>压缩上下文</button>
        <button data-single="archive" ${disabledWhenBusy()}>归档</button>
        <button data-single="active" ${disabledWhenBusy()}>活动</button>
        <button data-single="delete" class="danger" ${disabledWhenBusy()}>删除</button>
      </div>
    </aside>
  `;
}

function bindEvents(groups: ProjectGroup<SessionSummary>[]) {
  bindPageSwitching();
  bindFilters();
  if (state.activePage === "database-repair") {
    bindRepairEvents();
  } else if (state.activePage === "restore-backups") {
    bindBackupEvents();
  } else {
    bindBatchEditInputs();
    bindGlobalSelection();
    bindGroupSelection(groups);
    bindRowEvents();
    bindDetailEvents();
  }
  bindSettingsEvents();
  bindDialogEvents();
  bindTaskDialogEvents();

  document.querySelector("#refresh")?.addEventListener("click", () => {
    state.activePage === "database-repair"
      ? refreshDatabaseRepairs()
      : state.activePage === "restore-backups"
        ? refreshBackups()
        : refresh();
  });
  document.querySelector("#preview-selected-edit")?.addEventListener("click", () => editSelected(false));
  document.querySelector("#apply-selected-edit")?.addEventListener("click", () => editSelected(true));
  document.querySelector("#archive")?.addEventListener("click", () => mutateSelected("archive_sessions"));
  document.querySelector("#active")?.addEventListener("click", () => mutateSelected("active_sessions"));
  document.querySelector("#refresh-time")?.addEventListener("click", () => mutateSelected("refresh_session_updated_at"));
  document.querySelector("#compact-context")?.addEventListener("click", compactSelected);
  document.querySelector("#delete")?.addEventListener("click", () => mutateSelected("delete_sessions"));
}

function bindDialogEvents() {
  document.querySelectorAll<HTMLElement>("[data-close-dialog]").forEach((target) => {
    target.addEventListener("click", () => {
      state.dialog = null;
      render({ preserveTableScroll: true });
    });
  });
}

function bindTaskDialogEvents() {
  document.querySelector<HTMLElement>("[data-close-task]")?.addEventListener("click", () => {
    state.busy = idleBusyState();
    render({ preserveTableScroll: true });
  });
}

function bindSettingsEvents() {
  document.querySelector<HTMLElement>("[data-open-settings]")?.addEventListener("click", () => openSettings());
  document.querySelectorAll<HTMLElement>("[data-close-settings]").forEach((target) => {
    target.addEventListener("click", () => {
      state.settingsOpen = false;
      render({ preserveTableScroll: true });
    });
  });
  bindSettingsInputs();
  document.querySelector("#reload-settings")?.addEventListener("click", () => loadAppSettings(true));
  document.querySelector("#save-settings")?.addEventListener("click", saveAppSettings);
  document.querySelector<HTMLElement>("[data-open-github]")?.addEventListener("click", (event) => {
    event.preventDefault();
    openGithubRepository();
  });
}

function bindRepairEvents() {
  const selectAll = document.querySelector<HTMLInputElement>("#select-all-repair-checkbox");
  if (selectAll) {
    selectAll.indeterminate = someApplicableRepairsSelected() && !allApplicableRepairsSelected();
    selectAll.addEventListener("change", () => toggleAllRepairs(selectAll.checked));
  }
  document.querySelector("#select-all-repairs")?.addEventListener("click", () => {
    toggleAllRepairs(true);
  });
  document.querySelector("#apply-repairs")?.addEventListener("click", applySelectedRepairs);
  document.querySelector("#sync-database-local")?.addEventListener("click", applyDatabaseSyncFromLocal);
  document.querySelectorAll<HTMLInputElement>("[data-select-repair]").forEach((checkbox) => {
    checkbox.addEventListener("change", () => {
      const id = checkbox.dataset.selectRepair || "";
      checkbox.checked ? state.selectedRepairIds.add(id) : state.selectedRepairIds.delete(id);
      render({ preserveTableScroll: true });
    });
  });
}

function bindBackupEvents() {
  const selectAll = document.querySelector<HTMLInputElement>("#select-all-backups");
  if (selectAll) {
    selectAll.indeterminate = someBackupRowsSelected() && !allBackupRowsSelected();
    selectAll.addEventListener("change", () => toggleAllBackupRows(selectAll.checked));
  }
  document
    .querySelector("#delete-selected-backup-groups")
    ?.addEventListener("click", deleteSelectedBackupGroups);
  document.querySelectorAll<HTMLInputElement>("[data-select-backup-session]").forEach((checkbox) => {
    checkbox.addEventListener("change", () => {
      const sessionId = checkbox.dataset.selectBackupSession || "";
      checkbox.checked
        ? state.selectedBackupSessionIds.add(sessionId)
        : state.selectedBackupSessionIds.delete(sessionId);
      render({ preserveTableScroll: true });
    });
  });
  document.querySelectorAll<HTMLElement>("[data-backup-prev], [data-backup-next]").forEach((button) => {
    button.addEventListener("click", () => {
      const sessionId = button.dataset.backupPrev || button.dataset.backupNext || "";
      const row = state.backupRows.find((candidate) => candidate.session_id === sessionId);
      if (!row) return;
      const current = normalizedSnapshotIndex(row);
      state.selectedSnapshotBySession[sessionId] = button.dataset.backupPrev
        ? Math.max(0, current - 1)
        : Math.min(row.snapshots.length - 1, current + 1);
      render({ preserveTableScroll: true });
    });
  });
  document.querySelectorAll<HTMLElement>("[data-backup-restore]").forEach((button) => {
    button.addEventListener("click", () => restoreSelectedBackup(button.dataset.backupRestore || ""));
  });
  document.querySelectorAll<HTMLElement>("[data-backup-delete]").forEach((button) => {
    button.addEventListener("click", () => deleteSelectedBackup(button.dataset.backupDelete || ""));
  });
}

function bindPageSwitching() {
  document.querySelectorAll<HTMLElement>("[data-page]").forEach((button) => {
    button.addEventListener("click", () => {
      state.activePage = button.dataset.page as AppPage;
      render({ preserveTableScroll: true });
      if (state.activePage === "database-repair" && state.repairItems.length === 0) {
        refreshDatabaseRepairs();
      }
      if (state.activePage === "restore-backups" && state.backupRows.length === 0) {
        refreshBackups();
      }
    });
  });
}

function bindFilters() {
  bindInput("codex-home", (value) => {
    if (state.profile.codex_home !== value) {
      state.settings = null;
      state.settingsDraft = null;
      state.backupSummary = null;
    }
    state.profile.codex_home = value;
  });
  document.querySelector<HTMLInputElement>("#codex-home")?.addEventListener("change", () => {
    void loadAppSettings(false);
  });
  bindInput("project", (value) => (state.filter.project = emptyToUndefined(value)));
  bindInput("provider", (value) => (state.filter.provider = emptyToUndefined(value)));
  bindInput("model", (value) => (state.filter.model = emptyToUndefined(value)));
  bindInput("source", (value) => (state.filter.source = emptyToUndefined(value)));
  bindInput("search", (value) => (state.filter.search = emptyToUndefined(value)));
  document.querySelectorAll<HTMLElement>("[data-archived]").forEach((button) => {
    button.addEventListener("click", () => {
      state.filter.archived = button.dataset.archived as SessionScope;
      refresh();
    });
  });
}

function bindSettingsInputs() {
  const draft = state.settingsDraft;
  if (!draft) return;
  document.querySelector<HTMLInputElement>("#setting-max-mb")?.addEventListener("input", (event) => {
    draft.backup.max_bytes = mbInputToBytes((event.target as HTMLInputElement).value);
  });
  document.querySelector<HTMLInputElement>("#setting-max-age")?.addEventListener("input", (event) => {
    draft.backup.max_age_days = optionalInteger((event.target as HTMLInputElement).value);
  });
  document.querySelector<HTMLInputElement>("#setting-max-count")?.addEventListener("input", (event) => {
    draft.backup.max_count = optionalInteger((event.target as HTMLInputElement).value);
  });
  document.querySelector<HTMLInputElement>("#setting-min-free-mb")?.addEventListener("input", (event) => {
    draft.backup.minimum_free_bytes = mbInputToBytes((event.target as HTMLInputElement).value) ?? 0;
  });
  document.querySelector<HTMLInputElement>("#setting-skip-unique")?.addEventListener("change", (event) => {
    draft.backup.skip_unique_archive_on_auto_prune = (event.target as HTMLInputElement).checked;
  });
  document.querySelector<HTMLSelectElement>("#setting-sync-mode")?.addEventListener("change", (event) => {
    draft.database_sync.mode = (event.target as HTMLSelectElement).value as DatabaseSyncMode;
  });
  document.querySelector<HTMLInputElement>("#setting-codex-cli")?.addEventListener("input", (event) => {
    const value = (event.target as HTMLInputElement).value.trim();
    draft.codex_cli.command_path = value.length > 0 ? value : null;
  });
}

function bindBatchEditInputs() {
  bindInput("edit-title-prefix", (value) => (state.selectedEdit.titlePrefix = value));
  bindInput("edit-provider", (value) => (state.selectedEdit.provider = value));
  bindInput("edit-project", (value) => (state.selectedEdit.project = value));
}

function bindGlobalSelection() {
  const selectAll = document.querySelector<HTMLInputElement>("#select-all");
  if (!selectAll) return;
  selectAll.indeterminate = someVisibleSessionsSelected() && !allVisibleSessionsSelected();
  selectAll.addEventListener("click", (event) => event.stopPropagation());
  selectAll.addEventListener("change", () => {
    if (selectAll.checked) {
      state.sessions.forEach((session) => state.selectedIds.add(session.id));
    } else {
      state.sessions.forEach((session) => state.selectedIds.delete(session.id));
    }
    render({ preserveTableScroll: true });
  });
}

function bindGroupSelection(groups: ProjectGroup<SessionSummary>[]) {
  const groupsByKey = new Map(groups.map((group) => [group.key, group]));

  document.querySelectorAll<HTMLElement>("[data-toggle-project]").forEach((button) => {
    button.addEventListener("click", () => {
      const key = button.dataset.toggleProject || "";
      state.expandedProjects.has(key) ? state.expandedProjects.delete(key) : state.expandedProjects.add(key);
      saveProjectExpansionCache(state.expandedProjects);
      render({ preserveTableScroll: true });
    });
  });

  document.querySelectorAll<HTMLInputElement>("[data-select-project]").forEach((checkbox) => {
    const group = groupsByKey.get(checkbox.dataset.selectProject || "");
    if (!group) return;
    const selectedCount = group.sessions.filter((session) => state.selectedIds.has(session.id)).length;
    checkbox.indeterminate = selectedCount > 0 && selectedCount < group.sessions.length;
    checkbox.addEventListener("click", (event) => event.stopPropagation());
    checkbox.addEventListener("change", () => {
      for (const session of group.sessions) {
        checkbox.checked ? state.selectedIds.add(session.id) : state.selectedIds.delete(session.id);
      }
      render({ preserveTableScroll: true });
    });
  });
}

function bindRowEvents() {
  document.querySelectorAll<HTMLElement>("[data-open]").forEach((row) => {
    row.addEventListener("click", () => {
      state.activeId = row.dataset.open || "";
      state.detailOpen = true;
      state.detailEdit = blankDetailEdit();
      render({ preserveTableScroll: true });
    });
    row.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        state.activeId = row.dataset.open || "";
        state.detailOpen = true;
        state.detailEdit = blankDetailEdit();
        render({ preserveTableScroll: true });
      }
    });
  });
  document.querySelectorAll<HTMLElement>("[data-toggle-favorite]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      toggleFavorite(button.dataset.toggleFavorite || "");
    });
  });
  document.querySelectorAll<HTMLInputElement>("[data-select]").forEach((checkbox) => {
    checkbox.addEventListener("click", (event) => {
      event.stopPropagation();
      const id = checkbox.dataset.select || "";
      checkbox.checked ? state.selectedIds.add(id) : state.selectedIds.delete(id);
      render({ preserveTableScroll: true });
    });
  });
}

function bindDetailEvents() {
  document.querySelectorAll<HTMLElement>("[data-close-detail]").forEach((target) => {
    target.addEventListener("click", () => {
      state.detailOpen = false;
      state.detailEdit = blankDetailEdit();
      render({ preserveTableScroll: true });
    });
  });
  document.querySelectorAll<HTMLElement>("[data-detail-edit]").forEach((button) => {
    button.addEventListener("click", () => startDetailEdit(button.dataset.detailEdit as DetailEditField));
  });
  document.querySelector("#save-detail-title")?.addEventListener("click", saveDetailEdits);
  const detailEditInput = document.querySelector<HTMLInputElement>("#detail-edit-input");
  if (detailEditInput) {
    detailEditInput.focus();
    detailEditInput.select();
    detailEditInput.addEventListener("input", () => {
      state.detailEdit.draft = detailEditInput.value;
    });
    detailEditInput.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        commitDetailEditDraft();
        render({ preserveTableScroll: true });
      }
      if (event.key === "Escape") {
        state.detailEdit.editingField = "";
        render({ preserveTableScroll: true });
      }
    });
    detailEditInput.addEventListener("blur", () => {
      commitDetailEditDraft();
      window.setTimeout(() => render({ preserveTableScroll: true }), 0);
    });
  }
  document.querySelectorAll<HTMLElement>("[data-single], [data-single-command]").forEach((button) => {
    const command = button.dataset.singleCommand || `${button.dataset.single}_sessions`;
    button.addEventListener("click", () => mutateIds(command as SessionCommand, [state.activeId]));
  });
  document.querySelector<HTMLElement>("[data-compact-single]")?.addEventListener("click", () => {
    compactIds([state.activeId]);
  });
}

function bindInput(id: string, update: (value: string) => void) {
  document.querySelector<HTMLInputElement>(`#${id}`)?.addEventListener("input", (event) => {
    update((event.target as HTMLInputElement).value);
    saveCurrentInputCache();
  });
}

function saveCurrentInputCache() {
  saveInputCache({
    codexHome: state.profile.codex_home,
    filter: {
      project: state.filter.project,
      provider: state.filter.provider,
      model: state.filter.model,
      source: state.filter.source,
      search: state.filter.search,
    },
    selectedEdit: {
      provider: state.selectedEdit.provider,
      project: state.selectedEdit.project,
      titlePrefix: state.selectedEdit.titlePrefix,
    },
  });
}

async function refresh() {
  await runWithProgress("正在加载会话", async () => {
    await loadSessions();
    state.status = "已加载会话";
  });
}

async function loadSessions(activeId?: string) {
  state.sessions = await invoke<SessionSummary[]>("list_sessions", {
    profile: state.profile,
    filter: state.filter,
  });
  state.selectedIds.clear();
  state.activeId =
    activeId && state.sessions.some((session) => session.id === activeId)
      ? activeId
      : state.sessions[0]?.id || "";
  state.detailOpen = Boolean(activeId && state.activeId);
}

async function mutateSelected(command: SessionCommand) {
  await mutateIds(command, [...state.selectedIds]);
}

async function mutateIds(command: SessionCommand, ids: string[]) {
  if (ids.length === 0) {
    state.status = "请至少选择一个会话";
    render({ preserveTableScroll: true });
    return;
  }
  if (commandRequiresCodexExit(command)) {
    const ready = await ensureCodexStoppedBefore(commandLabel(command));
    if (!ready) return;
  }
  const label = `正在${commandLabel(command)}会话`;
  await runTaskList(label, taskItemsForSessionIds(ids), async (tasks) => {
    for (const [index, id] of ids.entries()) {
      tasks.start(index, "已提交给 Rust 处理");
      const report = await invoke<SessionOperationReport>(command, { profile: state.profile, ids: [id], apply: true });
      tasks.finish(index, formatSessionOperationReport(report));
    }
    await loadSessions();
    state.status = `已${commandLabel(command)} ${ids.length} 个会话`;
  });
}

async function compactSelected() {
  await compactIds([...state.selectedIds]);
}

async function compactIds(ids: string[]) {
  const selection = singleSelectionForCodexAction(ids, "压缩上下文");
  if (!selection.ok) {
    state.status = selection.message;
    render({ preserveTableScroll: true });
    return;
  }
  if (!(await ensureCodexStoppedBefore("压缩上下文"))) {
    return;
  }

  await runWithProgress("正在压缩上下文", async () => {
    const report = await invoke<CompactReport>("compact_session", {
      profile: state.profile,
      id: selection.id,
      apply: true,
    });
    state.status = formatCompactReport(report);
    await loadSessions();
  });
}

async function editSelected(apply: boolean) {
  const ids = [...state.selectedIds];
  const provider = state.selectedEdit.provider.trim();
  const project = state.selectedEdit.project.trim();
  const titlePrefix = state.selectedEdit.titlePrefix.trim();
  if (ids.length === 0) {
    state.status = "请至少选择一个会话";
    render({ preserveTableScroll: true });
    return;
  }
  if (!provider && !project && !titlePrefix) {
    state.status = "请填写会话名前缀、提供方或项目路径";
    render({ preserveTableScroll: true });
    return;
  }
  if (apply) {
    const ready = await ensureCodexStoppedBefore("修改会话元数据");
    if (!ready) return;
  }

  await runTaskList(apply ? "正在应用批量编辑" : "正在预览批量编辑", taskItemsForSessionIds(ids), async (tasks) => {
    ids.forEach((_, index) => tasks.start(index, "等待批量编辑结果"));
    const report = await invoke<MutationReport>("edit_selected_sessions", {
      profile: state.profile,
      ids,
      edit: {
        provider: provider || null,
        project: project || null,
        titlePrefix: titlePrefix || null,
      },
      apply,
    });
    if (apply) {
      await loadSessions();
    }
    state.status = formatMutationReport(report);
    ids.forEach((_, index) => tasks.finish(index, formatMutationReport(report)));
  });
}

function detailEditableRow(session: SessionSummary, label: string, field: DetailEditField) {
  const editing = state.detailEdit.editingField === field && state.detailEdit.pendingId === session.id;
  const value = editing ? state.detailEdit.draft : detailDisplayValue(session, field);
  return `
    <dt>${escapeHtml(label)}</dt>
    <dd class="detail-editable-value">
      ${
        editing
          ? `<input id="detail-edit-input" class="detail-inline-input" value="${escapeHtml(value)}" />`
          : `<span>${escapeHtml(value)}</span><button data-detail-edit="${field}" class="icon-button" title="修改${escapeHtml(label)}">✎</button>`
      }
    </dd>
  `;
}

function startDetailEdit(field: DetailEditField) {
  const active = state.sessions.find((session) => session.id === state.activeId);
  if (!active) return;
  state.detailEdit = {
    ...state.detailEdit,
    editingField: field,
    draft: detailPendingValue(active, field) || detailCurrentValue(active, field),
    pendingId: active.id,
  };
  render({ preserveTableScroll: true });
}

function commitDetailEditDraft() {
  const active = state.sessions.find((session) => session.id === state.activeId);
  const field = state.detailEdit.editingField;
  if (!active || !field || state.detailEdit.pendingId !== active.id) return;
  const value = state.detailEdit.draft.trim() || detailCurrentValue(active, field);
  state.detailEdit.editingField = "";
  setDetailPendingValue(field, value);
}

async function saveDetailEdits() {
  const active = state.sessions.find((session) => session.id === state.activeId);
  if (!active || !detailEditDirty(active)) return;
  const title = detailPendingValue(active, "title");
  const project = detailPendingValue(active, "project");
  const provider = detailPendingValue(active, "provider");
  const ready = await ensureCodexStoppedBefore("修改会话元数据");
  if (!ready) return;
  await runWithProgress("正在修改会话", async () => {
    const report = await invoke<MutationReport>("edit_selected_sessions", {
      profile: state.profile,
      ids: [active.id],
      edit: {
        title: title || null,
        project: project || null,
        provider: provider || null,
      },
      apply: true,
    });
    await loadSessions(active.id);
    state.detailEdit = blankDetailEdit();
    state.status = formatMutationReport(report);
  });
}

function sessionTitle(session: SessionSummary) {
  return displaySessionTitle(session);
}

async function toggleFavorite(id: string) {
  if (!id) return;
  const activeId = state.activeId;
  const wasDetailOpen = state.detailOpen;
  await runWithProgress("正在更新收藏", async () => {
    await invoke("toggle_favorite", { profile: state.profile, sessionId: id });
    await loadSessions(activeId);
    state.detailOpen = wasDetailOpen && Boolean(state.activeId);
    state.status = "收藏状态已更新";
  });
}

function detailCurrentValue(session: SessionSummary, field: DetailEditField) {
  if (field === "title") return sessionTitle(session);
  if (field === "project") return session.project || "";
  return session.provider || "";
}

function detailDisplayValue(session: SessionSummary, field: DetailEditField) {
  return detailPendingValue(session, field) || detailCurrentValue(session, field);
}

function detailPendingValue(session: SessionSummary, field: DetailEditField) {
  if (state.detailEdit.pendingId !== session.id) return "";
  if (field === "title") return state.detailEdit.pendingTitle.trim();
  if (field === "project") return state.detailEdit.pendingProject.trim();
  return state.detailEdit.pendingProvider.trim();
}

function setDetailPendingValue(field: DetailEditField, value: string) {
  if (field === "title") {
    state.detailEdit.pendingTitle = value;
  } else if (field === "project") {
    state.detailEdit.pendingProject = value;
  } else {
    state.detailEdit.pendingProvider = value;
  }
}

function detailEditDirty(session: SessionSummary) {
  return (["title", "project", "provider"] as DetailEditField[]).some((field) => {
    const pending = detailPendingValue(session, field);
    return pending.length > 0 && pending !== detailCurrentValue(session, field);
  });
}

function taskItemsForSessionIds(ids: string[]) {
  return ids.map((id) => {
    const session = state.sessions.find((candidate) => candidate.id === id);
    return {
      id,
      label: session ? sessionTitle(session) : id,
      status: "pending" as TaskItemStatus,
      detail: id,
    };
  });
}

function taskItemsForRepairIds(ids: string[]) {
  return ids.map((id) => {
    const item = state.repairItems.find((candidate) => candidate.id === id);
    return {
      id,
      label: item?.summary || id,
      status: "pending" as TaskItemStatus,
      detail: item?.session_id || id,
    };
  });
}

function taskItemsForBackupSessionIds(ids: string[]) {
  return ids.map((id) => {
    const row = state.backupRows.find((candidate) => candidate.session_id === id);
    return {
      id,
      label: row?.title || id,
      status: "pending" as TaskItemStatus,
      detail: row?.project || id,
    };
  });
}

async function openGithubRepository() {
  await runWithProgress("正在打开 GitHub 仓库", async () => {
    await invoke("open_external_url", { url: GITHUB_REPOSITORY_URL });
    state.status = "已在默认浏览器打开 GitHub 仓库";
  });
}

async function openSettings() {
  state.settingsOpen = true;
  render({ preserveTableScroll: true });
  await loadAppSettings(true);
}

async function loadAppSettings(showStatus: boolean) {
  await runWithProgress(showStatus ? "正在加载设置" : "正在加载备份设置", async () => {
    const settings = await invoke<AppSettings>("load_settings", { profile: state.profile });
    state.settings = settings;
    state.settingsDraft = cloneSettings(settings);
    const backups = await invoke<SessionBackupSummary[]>("list_session_backups", {
      profile: state.profile,
    });
    state.backupSummary = summarizeBackups(backups);
    if (showStatus) {
      state.status = "设置已加载";
    }
  });
}

async function saveAppSettings() {
  if (!state.settingsDraft) return;
  await runWithProgress("正在保存设置", async () => {
    const saved = await invoke<AppSettings>("save_settings", {
      profile: state.profile,
      settings: state.settingsDraft,
    });
    state.settings = saved;
    state.settingsDraft = cloneSettings(saved);
    state.status = "设置已保存";
  });
}

async function refreshDatabaseRepairs() {
  await runWithProgress("正在预览数据库修复", async () => {
    const preview = await invoke<DatabaseRepairPreview>("preview_database_repairs", {
      profile: state.profile,
    });
    state.repairItems = preview.items;
    state.repairBackupNote = preview.backup_note;
    state.selectedRepairIds.clear();
    state.status = `已预览 ${preview.items.length} 个修复项目`;
  });
}

async function applySelectedRepairs() {
  const selected = [...state.selectedRepairIds];
  if (selected.length === 0) {
    state.status = "请至少选择一个修复项目";
    render({ preserveTableScroll: true });
    return;
  }
  const ready = await ensureCodexStoppedBefore("应用数据库修复");
  if (!ready) return;

  await runTaskList("正在应用数据库修复", taskItemsForRepairIds(selected), async (tasks) => {
    selected.forEach((_, index) => tasks.start(index, "等待修复结果"));
    const report = await invoke<DatabaseRepairApplyReport>("apply_database_repairs", {
      profile: state.profile,
      options: { selected },
    });
    state.status = formatRepairApplyReport(report);
    const preview = await invoke<DatabaseRepairPreview>("preview_database_repairs", {
      profile: state.profile,
    });
    state.repairItems = preview.items;
    state.repairBackupNote = preview.backup_note;
    state.selectedRepairIds.clear();
    selected.forEach((_, index) => tasks.finish(index, formatRepairApplyReport(report)));
  });
}

async function refreshBackups() {
  await runWithProgress("正在加载备份", async () => {
    state.backupRows = await invoke<SessionBackupSummary[]>("list_session_backups", {
      profile: state.profile,
    });
    state.backupSummary = summarizeBackups(state.backupRows);
    state.selectedBackupSessionIds.clear();
    state.restorePreview = null;
    state.status = `已加载 ${state.backupRows.length} 个会话备份`;
  });
}

async function restoreSelectedBackup(sessionId: string) {
  const snapshot = selectedSnapshot(sessionId);
  if (!snapshot) return;
  const ready = await ensureCodexStoppedBefore("恢复备份");
  if (!ready) return;
  await runWithProgress("正在恢复备份", async () => {
    const preview = await invoke<RestorePreview>("preview_restore_session_backup", {
      profile: state.profile,
      backupId: snapshot.backup_id,
    });
    state.restorePreview = preview;
    const report = await invoke<RestoreReport>("restore_session_backup", {
      profile: state.profile,
      backupId: snapshot.backup_id,
      options: {
        apply: true,
        overwrite_existing: true,
        restore_favorite: true,
      },
    });
    state.status = formatRestoreReport(report);
    state.backupRows = await invoke<SessionBackupSummary[]>("list_session_backups", {
      profile: state.profile,
    });
    await loadSessions();
  });
}

async function deleteSelectedBackup(sessionId: string) {
  const row = state.backupRows.find((candidate) => candidate.session_id === sessionId);
  const snapshot = selectedSnapshot(sessionId);
  if (!row || !snapshot) return;
  const deletingLastMissingArchive = !row.local_exists && row.snapshots.length === 1;
  if (deletingLastMissingArchive) {
    const confirmed = window.confirm(
      `这是 ${sessionId} 在本工具中的最后一个备份，且本地 JSONL 已缺失。确认永久删除这个快照？`,
    );
    if (!confirmed) return;
  }
  await runWithProgress("正在删除备份", async () => {
    await invoke("delete_session_backup", {
      profile: state.profile,
      backupId: snapshot.backup_id,
      confirmedLastArchive: deletingLastMissingArchive,
    });
    state.status = "备份快照已删除";
    state.backupRows = await invoke<SessionBackupSummary[]>("list_session_backups", {
      profile: state.profile,
    });
    state.backupSummary = summarizeBackups(state.backupRows);
  });
}

async function deleteSelectedBackupGroups() {
  const sessionIds = [...state.selectedBackupSessionIds];
  if (sessionIds.length === 0) {
    state.status = "请至少选择一个备份条目";
    render({ preserveTableScroll: true });
    return;
  }
  const selectedRows = state.backupRows.filter((row) => state.selectedBackupSessionIds.has(row.session_id));
  const includesMissingLocal = selectedRows.some((row) => !row.local_exists);
  const warning = includesMissingLocal
    ? "\n其中包含本地 JSONL 已缺失的条目。删除后这些会话可能无法再恢复。"
    : "";
  const confirmed = window.confirm(
    `删除 ${sessionIds.length} 个备份条目？\n这会永久删除这些条目下的全部快照，不能从本工具恢复。${warning}`,
  );
  if (!confirmed) return;

  await runTaskList("正在删除备份", taskItemsForBackupSessionIds(sessionIds), async (tasks) => {
    sessionIds.forEach((_, index) => tasks.start(index, "等待删除结果"));
    const report = await invoke<BackupGroupDeleteReport>("delete_session_backup_groups", {
      profile: state.profile,
      sessionIds,
      confirmedLastArchives: includesMissingLocal,
    });
    state.status = `已删除 ${report.session_ids.length} 个备份条目 · ${report.deleted_backup_ids.length} 个快照`;
    state.backupRows = await invoke<SessionBackupSummary[]>("list_session_backups", {
      profile: state.profile,
    });
    state.backupSummary = summarizeBackups(state.backupRows);
    state.selectedBackupSessionIds.clear();
    sessionIds.forEach((_, index) => tasks.finish(index, `已删除 ${report.deleted_backup_ids.length} 个快照`));
  });
}

async function applyDatabaseSyncFromLocal() {
  const ready = await ensureCodexStoppedBefore("按本地文件同步数据库");
  if (!ready) return;
  await runWithProgress("正在同步数据库", async () => {
    const report = await invoke<DatabaseRepairApplyReport>("apply_database_sync_from_local", {
      profile: state.profile,
    });
    state.syncStatus = "已按本地文件同步 SQLite";
    state.status = formatRepairApplyReport(report);
    const preview = await invoke<DatabaseRepairPreview>("preview_database_repairs", {
      profile: state.profile,
    });
    state.repairItems = preview.items;
    state.repairBackupNote = preview.backup_note;
    state.selectedRepairIds.clear();
  });
}

async function pollCodexProcess() {
  if (state.autoSyncInFlight || state.settings?.database_sync.mode !== "auto-when-codex-stops") {
    return;
  }
  try {
    const running = await invoke<boolean>("detect_codex_running");
    if (state.codexWasRunning === true && !running) {
      state.autoSyncInFlight = true;
      const report = await invoke<DatabaseRepairApplyReport>("apply_database_sync_from_local", {
        profile: state.profile,
      });
      state.syncStatus = `Codex 已停止，已同步 SQLite：${report.applied_items} 项`;
      state.status = state.syncStatus;
      if (state.activePage === "database-repair") {
        const preview = await invoke<DatabaseRepairPreview>("preview_database_repairs", {
          profile: state.profile,
        });
        state.repairItems = preview.items;
        state.repairBackupNote = preview.backup_note;
      }
      render({ preserveTableScroll: true });
    } else {
      state.syncStatus = running ? "Codex 运行中，等待停止后同步" : "Codex 未运行";
      if (state.activePage === "database-repair") {
        render({ preserveTableScroll: true });
      }
    }
    state.codexWasRunning = running;
  } catch (error) {
    state.syncStatus = `自动同步跳过：${String(error)}`;
  } finally {
    state.autoSyncInFlight = false;
  }
}

async function runWithProgress(label: string, task: () => Promise<void>) {
  const item: TaskProgressItem = { id: label, label, status: "running" };
  try {
    state.busy = { active: true, label, completed: 0, total: 1, items: [item] };
    state.status = label;
    render({ preserveTableScroll: true });
    await nextFrame();
    await task();
    state.busy.completed = 1;
    state.busy.items[0] = { ...item, status: "done" };
  } catch (error) {
    failActiveTask(String(error), 0);
  } finally {
    if (!state.busy.error) {
      state.busy = idleBusyState();
      render({ preserveTableScroll: true });
    }
  }
}

async function runTaskList(
  label: string,
  items: TaskProgressItem[],
  task: (controls: TaskListControls) => Promise<void>,
) {
  try {
    state.busy = {
      active: true,
      label,
      completed: 0,
      total: items.length,
      items: items.map((item) => ({ ...item })),
    };
    state.status = label;
    render({ preserveTableScroll: true });
    await nextFrame();
    await task({
      start: (index, detail) => updateTaskItem(index, "running", detail),
      finish: (index, detail) => {
        updateTaskItem(index, "done", detail);
        state.busy.completed = state.busy.items.filter((item) => item.status === "done").length;
        render({ preserveTableScroll: true });
      },
    });
  } catch (error) {
    const runningIndex = state.busy.items.findIndex((item) => item.status === "running");
    failActiveTask(String(error), runningIndex >= 0 ? runningIndex : undefined);
  } finally {
    if (!state.busy.error) {
      state.busy = idleBusyState();
      render({ preserveTableScroll: true });
    }
  }
}

interface TaskListControls {
  start: (index: number, detail?: string) => void;
  finish: (index: number, detail?: string) => void;
}

function updateTaskItem(index: number, status: TaskItemStatus, detail?: string) {
  const item = state.busy.items[index];
  if (!item) return;
  state.busy.items[index] = { ...item, status, detail };
  render({ preserveTableScroll: true });
}

function failActiveTask(error: string, index?: number) {
  if (typeof index === "number" && state.busy.items[index]) {
    state.busy.items[index] = { ...state.busy.items[index], status: "failed", detail: error };
  }
  state.busy.error = error;
  state.status = error;
  render({ preserveTableScroll: true });
}

function nextFrame() {
  return new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
}

function formatMutationReport(report: MutationReport) {
  return `${report.action} · ${report.applied ? "已应用" : "预览"} · SQLite ${report.sqlite_rows} 行 · JSONL ${report.jsonl_files} 个 · 索引 ${report.index_entries} 条`;
}

function formatSessionOperationReport(report: SessionOperationReport) {
  const backups = report.backup_manifests?.length ? ` · 备份 ${report.backup_manifests.length} 个` : "";
  const trash = report.trash_manifest_path ? " · 已移入回收站" : "";
  return `SQLite ${report.sqlite_rows} 行 · 索引 ${report.index_entries} 条${backups}${trash}`;
}

function formatRepairApplyReport(report: DatabaseRepairApplyReport) {
  const backup = report.backup_dir ? ` · 备份 ${report.backup_dir}` : "";
  const skipped = report.skipped_items.length ? ` · 跳过 ${report.skipped_items.length} 项` : "";
  return `已应用 ${report.applied_items} 项 · SQLite ${report.sqlite_rows} 行${skipped}${backup}`;
}

function defaultSettings(): AppSettings {
  return {
    backup: {
      max_bytes: null,
      max_age_days: null,
      max_count: null,
      skip_unique_archive_on_auto_prune: true,
      minimum_free_bytes: 536_870_912,
    },
    database_sync: {
      mode: "never",
    },
    codex_cli: {
      command_path: null,
    },
  };
}

function cloneSettings(settings: AppSettings): AppSettings {
  return JSON.parse(JSON.stringify(settings)) as AppSettings;
}

function summarizeBackups(rows: SessionBackupSummary[]) {
  return {
    sessions: rows.length,
    snapshots: rows.reduce((sum, row) => sum + row.snapshots.length, 0),
    bytes: rows.reduce(
      (sum, row) => sum + row.snapshots.reduce((snapshotSum, snapshot) => snapshotSum + snapshot.size_bytes, 0),
      0,
    ),
  };
}

function normalizedSnapshotIndex(row: SessionBackupSummary) {
  const requested = state.selectedSnapshotBySession[row.session_id] ?? 0;
  if (row.snapshots.length === 0) return 0;
  const normalized = Math.min(Math.max(requested, 0), row.snapshots.length - 1);
  state.selectedSnapshotBySession[row.session_id] = normalized;
  return normalized;
}

function selectedSnapshot(sessionId: string) {
  const row = state.backupRows.find((candidate) => candidate.session_id === sessionId);
  if (!row || row.snapshots.length === 0) return undefined;
  return row.snapshots[normalizedSnapshotIndex(row)];
}

function formatUnixTime(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "";
  return new Date(value * 1000).toLocaleString();
}

function backupTriggerLabel(trigger: BackupTrigger) {
  const labels: Record<BackupTrigger, string> = {
    delete: "删除前",
    edit: "编辑前",
    manual: "手动",
    "database-repair": "数据库修复",
    "restore-preflight": "恢复预检",
    compact: "压缩前",
  };
  return labels[trigger];
}

function formatRestoreReport(report: RestoreReport) {
  const target = report.restored_session_path ? ` · ${report.restored_session_path}` : "";
  const preflight = report.preflight_backup_manifest ? " · 已创建覆盖前备份" : "";
  const favorite = report.favorite_restored ? " · 已恢复收藏" : "";
  return `已恢复 ${report.files_restored} 个文件 · 索引 ${report.index_entries} 条 · SQLite ${report.sqlite_rows} 行${favorite}${preflight}${target}`;
}

function formatCompactReport(report: CompactReport) {
  const backup = report.backup_manifest ? ` · 备份 ${report.backup_manifest}` : "";
  const output = report.stdout.trim() || report.stderr.trim();
  const outputNote = output ? ` · ${output.slice(0, 160)}` : "";
  return `已压缩上下文 ${report.session_id}${backup}${outputNote}`;
}

function commandRequiresCodexExit(command: SessionCommand) {
  return (
    command === "archive_sessions" ||
    command === "active_sessions" ||
    command === "delete_sessions"
  );
}

function commandLabel(command: SessionCommand) {
  const labels: Record<SessionCommand, string> = {
    archive_sessions: "归档",
    active_sessions: "设为活动",
    delete_sessions: "删除",
    refresh_session_updated_at: "置顶",
  };
  return labels[command];
}

async function ensureCodexStoppedBefore(action: string) {
  try {
    const running = await invoke<boolean>("detect_codex_running");
    if (!running) return true;
    state.dialog = {
      kind: "codex-running",
      title: "Codex 正在运行",
      message: `为避免数据被同时写入，请先关闭正在使用同一份数据的 Codex 后再${action}。`,
      primaryLabel: "知道了",
    };
    render({ preserveTableScroll: true });
    return false;
  } catch (error) {
    state.status = `无法检测 Codex 运行状态：${String(error)}`;
    render({ preserveTableScroll: true });
    return false;
  }
}

function optionalNumber(value: number | null | undefined) {
  return value == null ? "" : String(value);
}

function optionalInteger(value: string) {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number.parseInt(trimmed, 10);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

function bytesToMb(value: number) {
  return Math.round(value / 1_048_576).toString();
}

function optionalBytesToMb(value: number | null | undefined) {
  return value == null ? "" : bytesToMb(value);
}

function mbInputToBytes(value: string) {
  const parsed = optionalInteger(value);
  return parsed == null ? null : parsed * 1_048_576;
}

function formatBytes(value: number) {
  if (value >= 1_073_741_824) return `${(value / 1_073_741_824).toFixed(1)} GB`;
  if (value >= 1_048_576) return `${(value / 1_048_576).toFixed(1)} MB`;
  return `${value} B`;
}

function allVisibleSessionsSelected() {
  return state.sessions.length > 0 && state.sessions.every((session) => state.selectedIds.has(session.id));
}

function someVisibleSessionsSelected() {
  return state.sessions.some((session) => state.selectedIds.has(session.id));
}

function applicableRepairItems() {
  return state.repairItems.filter((item) => item.applicable);
}

function allApplicableRepairsSelected() {
  const applicable = applicableRepairItems();
  return applicable.length > 0 && applicable.every((item) => state.selectedRepairIds.has(item.id));
}

function someApplicableRepairsSelected() {
  return applicableRepairItems().some((item) => state.selectedRepairIds.has(item.id));
}

function toggleAllRepairs(selected: boolean) {
  for (const item of applicableRepairItems()) {
    selected ? state.selectedRepairIds.add(item.id) : state.selectedRepairIds.delete(item.id);
  }
  render({ preserveTableScroll: true });
}

function allBackupRowsSelected() {
  return state.backupRows.length > 0 && state.backupRows.every((row) => state.selectedBackupSessionIds.has(row.session_id));
}

function someBackupRowsSelected() {
  return state.backupRows.some((row) => state.selectedBackupSessionIds.has(row.session_id));
}

function toggleAllBackupRows(selected: boolean) {
  for (const row of state.backupRows) {
    selected ? state.selectedBackupSessionIds.add(row.session_id) : state.selectedBackupSessionIds.delete(row.session_id);
  }
  render({ preserveTableScroll: true });
}

function repairKindLabel(kind: DatabaseRepairKind) {
  const labels: Record<DatabaseRepairKind, string> = {
    "missing-thread-row": "补 threads 行",
    "repair-rollout-path": "修 rollout_path",
    "normalize-rollout-path": "路径归一化",
    "sync-archived-state": "同步归档状态",
    "sqlite-only-thread": "删除 SQLite-only 行",
    "duplicate-jsonl": "重复 JSONL 报告",
  };
  return labels[kind];
}

function readTableScroll() {
  const table = document.querySelector<HTMLElement>(".table, .repair-table, .backup-table");
  return {
    left: table?.scrollLeft ?? 0,
    top: table?.scrollTop ?? 0,
  };
}

function restoreTableScroll(scroll: { left: number; top: number }) {
  const table = document.querySelector<HTMLElement>(".table, .repair-table, .backup-table");
  if (!table) return;
  table.scrollLeft = scroll.left;
  table.scrollTop = scroll.top;
}

function emptyToUndefined(value: string) {
  const trimmed = value.trim();
  return trimmed ? trimmed : undefined;
}

function escapeHtml(value: string) {
  return value.replace(/[&<>"']/g, (char) => {
    const map: Record<string, string> = {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      "\"": "&quot;",
      "'": "&#039;",
    };
    return map[char];
  });
}

render();
void loadAppSettings(false);
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && state.dialog) {
    state.dialog = null;
    render({ preserveTableScroll: true });
  }
});
window.setInterval(() => {
  void pollCodexProcess();
}, 30_000);
