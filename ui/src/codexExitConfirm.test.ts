import {
  codexExitConfirmationMessage,
  codexRunningDialogMarkup,
  codexRunningDialogState,
  commandRequiresCodexExit,
  singleSelectionForCodexAction,
} from "./codexExitConfirm.js";

function expectEqual<T>(actual: T, expected: T, message: string) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`);
  }
}

expectEqual(
  singleSelectionForCodexAction(["thread-1"], "压缩上下文"),
  { ok: true, id: "thread-1" },
  "single selection should be accepted",
);

expectEqual(
  singleSelectionForCodexAction([], "压缩上下文"),
  { ok: false, message: "请先选择一个会话再压缩上下文" },
  "empty selection should be rejected",
);

expectEqual(
  singleSelectionForCodexAction(["thread-1", "thread-2"], "压缩上下文"),
  { ok: false, message: "压缩上下文一次只能处理一个会话，请只选择一个会话" },
  "multiple selections should be rejected",
);

const message = codexExitConfirmationMessage({
  action: "压缩上下文",
  count: 1,
  backup: true,
  extra: "可能需要等待 Codex app-server 完成 thread/compact/start。",
});

if (!message.includes("请先退出正在使用同一份数据的 Codex")) {
  throw new Error("confirmation should mention exiting Codex first");
}
if (!message.includes("执行前会自动创建备份")) {
  throw new Error("confirmation should mention backup");
}
if (!message.includes("可能需要等待 Codex app-server 完成 thread/compact/start。")) {
  throw new Error("confirmation should include extra context");
}

const runningDialog = codexRunningDialogState("删除会话");
if (runningDialog.kind !== "codex-running") {
  throw new Error("running dialog should be identified as the codex running blocker");
}
if (!runningDialog.message.includes("请先关闭正在使用同一份数据的 Codex 后再删除会话")) {
  throw new Error("running dialog should describe the blocked action");
}

const runningMarkup = codexRunningDialogMarkup(runningDialog);
if (!runningMarkup.includes("app-dialog codex-running-dialog")) {
  throw new Error("running blocker should render as a styled app dialog");
}
if (!runningMarkup.includes("role=\"dialog\"")) {
  throw new Error("running blocker should expose dialog semantics");
}
if (!runningMarkup.includes("data-close-dialog")) {
  throw new Error("running blocker should provide a close action");
}

expectEqual(commandRequiresCodexExit("archive_sessions"), true, "archive should be preflighted");
expectEqual(commandRequiresCodexExit("active_sessions"), true, "unarchive should be preflighted");
expectEqual(commandRequiresCodexExit("delete_sessions"), true, "delete should be preflighted");
expectEqual(commandRequiresCodexExit("refresh_session_updated_at"), false, "refresh timestamp should not be preflighted");
