import {
  codexExitConfirmationMessage,
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
