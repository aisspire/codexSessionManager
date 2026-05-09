import {
  INPUT_CACHE_KEY,
  loadInputCache,
  saveInputCache,
  type CachedInputState,
  type InputCacheStorage,
} from "./inputCache.js";

function expectEqual<T>(actual: T, expected: T, message: string) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`);
  }
}

class MemoryStorage implements InputCacheStorage {
  private readonly values = new Map<string, string>();

  getItem(key: string) {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string) {
    this.values.set(key, value);
  }
}

const cached: CachedInputState = {
  codexHome: "D:\\codex-home",
  filter: {
    project: "E:\\code\\demo",
    provider: "cm",
    model: "gpt-5.5",
    source: "cli",
    search: "last query",
  },
  selectedEdit: {
    provider: "custom-provider",
    project: "E:\\code\\renamed",
    titlePrefix: "复盘",
  },
};

const storage = new MemoryStorage();
saveInputCache(cached, storage);

expectEqual(
  JSON.parse(storage.getItem(INPUT_CACHE_KEY) ?? "{}"),
  cached,
  "saveInputCache should persist the input state as JSON",
);

expectEqual(loadInputCache(storage), cached, "loadInputCache should restore the last input state");

storage.setItem(INPUT_CACHE_KEY, "{bad json");
expectEqual(loadInputCache(storage), null, "loadInputCache should ignore corrupt cached JSON");
