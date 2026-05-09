import {
  buildSessionMetaItems,
  sessionStateDisplay,
  sessionTitle,
  type DisplaySession,
} from "./sessionDisplay.js";

function expectEqual<T>(actual: T, expected: T, message: string) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`);
  }
}

const titled: DisplaySession = {
  id: "session-1234567890",
  title: "整理前端层级",
  first_user_message: "fallback message",
  provider: "openai",
  model: "gpt-5.5",
  source: "desktop",
  updated_at: "2026-05-09 10:30",
  archived: false,
};

expectEqual(sessionTitle(titled), "整理前端层级", "sessionTitle should prefer explicit titles");

expectEqual(
  sessionTitle({ id: "abc", first_user_message: "第一条消息", archived: false }),
  "第一条消息",
  "sessionTitle should fall back to the first user message",
);

expectEqual(sessionTitle({ id: "abc", archived: false }), "abc", "sessionTitle should fall back to id");

expectEqual(
  buildSessionMetaItems(titled),
  ["openai", "gpt-5.5", "desktop", "2026-05-09 10:30", "ID session-1234"],
  "metadata should keep present values together and include a short id",
);

expectEqual(
  buildSessionMetaItems({ id: "abc", archived: false }),
  ["ID abc"],
  "metadata should omit empty optional values",
);

expectEqual(
  sessionStateDisplay({ id: "active", archived: false }),
  { label: "活动", tone: "active" },
  "active sessions should use the active tone",
);

expectEqual(
  sessionStateDisplay({ id: "archived", archived: true }),
  { label: "已归档", tone: "archived" },
  "archived sessions should use the archived tone",
);
