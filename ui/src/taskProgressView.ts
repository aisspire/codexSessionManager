export type TaskItemStatus = "pending" | "running" | "done" | "failed";

export interface TaskProgressItem {
  id: string;
  label: string;
  status: TaskItemStatus;
  detail?: string;
}

export interface TaskProgressState {
  label: string;
  completed: number;
  total: number;
  items: TaskProgressItem[];
  error?: string;
}

export function taskProgressDialogMarkup(state: TaskProgressState) {
  const total = Math.max(state.total, state.items.length, 1);
  const completed = Math.min(state.completed, total);
  const percent = Math.round((completed / total) * 100);

  return `
    <div class="task-backdrop" aria-hidden="true"></div>
    <section class="task-dialog" role="dialog" aria-modal="true" aria-labelledby="task-dialog-title">
      <div class="task-dialog-top">
        <div>
          <h2 id="task-dialog-title">${escapeHtml(state.label)}</h2>
          <p>${completed} / ${total}</p>
        </div>
        ${state.error ? `<button class="icon-button" data-close-task title="关闭任务进度">×</button>` : ""}
      </div>
      <div class="task-meter" role="progressbar" aria-valuemin="0" aria-valuemax="${total}" aria-valuenow="${completed}">
        <span style="width:${percent}%"></span>
      </div>
      <div class="task-items">
        ${state.items.map(taskProgressRowMarkup).join("")}
      </div>
      ${state.error ? `<div class="task-error">${escapeHtml(state.error)}</div>` : ""}
    </section>
  `;
}

function taskProgressRowMarkup(item: TaskProgressItem) {
  const label =
    item.status === "pending" ? "等待" : item.status === "running" ? "处理中" : item.status === "done" ? "完成" : "失败";

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
