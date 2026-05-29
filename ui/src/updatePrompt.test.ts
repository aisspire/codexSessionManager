import {
  updateCheckButtonMarkup,
  updatePromptMarkup,
  type UpdatePromptState,
} from "./updatePrompt.js";

function expectIncludes(markup: string, expected: string, message: string) {
  if (!markup.includes(expected)) {
    throw new Error(`${message}\nexpected to include: ${expected}\nactual: ${markup}`);
  }
}

const available: UpdatePromptState = {
  kind: "available",
  version: "0.4.0",
  date: "2026-05-30T00:00:00Z",
  body: "Fix <tag> & improve updater",
};

const availableMarkup = updatePromptMarkup(available);
expectIncludes(availableMarkup, "dialog-backdrop", "available prompt should use the styled dialog backdrop");
expectIncludes(availableMarkup, "app-dialog update-dialog", "available prompt should use the styled update dialog");
expectIncludes(availableMarkup, "0.4.0", "available prompt should show the update version");
expectIncludes(availableMarkup, "Fix &lt;tag&gt; &amp; improve updater", "available prompt should escape release notes");
expectIncludes(availableMarkup, "data-install-update", "available prompt should expose an install action");
expectIncludes(availableMarkup, "data-dismiss-update", "available prompt should expose a dismiss action");

const installingMarkup = updatePromptMarkup({
  kind: "installing",
  version: "0.4.0",
  stage: "Downloading",
  downloaded: 42,
  total: 100,
});
expectIncludes(installingMarkup, "app-dialog update-dialog", "installing prompt should use the styled update dialog");
expectIncludes(installingMarkup, "role=\"progressbar\"", "installing prompt should expose progress semantics");
expectIncludes(installingMarkup, "aria-valuenow=\"42\"", "installing prompt should report downloaded progress");
expectIncludes(installingMarkup, "disabled", "installing prompt should disable dialog actions");

const errorMarkup = updatePromptMarkup({
  kind: "error",
  title: "Update failed",
  message: "network <offline>",
  retryable: true,
});
expectIncludes(errorMarkup, "app-dialog update-dialog", "update errors should use the styled update dialog");
expectIncludes(errorMarkup, "network &lt;offline&gt;", "error prompt should escape failure details");
expectIncludes(errorMarkup, "data-retry-update-check", "retryable error prompt should expose a retry action");

const idleButton = updateCheckButtonMarkup({ checking: false });
expectIncludes(idleButton, "data-check-updates", "manual update button should expose a click target");

const checkingButton = updateCheckButtonMarkup({ checking: true });
expectIncludes(checkingButton, "disabled", "manual update button should be disabled while checking");
