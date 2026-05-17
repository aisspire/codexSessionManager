import { taskProgressDialogMarkup } from "./taskProgressView.ts";

function expectIncludes(actual: string, expected: string, message: string) {
  if (!actual.includes(expected)) {
    throw new Error(`${message}\nmissing: ${expected}`);
  }
}

function expectNotIncludes(actual: string, unexpected: string, message: string) {
  if (actual.includes(unexpected)) {
    throw new Error(`${message}\nunexpected: ${unexpected}`);
  }
}

const manyItems = Array.from({ length: 12 }, (_, index) => ({
  id: `task-${index + 1}`,
  label: `Task ${index + 1}`,
  status: "pending" as const,
}));

const markup = taskProgressDialogMarkup({
  label: "Batch work",
  completed: 0,
  total: manyItems.length,
  items: manyItems,
});

expectIncludes(markup, "Task 1", "task progress dialog should render the first task");
expectIncludes(markup, "Task 12", "task progress dialog should render tasks beyond the old ten-item cutoff");
expectNotIncludes(markup, "还有 2 项", "task progress dialog should not hide extra tasks behind an overflow summary");
