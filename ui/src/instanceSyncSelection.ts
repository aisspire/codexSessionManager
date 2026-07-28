export type InstanceSyncProjectSelection = string | null;

export interface InstanceSyncGroupableSession {
  id: string;
  project?: string;
  updated_at?: string;
  sort_updated_at_ms?: number;
}

export interface InstanceSyncProjectGroup<TSession extends InstanceSyncGroupableSession> {
  key: string;
  project: InstanceSyncProjectSelection;
  label: string;
  sessions: TSession[];
}

export interface InstanceSyncSessionSelection {
  projectSelections: Set<InstanceSyncProjectSelection>;
  sessionIds: Set<string>;
}

export const INSTANCE_SYNC_UNGROUPED_PROJECT_LABEL = "未分组项目";

export function normalizeInstanceSyncProject(project?: string): InstanceSyncProjectSelection {
  let normalized = project?.trim().replace(/[\\/]+/g, "/") ?? "";
  if (!normalized) return null;
  if (normalized.length >= 2 && normalized[1] === ":") {
    normalized = `${normalized[0].toLocaleLowerCase()}${normalized.slice(1)}`;
  }
  return normalized.replace(/\/+$/, "") || null;
}

export function instanceSyncProjectSelectionKey(project: InstanceSyncProjectSelection) {
  return JSON.stringify(project);
}

export function instanceSyncProjectSelectionFromKey(
  key: string,
): InstanceSyncProjectSelection | undefined {
  try {
    const parsed: unknown = JSON.parse(key);
    if (parsed === null) return null;
    return typeof parsed === "string" ? normalizeInstanceSyncProject(parsed) : undefined;
  } catch {
    return undefined;
  }
}

export function buildInstanceSyncProjectGroups<TSession extends InstanceSyncGroupableSession>(
  sessions: TSession[],
): InstanceSyncProjectGroup<TSession>[] {
  const groupByKey = new Map<string, InstanceSyncProjectGroup<TSession>>();

  for (const session of sessions) {
    const project = normalizeInstanceSyncProject(session.project);
    const key = instanceSyncProjectSelectionKey(project);
    let group = groupByKey.get(key);
    if (!group) {
      group = {
        key,
        project,
        label: project ?? INSTANCE_SYNC_UNGROUPED_PROJECT_LABEL,
        sessions: [],
      };
      groupByKey.set(key, group);
    }
    group.sessions.push(session);
  }

  const groups = [...groupByKey.values()];
  for (const group of groups) {
    group.sessions.sort(compareInstanceSyncSessionsByNewest);
  }
  return groups.sort((left, right) => {
    const newest = compareInstanceSyncSessionsByNewest(left.sessions[0], right.sessions[0]);
    if (newest !== 0) return newest;
    return left.label.localeCompare(right.label);
  });
}

export function isInstanceSyncSessionSelected(
  session: InstanceSyncGroupableSession,
  selection: InstanceSyncSessionSelection,
) {
  return (
    selection.projectSelections.has(normalizeInstanceSyncProject(session.project)) ||
    selection.sessionIds.has(session.id)
  );
}

export function selectedInstanceSyncSessionIds<TSession extends InstanceSyncGroupableSession>(
  sessions: TSession[],
  selection: InstanceSyncSessionSelection,
) {
  return sessions
    .filter((session) => isInstanceSyncSessionSelected(session, selection))
    .map((session) => session.id);
}

export function setInstanceSyncProjectSelection<TSession extends InstanceSyncGroupableSession>(
  sessions: TSession[],
  selection: InstanceSyncSessionSelection,
  project: InstanceSyncProjectSelection,
  selected: boolean,
): InstanceSyncSessionSelection {
  const projectSelections = new Set(selection.projectSelections);
  const sessionIds = new Set(selection.sessionIds);
  const projectSessionIds = sessions
    .filter((session) => normalizeInstanceSyncProject(session.project) === project)
    .map((session) => session.id);

  if (selected) {
    projectSelections.add(project);
  } else {
    projectSelections.delete(project);
  }
  projectSessionIds.forEach((id) => sessionIds.delete(id));

  return { projectSelections, sessionIds };
}

export function setInstanceSyncSessionSelection<TSession extends InstanceSyncGroupableSession>(
  sessions: TSession[],
  selection: InstanceSyncSessionSelection,
  session: TSession,
  selected: boolean,
): InstanceSyncSessionSelection {
  const projectSelections = new Set(selection.projectSelections);
  const sessionIds = new Set(selection.sessionIds);
  const project = normalizeInstanceSyncProject(session.project);

  if (projectSelections.delete(project)) {
    sessions
      .filter((candidate) => normalizeInstanceSyncProject(candidate.project) === project)
      .forEach((candidate) => sessionIds.add(candidate.id));
  }

  if (selected) {
    sessionIds.add(session.id);
  } else {
    sessionIds.delete(session.id);
  }

  return { projectSelections, sessionIds };
}

export function reconcileInstanceSyncSessionSelection<TSession extends InstanceSyncGroupableSession>(
  sessions: TSession[],
  selection: InstanceSyncSessionSelection,
): InstanceSyncSessionSelection {
  const availableProjects = new Set(sessions.map((session) => normalizeInstanceSyncProject(session.project)));
  const availableSessionIds = new Set(sessions.map((session) => session.id));
  return {
    projectSelections: new Set(
      [...selection.projectSelections].filter((project) => availableProjects.has(project)),
    ),
    sessionIds: new Set([...selection.sessionIds].filter((id) => availableSessionIds.has(id))),
  };
}

function compareInstanceSyncSessionsByNewest(
  left: InstanceSyncGroupableSession | undefined,
  right: InstanceSyncGroupableSession | undefined,
) {
  if (!left) return right ? 1 : 0;
  if (!right) return -1;
  const leftUpdatedAt = left.sort_updated_at_ms ?? Number.NEGATIVE_INFINITY;
  const rightUpdatedAt = right.sort_updated_at_ms ?? Number.NEGATIVE_INFINITY;
  if (leftUpdatedAt !== rightUpdatedAt) return rightUpdatedAt > leftUpdatedAt ? 1 : -1;

  const textTime = (right.updated_at ?? "").localeCompare(left.updated_at ?? "");
  if (textTime !== 0) return textTime;
  return left.id.localeCompare(right.id);
}
