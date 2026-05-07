import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type ArchivedFilter = "active" | "archived" | "all";
type TableColumnKey = "select" | "session" | "project" | "provider" | "model" | "state" | "updated";

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
  backup_dir?: string;
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

const tableColumns: TableColumn[] = [
  { key: "select", label: "", width: 42, minWidth: 42, resizable: false },
  { key: "session", label: "会话", width: 280, minWidth: 180, resizable: true },
  { key: "project", label: "项目", width: 220, minWidth: 140, resizable: true },
  { key: "provider", label: "提供方", width: 120, minWidth: 90, resizable: true },
  { key: "model", label: "模型", width: 150, minWidth: 100, resizable: true },
  { key: "state", label: "状态", width: 110, minWidth: 86, resizable: true },
  { key: "updated", label: "更新时间", width: 190, minWidth: 140, resizable: true },
];

const state = {
  profile: {
    codex_home: "~/.codex",
    path_maps: [],
  } satisfies ProfileInput,
  filter: {
    archived: "all",
  } as SessionListFilter,
  selectedEdit: {
    provider: "",
    project: "",
    titlePrefix: "",
  },
  detailEdit: {
    editingField: "" as DetailEditField | "",
    draft: "",
    pendingId: "",
    pendingTitle: "",
    pendingProject: "",
    pendingProvider: "",
  } satisfies DetailEditState,
  sessions: [] as SessionSummary[],
  selectedIds: new Set<string>(),
  activeId: "",
  status: "就绪",
  columnWidths: tableColumns.map((column) => column.width),
};

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("missing app root");
const appRoot = app;

interface RenderOptions {
  preserveTableScroll?: boolean;
}

function render(options: RenderOptions = {}) {
  const tableScroll = options.preserveTableScroll ? readTableScroll() : undefined;
  const active = state.sessions.find((session) => session.id === state.activeId);
  appRoot.innerHTML = `
    <main class="shell">
      <aside class="filters">
        <div class="brand">Codex 会话管理</div>
        <label>Codex 主目录<input id="codex-home" value="${escapeHtml(state.profile.codex_home)}" /></label>
        <label>项目<input id="project" value="${escapeHtml(state.filter.project ?? "")}" /></label>
        <label>提供方<input id="provider" value="${escapeHtml(state.filter.provider ?? "")}" /></label>
        <label>模型<input id="model" value="${escapeHtml(state.filter.model ?? "")}" /></label>
        <label>来源<input id="source" value="${escapeHtml(state.filter.source ?? "")}" /></label>
        <label>搜索<input id="search" value="${escapeHtml(state.filter.search ?? "")}" /></label>
        <div class="segmented" role="group">
          ${archivedButton("all", "全部")}
          ${archivedButton("active", "活动")}
          ${archivedButton("archived", "已归档")}
        </div>
        <button id="refresh" class="primary">刷新</button>
        <div class="edit-panel">
          <div class="edit-title">修改已选</div>
          <label>会话名前缀<input id="edit-title-prefix" placeholder="多选时生成 前缀(1)" value="${escapeHtml(state.selectedEdit.titlePrefix)}" /></label>
          <label>提供方<input id="edit-provider" placeholder="留空则不改" value="${escapeHtml(state.selectedEdit.provider)}" /></label>
          <label>项目路径<input id="edit-project" placeholder="留空则不改" value="${escapeHtml(state.selectedEdit.project)}" /></label>
          <div class="edit-actions">
            <button id="preview-selected-edit">预览</button>
            <button id="apply-selected-edit" class="primary">应用</button>
          </div>
        </div>
      </aside>
      <section class="workbench">
        <div class="toolbar">
          <div>${state.sessions.length} 个会话 · 已选 ${state.selectedIds.size} 个</div>
          <button id="probe" title="探测 app-server">探测</button>
          <button id="backup" title="创建备份">备份</button>
          <button id="archive" title="归档已选会话">归档</button>
          <button id="restore" title="恢复已选会话">恢复</button>
          <button id="refresh-time" title="将已选会话更新时间改为当前时间">置顶</button>
          <button id="delete" class="danger" title="将已选会话移入回收站">删除</button>
        </div>
        <div class="table" style="${tableSizingStyle()}">
          ${tableHeader()}
          ${state.sessions.map(sessionRow).join("")}
        </div>
        <div class="status">${escapeHtml(state.status)}</div>
      </section>
      <aside class="details">
        ${active ? detailPanel(active) : "<div class=\"empty\">请选择一个会话</div>"}
      </aside>
    </main>
  `;
  bindEvents();
  if (tableScroll) {
    restoreTableScroll(tableScroll);
  }
}

function archivedButton(value: ArchivedFilter, label: string) {
  return `<button data-archived="${value}" class="${state.filter.archived === value ? "selected" : ""}">${label}</button>`;
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

function sessionRow(session: SessionSummary) {
  const selected = state.selectedIds.has(session.id);
  const active = state.activeId === session.id;
  return `
    <button class="row ${active ? "active" : ""}" data-open="${escapeHtml(session.id)}">
      <input type="checkbox" data-select="${escapeHtml(session.id)}" ${selected ? "checked" : ""} />
      <span>${escapeHtml(session.title || session.first_user_message || session.id)}</span>
      <span>${escapeHtml(session.project || "")}</span>
      <span>${escapeHtml(session.provider || "")}</span>
      <span>${escapeHtml(session.model || "")}</span>
      <span>${session.archived ? "已归档" : "活动"}</span>
      <span>${escapeHtml(session.updated_at || "")}</span>
    </button>
  `;
}

function detailPanel(session: SessionSummary) {
  const currentTitle = sessionTitle(session);
  const pendingTitle = detailPendingValue(session, "title");
  const dirty = detailEditDirty(session);
  return `
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
      <button data-single="restore">恢复</button>
      <button data-single="delete" class="danger">删除</button>
    </div>
  `;
}

function bindEvents() {
  bindInput("codex-home", (value) => (state.profile.codex_home = value));
  bindInput("project", (value) => (state.filter.project = emptyToUndefined(value)));
  bindInput("provider", (value) => (state.filter.provider = emptyToUndefined(value)));
  bindInput("model", (value) => (state.filter.model = emptyToUndefined(value)));
  bindInput("source", (value) => (state.filter.source = emptyToUndefined(value)));
  bindInput("search", (value) => (state.filter.search = emptyToUndefined(value)));
  bindInput("edit-title-prefix", (value) => (state.selectedEdit.titlePrefix = value));
  bindInput("edit-provider", (value) => (state.selectedEdit.provider = value));
  bindInput("edit-project", (value) => (state.selectedEdit.project = value));
  document.querySelector("#refresh")?.addEventListener("click", refresh);
  const selectAll = document.querySelector<HTMLInputElement>("#select-all");
  if (selectAll) {
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
  document.querySelector("#preview-selected-edit")?.addEventListener("click", () => editSelected(false));
  document.querySelector("#apply-selected-edit")?.addEventListener("click", () => editSelected(true));
  document.querySelector("#archive")?.addEventListener("click", () => mutateSelected("archive_sessions"));
  document.querySelector("#restore")?.addEventListener("click", () => mutateSelected("restore_sessions"));
  document.querySelector("#refresh-time")?.addEventListener("click", () => mutateSelected("refresh_session_updated_at"));
  document.querySelector("#delete")?.addEventListener("click", () => mutateSelected("delete_sessions"));
  document.querySelector("#backup")?.addEventListener("click", createBackup);
  document.querySelector("#probe")?.addEventListener("click", probe);
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
    });
    detailEditInput.addEventListener("blur", () => {
      commitDetailEditDraft();
      window.setTimeout(() => render({ preserveTableScroll: true }), 0);
    });
  }
  document.querySelectorAll<HTMLElement>("[data-archived]").forEach((button) => {
    button.addEventListener("click", () => {
      state.filter.archived = button.dataset.archived as ArchivedFilter;
      refresh();
    });
  });
  document.querySelectorAll<HTMLElement>("[data-open]").forEach((row) => {
    row.addEventListener("click", () => {
      state.activeId = row.dataset.open || "";
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
  document.querySelectorAll<HTMLElement>("[data-single], [data-single-command]").forEach((button) => {
    const command = button.dataset.singleCommand || `${button.dataset.single}_sessions`;
    button.addEventListener("click", () => mutateIds(command, [state.activeId]));
  });
  bindColumnResize();
}

function bindInput(id: string, update: (value: string) => void) {
  document.querySelector<HTMLInputElement>(`#${id}`)?.addEventListener("change", (event) => {
    update((event.target as HTMLInputElement).value);
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
    state.status = "已加载会话";
  });
}

async function mutateSelected(command: string) {
  await mutateIds(command, [...state.selectedIds]);
}

async function mutateIds(command: string, ids: string[]) {
  if (ids.length === 0) {
    state.status = "请至少选择一个会话";
    render();
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
    render();
    return;
  }
  if (!provider && !project && !titlePrefix) {
    state.status = "请填写会话名前缀、提供方或项目路径";
    render({ preserveTableScroll: true });
    return;
  }
  if (apply && !window.confirm(`将修改 ${ids.length} 个已选会话，并在写入前创建备份。继续？`)) {
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
    state.detailEdit = {
      editingField: "" as DetailEditField | "",
      draft: "",
      pendingId: "",
      pendingTitle: "",
      pendingProject: "",
      pendingProvider: "",
    } satisfies DetailEditState;
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

async function createBackup() {
  await run(async () => {
    const report = await invoke("create_backup", { profile: state.profile, includeSessions: false });
    state.status = JSON.stringify(report);
  });
}

async function probe() {
  const endpoint = window.prompt("App-server 端点", "http://127.0.0.1:0");
  if (!endpoint) return;
  await run(async () => {
    const report = await invoke("app_server_probe", { profile: state.profile, endpoint });
    state.status = JSON.stringify(report);
  });
}

async function run(task: () => Promise<void>) {
  try {
    state.status = "正在处理...";
    render();
    await task();
  } catch (error) {
    state.status = String(error);
  } finally {
    render();
  }
}

function formatMutationReport(report: MutationReport) {
  const backup = report.backup_dir ? ` · 备份 ${report.backup_dir}` : "";
  return `${report.action} · ${report.applied ? "已应用" : "预览"} · SQLite ${report.sqlite_rows} 行 · JSONL ${report.jsonl_files} 个${backup}`;
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
