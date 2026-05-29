interface CodexExitConfirmationOptions {
  action: string;
  count: number;
  backup: boolean;
  extra?: string;
}

export interface CodexRunningDialogState {
  kind: "codex-running";
  title: string;
  message: string;
  primaryLabel: string;
}

type SingleSelectionResult =
  | { ok: true; id: string }
  | { ok: false; message: string };

const commandsRequiringCodexExit = new Set([
  "archive_sessions",
  "active_sessions",
  "delete_sessions",
]);

export function codexExitConfirmationMessage(options: CodexExitConfirmationOptions) {
  const lines = [
    `将${options.action} ${options.count} 个会话。`,
    "请先退出正在使用同一份数据的 Codex，再继续执行。",
  ];
  if (options.backup) {
    lines.push("执行前会自动创建备份。");
  }
  if (options.extra) {
    lines.push(options.extra);
  }
  lines.push("继续？");
  return lines.join("\n");
}

export function singleSelectionForCodexAction(
  ids: string[],
  action: string,
): SingleSelectionResult {
  if (ids.length === 0) {
    return { ok: false, message: `请先选择一个会话再${action}` };
  }
  if (ids.length > 1) {
    return {
      ok: false,
      message: `${action}一次只能处理一个会话，请只选择一个会话`,
    };
  }
  return { ok: true, id: ids[0] };
}

export function commandRequiresCodexExit(command: string) {
  return commandsRequiringCodexExit.has(command);
}

export function codexRunningDialogState(action: string): CodexRunningDialogState {
  return {
    kind: "codex-running",
    title: "Codex 正在运行",
    message: `为避免数据被同时写入，请先关闭正在使用同一份数据的 Codex 后再${action}。`,
    primaryLabel: "知道了",
  };
}

export function codexRunningDialogMarkup(dialog: CodexRunningDialogState) {
  return `
    <div class="dialog-backdrop" data-close-dialog></div>
    <section class="app-dialog codex-running-dialog" role="dialog" aria-modal="true" aria-labelledby="codex-running-dialog-title">
      <h2 id="codex-running-dialog-title">${escapeHtml(dialog.title)}</h2>
      <p>${escapeHtml(dialog.message)}</p>
      <div class="dialog-actions">
        <button class="primary" data-close-dialog>${escapeHtml(dialog.primaryLabel)}</button>
      </div>
    </section>
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
