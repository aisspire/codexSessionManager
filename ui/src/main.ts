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
  providerMigration: {
    from: "codex-auto-review",
    to: "cm",
  },
  sessions: [] as SessionSummary[],
  selectedIds: new Set<string>(),
  activeId: "",
  status: "就绪",
  columnWidths: tableColumns.map((column) => column.width),
};

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("missing app root");
const appRoot = app;

function render() {
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
        <div class="migration-panel">
          <div class="migration-title">迁移提供方</div>
          <label>从<input id="provider-from" value="${escapeHtml(state.providerMigration.from)}" /></label>
          <label>到<input id="provider-to" value="${escapeHtml(state.providerMigration.to)}" /></label>
          <div class="migration-actions">
            <button id="preview-provider-migration">预览</button>
            <button id="apply-provider-migration" class="primary">应用</button>
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
}

function archivedButton(value: ArchivedFilter, label: string) {
  return `<button data-archived="${value}" class="${state.filter.archived === value ? "selected" : ""}">${label}</button>`;
}

function tableHeader() {
  const cells = tableColumns
    .map((column, index) => `
      <span class="header-cell">
        <span class="header-label">${escapeHtml(column.label)}</span>
        ${column.resizable ? `<span class="resize-handle" data-resize-column="${index}" role="separator" aria-label="调整${escapeHtml(column.label)}列宽"></span>` : ""}
      </span>
    `)
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
  return `
    <h2>${escapeHtml(session.title || session.id)}</h2>
    <dl>
      <dt>ID</dt><dd>${escapeHtml(session.id)}</dd>
      <dt>项目</dt><dd>${escapeHtml(session.project || "")}</dd>
      <dt>提供方</dt><dd>${escapeHtml(session.provider || "")}</dd>
      <dt>模型</dt><dd>${escapeHtml(session.model || "")}</dd>
      <dt>来源</dt><dd>${escapeHtml(session.source || "")}</dd>
      <dt>会话文件</dt><dd>${escapeHtml(session.rollout_path || "")}</dd>
      <dt>会话索引</dt><dd>${session.in_session_index ? "存在" : "缺失"}</dd>
    </dl>
    <div class="detail-actions">
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
  bindInput("provider-from", (value) => (state.providerMigration.from = value));
  bindInput("provider-to", (value) => (state.providerMigration.to = value));
  document.querySelector("#refresh")?.addEventListener("click", refresh);
  document.querySelector("#preview-provider-migration")?.addEventListener("click", () => migrateProvider(false));
  document.querySelector("#apply-provider-migration")?.addEventListener("click", () => migrateProvider(true));
  document.querySelector("#archive")?.addEventListener("click", () => mutateSelected("archive_sessions"));
  document.querySelector("#restore")?.addEventListener("click", () => mutateSelected("restore_sessions"));
  document.querySelector("#delete")?.addEventListener("click", () => mutateSelected("delete_sessions"));
  document.querySelector("#backup")?.addEventListener("click", createBackup);
  document.querySelector("#probe")?.addEventListener("click", probe);
  document.querySelectorAll<HTMLElement>("[data-archived]").forEach((button) => {
    button.addEventListener("click", () => {
      state.filter.archived = button.dataset.archived as ArchivedFilter;
      refresh();
    });
  });
  document.querySelectorAll<HTMLElement>("[data-open]").forEach((row) => {
    row.addEventListener("click", () => {
      state.activeId = row.dataset.open || "";
      render();
    });
  });
  document.querySelectorAll<HTMLInputElement>("[data-select]").forEach((checkbox) => {
    checkbox.addEventListener("click", (event) => {
      event.stopPropagation();
      const id = checkbox.dataset.select || "";
      checkbox.checked ? state.selectedIds.add(id) : state.selectedIds.delete(id);
      render();
    });
  });
  document.querySelectorAll<HTMLElement>("[data-single]").forEach((button) => {
    button.addEventListener("click", () => mutateIds(`${button.dataset.single}_sessions`, [state.activeId]));
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

async function migrateProvider(apply: boolean) {
  const from = state.providerMigration.from.trim();
  const to = state.providerMigration.to.trim();
  if (!from || !to) {
    state.status = "请填写来源和目标提供方";
    render();
    return;
  }
  if (apply && !window.confirm(`将 ${from} 迁移为 ${to}，并在写入前创建备份。继续？`)) {
    return;
  }

  await run(async () => {
    const report = await invoke<MutationReport>("migrate_provider", {
      profile: state.profile,
      from,
      to,
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
