import { compactFailureDialogMarkup } from "./compactFailureDialog.js";

const markup = compactFailureDialogMarkup({
  sessionId: "thread-1",
  error: "codex app-server compact failed\nstderr:\nlogin required",
  copied: false,
  retryingLocal: false,
});

if (!markup.includes("压缩上下文失败")) {
  throw new Error("dialog should identify the compact failure");
}
if (!markup.includes("login required")) {
  throw new Error("dialog should include the captured error");
}
if (!markup.includes("data-copy-compact-error")) {
  throw new Error("dialog should expose a copy action");
}
if (!markup.includes("data-stop-compact-failure")) {
  throw new Error("dialog should expose a stop action");
}
if (!markup.includes("data-retry-local-compact")) {
  throw new Error("dialog should expose a local compact retry action");
}

const copied = compactFailureDialogMarkup({
  sessionId: "thread-1",
  error: "failed",
  copied: true,
  retryingLocal: false,
});

if (!copied.includes("已复制")) {
  throw new Error("copy action should keep the dialog open and show copied state");
}

const retrying = compactFailureDialogMarkup({
  sessionId: "thread-1",
  error: "failed",
  copied: false,
  retryingLocal: true,
});

if (!retrying.includes("disabled")) {
  throw new Error("retrying state should disable dialog actions");
}
