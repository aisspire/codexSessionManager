interface CodexExitConfirmationOptions {
  action: string;
  count: number;
  backup: boolean;
  extra?: string;
}

type SingleSelectionResult =
  | { ok: true; id: string }
  | { ok: false; message: string };

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
