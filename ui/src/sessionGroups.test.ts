import { buildProjectGroups, type GroupableSession } from "./sessionGroups.js";

function expectEqual<T>(actual: T, expected: T, message: string) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`);
  }
}

const sessions: GroupableSession[] = [
  { id: "a", project: "E:\\code\\alpha" },
  { id: "b", project: "" },
  { id: "c", project: "E:\\code\\alpha" },
  { id: "d", project: "E:\\code\\beta" },
  { id: "e" },
];

const groups = buildProjectGroups(sessions);

expectEqual(
  groups.map((group) => group.project),
  ["E:\\code\\alpha", "未分组项目", "E:\\code\\beta"],
  "groups should preserve the first-seen project order",
);

expectEqual(
  groups.map((group) => group.sessions.map((session) => session.id)),
  [["a", "c"], ["b", "e"], ["d"]],
  "sessions should be grouped by project and missing projects should share the fallback group",
);
