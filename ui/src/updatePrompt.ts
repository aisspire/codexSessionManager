export type UpdatePromptState =
  | {
      kind: "available";
      version: string;
      date?: string;
      body?: string;
    }
  | {
      kind: "installing";
      version: string;
      stage: string;
      downloaded: number;
      total: number;
    }
  | {
      kind: "error";
      title: string;
      message: string;
      retryable: boolean;
    };

export interface UpdateCheckButtonState {
  checking: boolean;
}

export function updateCheckButtonMarkup(state: UpdateCheckButtonState) {
  const disabled = state.checking ? "disabled" : "";
  const label = state.checking ? "正在检查更新" : "检查更新";
  return `
    <button class="settings-open-button update-check-button" data-check-updates ${disabled}>${label}</button>
  `;
}

export function updatePromptMarkup(state: UpdatePromptState) {
  if (state.kind === "available") {
    return availableUpdateMarkup(state);
  }
  if (state.kind === "installing") {
    return installingUpdateMarkup(state);
  }
  return updateErrorMarkup(state);
}

function availableUpdateMarkup(state: Extract<UpdatePromptState, { kind: "available" }>) {
  const date = state.date ? `<p class="update-meta">发布日期：${escapeHtml(state.date)}</p>` : "";
  const notes = state.body
    ? `<pre class="update-notes">${escapeHtml(state.body)}</pre>`
    : `<p class="update-meta">这个版本没有附加更新说明。</p>`;

  return `
    <div class="dialog-backdrop" data-dismiss-update></div>
    <section class="app-dialog update-dialog" role="dialog" aria-modal="true" aria-labelledby="update-dialog-title">
      <h2 id="update-dialog-title">发现新版本 ${escapeHtml(state.version)}</h2>
      ${date}
      ${notes}
      <div class="dialog-actions update-actions">
        <button data-dismiss-update>稍后</button>
        <button class="primary" data-install-update>立即更新</button>
      </div>
    </section>
  `;
}

function installingUpdateMarkup(state: Extract<UpdatePromptState, { kind: "installing" }>) {
  const total = Math.max(state.total, 0);
  const downloaded = Math.max(Math.min(state.downloaded, total || state.downloaded), 0);
  const percent = total > 0 ? Math.round((downloaded / total) * 100) : 0;
  const detail =
    total > 0
      ? `${formatBytes(downloaded)} / ${formatBytes(total)}`
      : downloaded > 0
        ? formatBytes(downloaded)
        : "准备中";

  return `
    <div class="dialog-backdrop" aria-hidden="true"></div>
    <section class="app-dialog update-dialog" role="dialog" aria-modal="true" aria-labelledby="update-dialog-title">
      <h2 id="update-dialog-title">正在安装 ${escapeHtml(state.version)}</h2>
      <p>${escapeHtml(state.stage)} · ${escapeHtml(detail)}</p>
      <div class="update-meter" role="progressbar" aria-valuemin="0" aria-valuemax="${total || 100}" aria-valuenow="${total > 0 ? downloaded : percent}">
        <span style="width:${percent}%"></span>
      </div>
      <div class="dialog-actions update-actions">
        <button disabled>安装中</button>
      </div>
    </section>
  `;
}

function updateErrorMarkup(state: Extract<UpdatePromptState, { kind: "error" }>) {
  const retry = state.retryable ? `<button class="primary" data-retry-update-check>重试</button>` : "";
  return `
    <div class="dialog-backdrop" data-dismiss-update></div>
    <section class="app-dialog update-dialog" role="dialog" aria-modal="true" aria-labelledby="update-dialog-title">
      <h2 id="update-dialog-title">${escapeHtml(state.title)}</h2>
      <pre class="update-error-text">${escapeHtml(state.message)}</pre>
      <div class="dialog-actions update-actions">
        <button data-dismiss-update>关闭</button>
        ${retry}
      </div>
    </section>
  `;
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
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
