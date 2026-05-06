import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type ArchivedFilter = "active" | "archived" | "all";

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

const state = {
  profile: {
    codex_home: "~/.codex",
    path_maps: [],
  } satisfies ProfileInput,
  filter: {
    archived: "all",
  } satisfies SessionListFilter,
  sessions: [] as SessionSummary[],
  selectedIds: new Set<string>(),
  activeId: "",
  status: "Ready",
};

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("missing app root");

function render() {
  const active = state.sessions.find((session) => session.id === state.activeId);
  app.innerHTML = `
    <main class="shell">
      <aside class="filters">
        <div class="brand">Codex Sessions</div>
        <label>Codex home<input id="codex-home" value="${escapeHtml(state.profile.codex_home)}" /></label>
        <label>Project<input id="project" value="${escapeHtml(state.filter.project ?? "")}" /></label>
        <label>Provider<input id="provider" value="${escapeHtml(state.filter.provider ?? "")}" /></label>
        <label>Model<input id="model" value="${escapeHtml(state.filter.model ?? "")}" /></label>
        <label>Source<input id="source" value="${escapeHtml(state.filter.source ?? "")}" /></label>
        <label>Search<input id="search" value="${escapeHtml(state.filter.search ?? "")}" /></label>
        <div class="segmented" role="group">
          ${archivedButton("all", "All")}
          ${archivedButton("active", "Active")}
          ${archivedButton("archived", "Archived")}
        </div>
        <button id="refresh" class="primary">Refresh</button>
      </aside>
      <section class="workbench">
        <div class="toolbar">
          <div>${state.sessions.length} sessions · ${state.selectedIds.size} selected</div>
          <button id="probe" title="Run app-server probe">Probe</button>
          <button id="backup" title="Create backup">Backup</button>
          <button id="archive" title="Archive selected">Archive</button>
          <button id="restore" title="Restore selected">Restore</button>
          <button id="delete" class="danger" title="Move selected sessions to trash">Delete</button>
        </div>
        <div class="table">
          <div class="row header">
            <span></span><span>Session</span><span>Project</span><span>Provider</span><span>Model</span><span>State</span><span>Updated</span>
          </div>
          ${state.sessions.map(sessionRow).join("")}
        </div>
        <div class="status">${escapeHtml(state.status)}</div>
      </section>
      <aside class="details">
        ${active ? detailPanel(active) : "<div class=\"empty\">Select a session</div>"}
      </aside>
    </main>
  `;
  bindEvents();
}

function archivedButton(value: ArchivedFilter, label: string) {
  return `<button data-archived="${value}" class="${state.filter.archived === value ? "selected" : ""}">${label}</button>`;
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
      <span>${session.archived ? "Archived" : "Active"}</span>
      <span>${escapeHtml(session.updated_at || "")}</span>
    </button>
  `;
}

function detailPanel(session: SessionSummary) {
  return `
    <h2>${escapeHtml(session.title || session.id)}</h2>
    <dl>
      <dt>ID</dt><dd>${escapeHtml(session.id)}</dd>
      <dt>Project</dt><dd>${escapeHtml(session.project || "")}</dd>
      <dt>Provider</dt><dd>${escapeHtml(session.provider || "")}</dd>
      <dt>Model</dt><dd>${escapeHtml(session.model || "")}</dd>
      <dt>Source</dt><dd>${escapeHtml(session.source || "")}</dd>
      <dt>Rollout</dt><dd>${escapeHtml(session.rollout_path || "")}</dd>
      <dt>Session index</dt><dd>${session.in_session_index ? "Present" : "Missing"}</dd>
    </dl>
    <div class="detail-actions">
      <button data-single="archive">Archive</button>
      <button data-single="restore">Restore</button>
      <button data-single="delete" class="danger">Delete</button>
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
  document.querySelector("#refresh")?.addEventListener("click", refresh);
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
    state.status = "Loaded sessions";
  });
}

async function mutateSelected(command: string) {
  await mutateIds(command, [...state.selectedIds]);
}

async function mutateIds(command: string, ids: string[]) {
  if (ids.length === 0) {
    state.status = "Select at least one session";
    render();
    return;
  }
  await run(async () => {
    const report = await invoke(command, { profile: state.profile, ids, apply: true });
    state.status = JSON.stringify(report);
    await refresh();
  });
}

async function createBackup() {
  await run(async () => {
    const report = await invoke("create_backup", { profile: state.profile, includeSessions: false });
    state.status = JSON.stringify(report);
  });
}

async function probe() {
  const endpoint = window.prompt("App-server endpoint", "http://127.0.0.1:0");
  if (!endpoint) return;
  await run(async () => {
    const report = await invoke("app_server_probe", { profile: state.profile, endpoint });
    state.status = JSON.stringify(report);
  });
}

async function run(task: () => Promise<void>) {
  try {
    state.status = "Working...";
    render();
    await task();
  } catch (error) {
    state.status = String(error);
  } finally {
    render();
  }
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
