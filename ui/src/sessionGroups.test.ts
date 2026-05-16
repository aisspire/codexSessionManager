import { buildProjectGroups, type GroupableSession } from "./sessionGroups.js";

function expectEqual<T>(actual: T, expected: T, message: string) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`);
  }
}

const sessions: GroupableSession[] = [
  { id: "a", project: "E:\\code\\alpha", sort_updated_at_ms: 20 },
  { id: "b", project: "", sort_updated_at_ms: 50 },
  { id: "c", project: "E:\\code\\alpha", sort_updated_at_ms: 80 },
  { id: "d", project: "E:\\code\\beta", sort_updated_at_ms: 100 },
  { id: "e", sort_updated_at_ms: 10 },
];

const groups = buildProjectGroups(sessions);

expectEqual(
  groups.map((group) => group.project),
  ["E:\\code\\beta", "E:\\code\\alpha", "未分组项目"],
  "groups should be sorted by the newest session in each project",
);

expectEqual(
  groups.map((group) => group.sessions.map((session) => session.id)),
  [["d"], ["c", "a"], ["b", "e"]],
  "sessions should be grouped by project and sorted newest first inside each group",
);
