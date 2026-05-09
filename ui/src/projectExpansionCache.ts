export const PROJECT_EXPANSION_CACHE_KEY = "codex-session-manager:expanded-projects";

export interface ProjectExpansionCacheStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export function loadProjectExpansionCache(
  storage = defaultStorage(),
): Set<string> | null {
  if (!storage) return null;

  try {
    const raw = storage.getItem(PROJECT_EXPANSION_CACHE_KEY);
    if (!raw) return null;
    return parseProjectExpansionCache(JSON.parse(raw));
  } catch {
    return null;
  }
}

export function saveProjectExpansionCache(
  expandedProjectKeys: Set<string>,
  storage = defaultStorage(),
) {
  if (!storage) return;

  try {
    storage.setItem(
      PROJECT_EXPANSION_CACHE_KEY,
      JSON.stringify({ expandedProjectKeys: [...expandedProjectKeys] }),
    );
  } catch {
    // Cache writes should never block list interaction.
  }
}

function defaultStorage(): ProjectExpansionCacheStorage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

function parseProjectExpansionCache(value: unknown) {
  if (!isRecord(value) || !Array.isArray(value.expandedProjectKeys)) {
    return null;
  }

  return new Set(
    value.expandedProjectKeys.filter(
      (key): key is string => typeof key === "string" && key.length > 0,
    ),
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
