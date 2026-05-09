import {
  PROJECT_EXPANSION_CACHE_KEY,
  loadProjectExpansionCache,
  saveProjectExpansionCache,
  type ProjectExpansionCacheStorage,
} from "./projectExpansionCache.js";

function expectEqual<T>(actual: T, expected: T, message: string) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`);
  }
}

class MemoryStorage implements ProjectExpansionCacheStorage {
  private readonly values = new Map<string, string>();

  getItem(key: string) {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string) {
    this.values.set(key, value);
  }
}

const storage = new MemoryStorage();
saveProjectExpansionCache(new Set(["project-a", "project-b"]), storage);

expectEqual(
  JSON.parse(storage.getItem(PROJECT_EXPANSION_CACHE_KEY) ?? "{}"),
  { expandedProjectKeys: ["project-a", "project-b"] },
  "saveProjectExpansionCache should persist expanded project keys as JSON",
);

expectEqual(
  loadProjectExpansionCache(storage),
  new Set(["project-a", "project-b"]),
  "loadProjectExpansionCache should restore expanded project keys",
);

storage.setItem(
  PROJECT_EXPANSION_CACHE_KEY,
  JSON.stringify({ expandedProjectKeys: ["project-a", 42, "", "project-a"] }),
);
expectEqual(
  loadProjectExpansionCache(storage),
  new Set(["project-a"]),
  "loadProjectExpansionCache should ignore invalid and duplicate keys",
);

storage.setItem(PROJECT_EXPANSION_CACHE_KEY, "{bad json");
expectEqual(loadProjectExpansionCache(storage), null, "loadProjectExpansionCache should ignore corrupt cached JSON");
