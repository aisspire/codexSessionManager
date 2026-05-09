import { invoke } from "@tauri-apps/api/core";
import { loadInputCache, saveInputCache } from "./inputCache";
import { loadProjectExpansionCache, saveProjectExpansionCache } from "./projectExpansionCache";
import { buildProjectGroups, type ProjectGroup } from "./sessionGroups";
import "./styles.css";

type AppPage = "batch-edit" | "session-management";
type ArchivedFilter = "active" | "archived" | "all";
type TableColumnKey = "select" | "session" | "provider" | "model" | "state" | "updated";
type SessionCommand =
  | "archive_sessions"
  | "active_sessions"
  | "delete_sessions"
  | "refresh_session_updated_at";

interface TableColumn {
  key: TableColumnKey;
  label: string;
  width: number;
  minWidth: number;
  resizable: boolean;
}

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
  updated_at?: string;
  rollout_path?: string;
  in_session_index: boolean;
}

interface SessionListFilter {
  project?: string;
  provider?: string;
  model?: string;
  source?: string;
  archived: ArchivedFilter;
  search?: string;
}

interface MutationReport {
  action: string;
  applied: boolean;
  sqlite_rows: number;
  jsonl_files: number;
  index_entries: number;
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

const pageLabels: Record<AppPage, string> = {
  "batch-edit": "批量编辑",
  "session-management": "会话管理",
};

const GITHUB_REPOSITORY_URL = "https://github.com/aisspire/codexSessionManager";

const tableColumns: TableColumn[] = [
  { key: "select", label: "", width: 46, minWidth: 46, resizable: false },
  { key: "session", label: "会话", width: 360, minWidth: 220, resizable: true },
  { key: "provider", label: "提供方", width: 130, minWidth: 96, resizable: true },
  { key: "model", label: "模型", width: 160, minWidth: 110, resizable: true },
  { key: "state", label: "状态", width: 112, minWidth: 86, resizable: true },
  { key: "updated", label: "更新时间", width: 200, minWidth: 150, resizable: true },
];

const blankDetailEdit = (): DetailEditState => ({
  editingField: "",
  draft: "",
  pendingId: "",
  pendingTitle: "",
  pendingProject: "",
  pendingProvider: "",
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
  activeId: "",
  detailOpen: false,
  status: "就绪",
  columnWidths: tableColumns.map((column) => column.width),
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

  appRoot.innerHTML = `
    <main class="shell">
      ${navigation()}
      <section class="workbench" aria-label="${escapeHtml(pageLabels[state.activePage])}">
        ${pageHeader()}
        ${filterBar()}
        ${actionBar()}
        ${groupedTable(groups)}
        <div class="status">${escapeHtml(state.status)}</div>
      </section>
      ${active && state.detailOpen ? detailDrawer(active) : ""}
    </main>
  `;
  bindEvents(groups);
  if (tableScroll) {
    restoreTableScroll(tableScroll);
  }
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
      : "归档、活动、置顶或删除已选会话。";
  return `
    <header class="page-header">
      <div>
        <h1>${escapeHtml(pageLabels[state.activePage])}</h1>
        <p>${escapeHtml(description)}</p>
      </div>
      <div class="page-count">
        <strong>${state.sessions.length}</strong>
        <span>会话</span>
        <strong>${state.selectedIds.size}</strong>
        <span>已选</span>
      </div>
    </header>
  `;
}

function filterBar() {
  return `
    <section class="toolbar filter-toolbar" aria-label="搜索筛选">
      <label>Codex 主目录<input id="codex-home" value="${escapeHtml(state.profile.codex_home)}" /></label>
      <label>项目<input id="project" value="${escapeHtml(state.filter.project ?? "")}" /></label>
      <label>提供方<input id="provider" value="${escapeHtml(state.filter.provider ?? "")}" /></label>
      <label>模型<input id="model" value="${escapeHtml(state.filter.model ?? "")}" /></label>
      <label>来源<input id="source" value="${escapeHtml(state.filter.source ?? "")}" /></label>
      <label>搜索<input id="search" value="${escapeHtml(state.filter.search ?? "")}" /></label>
      <div class="segmented" role="group" aria-label="归档状态">
        ${archivedButton("all", "全部")}
        ${archivedButton("active", "活动")}
        ${archivedButton("archived", "已归档")}
      </div>
      <button id="refresh" class="primary">刷新</button>
    </section>
  `;
}

function actionBar() {
  return state.activePage === "batch-edit" ? batchEditBar() : sessionManagementBar();
}

function batchEditBar() {
  return `
    <section class="toolbar action-toolbar" aria-label="批量编辑操作">
      <label>会话名前缀<input id="edit-title-prefix" placeholder="多选时生成 前缀(1)" value="${escapeHtml(state.selectedEdit.titlePrefix)}" /></label>
      <label>提供方<input id="edit-provider" placeholder="留空则不改" value="${escapeHtml(state.selectedEdit.provider)}" /></label>
      <label>项目路径<input id="edit-project" placeholder="留空则不改" value="${escapeHtml(state.selectedEdit.project)}" /></label>
      <div class="action-buttons">
        <button id="preview-selected-edit">预览</button>
        <button id="apply-selected-edit" class="primary">应用</button>
      </div>
    </section>
  `;
}

function sessionManagementBar() {
  return `
    <section class="toolbar action-toolbar management-toolbar" aria-label="会话管理操作">
      <button id="archive">归档</button>
      <button id="active">活动</button>
      <button id="refresh-time">置顶</button>
      <button id="delete" class="danger">删除</button>
    </section>
  `;
}

function archivedButton(value: ArchivedFilter, label: string) {
  return `<button data-archived="${value}" class="${state.filter.archived === value ? "selected" : ""}">${label}</button>`;
}

function groupedTable(groups: ProjectGroup<SessionSummary>[]) {
  return `
    <section class="table-shell" aria-label="会话列表">
      <div class="table" style="${tableSizingStyle()}">
        ${tableHeader()}
        ${
          groups.length
            ? groups.map((group) => projectGroup(group)).join("")
            : `<div class="empty-list">没有匹配的会话</div>`
        }
      </div>
    </section>
  `;
}

function tableHeader() {
  const cells = tableColumns
    .map((column, index) => {
      if (column.key === "select") {
        return `
      <span class="header-cell select-header-cell">
        <input id="select-all" type="checkbox" aria-label="全选当前列表" ${allVisibleSessionsSelected() ? "checked" : ""} />
      </span>
    `;
      }

      return `
      <span class="header-cell">
        <span class="header-label">${escapeHtml(column.label)}</span>
        ${column.resizable ? `<span class="resize-handle" data-resize-column="${index}" role="separator" aria-label="调整${escapeHtml(column.label)}列宽"></span>` : ""}
      </span>
    `;
    })
    .join("");
  return `<div class="row header">${cells}</div>`;
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
      ${expanded ? group.sessions.map(sessionRow).join("") : ""}
    </section>
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
  return `
    <button class="row session-row ${active ? "active" : ""}" data-open="${escapeHtml(session.id)}">
      <input type="checkbox" data-select="${escapeHtml(session.id)}" ${selected ? "checked" : ""} />
      <span class="session-title">${escapeHtml(sessionTitle(session))}</span>
      <span>${escapeHtml(session.provider || "")}</span>
      <span>${escapeHtml(session.model || "")}</span>
      <span>${session.archived ? "已归档" : "活动"}</span>
      <span>${escapeHtml(session.updated_at || "")}</span>
    </button>
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
        <button id="save-detail-title" class="primary" ${dirty ? "" : "disabled"}>保存</button>
        <button data-single-command="refresh_session_updated_at">置顶</button>
        <button data-single="archive">归档</button>
        <button data-single="active">活动</button>
        <button data-single="delete" class="danger">删除</button>
      </div>
    </aside>
  `;
}

function bindEvents(groups: ProjectGroup<SessionSummary>[]) {
  bindPageSwitching();
  bindFilters();
  bindBatchEditInputs();
  bindGlobalSelection();
  bindGroupSelection(groups);
  bindRowEvents();
  bindDetailEvents();
  bindColumnResize();
  bindSettingsEvents();

  document.querySelector("#refresh")?.addEventListener("click", refresh);
  document.querySelector("#preview-selected-edit")?.addEventListener("click", () => editSelected(false));
  document.querySelector("#apply-selected-edit")?.addEventListener("click", () => editSelected(true));
  document.querySelector("#archive")?.addEventListener("click", () => mutateSelected("archive_sessions"));
  document.querySelector("#active")?.addEventListener("click", () => mutateSelected("active_sessions"));
  document.querySelector("#refresh-time")?.addEventListener("click", () => mutateSelected("refresh_session_updated_at"));
  document.querySelector("#delete")?.addEventListener("click", () => mutateSelected("delete_sessions"));
}

function bindSettingsEvents() {
  document.querySelector<HTMLElement>("[data-open-github]")?.addEventListener("click", (event) => {
    event.preventDefault();
    openGithubRepository();
  });
}

function bindPageSwitching() {
  document.querySelectorAll<HTMLElement>("[data-page]").forEach((button) => {
    button.addEventListener("click", () => {
      state.activePage = button.dataset.page as AppPage;
      render({ preserveTableScroll: true });
    });
  });
}

function bindFilters() {
  bindInput("codex-home", (value) => (state.profile.codex_home = value));
  bindInput("project", (value) => (state.filter.project = emptyToUndefined(value)));
  bindInput("provider", (value) => (state.filter.provider = emptyToUndefined(value)));
  bindInput("model", (value) => (state.filter.model = emptyToUndefined(value)));
  bindInput("source", (value) => (state.filter.source = emptyToUndefined(value)));
  bindInput("search", (value) => (state.filter.search = emptyToUndefined(value)));
  document.querySelectorAll<HTMLElement>("[data-archived]").forEach((button) => {
    button.addEventListener("click", () => {
      state.filter.archived = button.dataset.archived as ArchivedFilter;
      refresh();
    });
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
  await run(async () => {
    state.sessions = await invoke<SessionSummary[]>("list_sessions", {
      profile: state.profile,
      filter: state.filter,
    });
    state.selectedIds.clear();
    state.activeId = state.sessions[0]?.id || "";
    state.detailOpen = false;
    state.status = "已加载会话";
  });
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
  await run(async () => {
    const report = await invoke(command, { profile: state.profile, ids, apply: true });
    state.status = JSON.stringify(report);
    await refresh();
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
  if (apply && !window.confirm(`将修改 ${ids.length} 个已选会话。继续？`)) {
    return;
  }

  await run(async () => {
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
      state.sessions = await invoke<SessionSummary[]>("list_sessions", {
        profile: state.profile,
        filter: state.filter,
      });
      state.selectedIds.clear();
      state.activeId = state.sessions[0]?.id || "";
      state.detailOpen = false;
    }
    state.status = formatMutationReport(report);
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
  await run(async () => {
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
    const activeId = active.id;
    state.sessions = await invoke<SessionSummary[]>("list_sessions", {
      profile: state.profile,
      filter: state.filter,
    });
    state.activeId = state.sessions.some((session) => session.id === activeId)
      ? activeId
      : state.sessions[0]?.id || "";
    state.detailOpen = Boolean(state.activeId);
    state.detailEdit = blankDetailEdit();
    state.status = formatMutationReport(report);
  });
}

function sessionTitle(session: SessionSummary) {
  return session.title || session.first_user_message || session.id;
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

async function openGithubRepository() {
  await run(async () => {
    await invoke("open_external_url", { url: GITHUB_REPOSITORY_URL });
    state.status = "已在默认浏览器打开 GitHub 仓库";
  });
}

async function run(task: () => Promise<void>) {
  try {
    state.status = "正在处理...";
    render({ preserveTableScroll: true });
    await task();
  } catch (error) {
    state.status = String(error);
  } finally {
    render({ preserveTableScroll: true });
  }
}

function formatMutationReport(report: MutationReport) {
  return `${report.action} · ${report.applied ? "已应用" : "预览"} · SQLite ${report.sqlite_rows} 行 · JSONL ${report.jsonl_files} 个 · 索引 ${report.index_entries} 条`;
}

function tableSizingStyle() {
  const grid = state.columnWidths.map((width) => `${width}px`).join(" ");
  const width = state.columnWidths.reduce((total, columnWidth) => total + columnWidth, 0);
  return `--session-grid: ${grid}; --session-table-width: ${width}px;`;
}

function applyTableSizing() {
  const table = document.querySelector<HTMLElement>(".table");
  if (!table) return;
  const grid = state.columnWidths.map((width) => `${width}px`).join(" ");
  const width = state.columnWidths.reduce((total, columnWidth) => total + columnWidth, 0);
  table.style.setProperty("--session-grid", grid);
  table.style.setProperty("--session-table-width", `${width}px`);
}

function allVisibleSessionsSelected() {
  return state.sessions.length > 0 && state.sessions.every((session) => state.selectedIds.has(session.id));
}

function someVisibleSessionsSelected() {
  return state.sessions.some((session) => state.selectedIds.has(session.id));
}

function readTableScroll() {
  const table = document.querySelector<HTMLElement>(".table");
  return {
    left: table?.scrollLeft ?? 0,
    top: table?.scrollTop ?? 0,
  };
}

function restoreTableScroll(scroll: { left: number; top: number }) {
  const table = document.querySelector<HTMLElement>(".table");
  if (!table) return;
  table.scrollLeft = scroll.left;
  table.scrollTop = scroll.top;
}

function bindColumnResize() {
  document.querySelectorAll<HTMLElement>("[data-resize-column]").forEach((handle) => {
    handle.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      const columnIndex = Number(handle.dataset.resizeColumn);
      const column = tableColumns[columnIndex];
      if (!column) return;

      const startX = event.clientX;
      const startWidth = state.columnWidths[columnIndex];
      document.body.classList.add("resizing-column");

      const onPointerMove = (moveEvent: PointerEvent) => {
        const nextWidth = Math.max(column.minWidth, startWidth + moveEvent.clientX - startX);
        state.columnWidths[columnIndex] = Math.round(nextWidth);
        applyTableSizing();
      };

      const onPointerUp = () => {
        document.body.classList.remove("resizing-column");
        document.removeEventListener("pointermove", onPointerMove);
        document.removeEventListener("pointerup", onPointerUp);
        document.removeEventListener("pointercancel", onPointerUp);
      };

      document.addEventListener("pointermove", onPointerMove);
      document.addEventListener("pointerup", onPointerUp);
      document.addEventListener("pointercancel", onPointerUp);
    });
  });
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
