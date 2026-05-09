export const INPUT_CACHE_KEY = "codex-session-manager:last-inputs";

export interface InputCacheStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export interface CachedInputState {
  codexHome?: string;
  filter?: {
    project?: string;
    provider?: string;
    model?: string;
    source?: string;
    search?: string;
  };
  selectedEdit?: {
    provider?: string;
    project?: string;
    titlePrefix?: string;
  };
}

export function loadInputCache(storage = defaultStorage()): CachedInputState | null {
  if (!storage) return null;

  try {
    const raw = storage.getItem(INPUT_CACHE_KEY);
    if (!raw) return null;
    return parseCachedInputState(JSON.parse(raw));
  } catch {
    return null;
  }
}

export function saveInputCache(value: CachedInputState, storage = defaultStorage()) {
  if (!storage) return;

  try {
    storage.setItem(INPUT_CACHE_KEY, JSON.stringify(value));
  } catch {
    // Cache writes should never block the session management workflow.
  }
}

function defaultStorage(): InputCacheStorage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

function parseCachedInputState(value: unknown): CachedInputState | null {
  if (!isRecord(value)) return null;
  const filter = optionalStringRecord(value.filter, ["project", "provider", "model", "source", "search"]);
  const selectedEdit = optionalStringRecord(value.selectedEdit, ["provider", "project", "titlePrefix"]);

  return {
    codexHome: optionalString(value.codexHome),
    filter: filter ?? undefined,
    selectedEdit: selectedEdit ?? undefined,
  };
}

function optionalString(value: unknown) {
  return typeof value === "string" ? value : undefined;
}

function optionalStringRecord(value: unknown, keys: string[]) {
  if (!isRecord(value)) return null;
  const result: Record<string, string> = {};

  for (const key of keys) {
    const item = optionalString(value[key]);
    if (item !== undefined) {
      result[key] = item;
    }
  }

  return result;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
