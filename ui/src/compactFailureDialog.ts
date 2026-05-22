export interface CompactFailureDialogState {
  sessionId: string;
  error: string;
  copied: boolean;
  retryingLocal: boolean;
}

export function compactFailureDialogMarkup(state: CompactFailureDialogState) {
  const disabled = state.retryingLocal ? "disabled" : "";
  const copyLabel = state.copied ? "已复制" : "复制报错信息";
  return `
    <div class="dialog-backdrop" aria-hidden="true"></div>
    <section class="app-dialog compact-failure-dialog" role="dialog" aria-modal="true" aria-labelledby="compact-failure-title">
      <h2 id="compact-failure-title">压缩上下文失败</h2>
      <p>会话 ${escapeHtml(state.sessionId)} 压缩失败。</p>
      <pre class="compact-error-text">${escapeHtml(state.error)}</pre>
      <div class="dialog-actions compact-failure-actions">
        <button data-copy-compact-error ${disabled}>${copyLabel}</button>
        <button data-stop-compact-failure ${disabled}>停止</button>
        <button class="primary" data-retry-local-compact ${disabled}>尝试本地压缩</button>
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
