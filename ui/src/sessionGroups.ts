export interface GroupableSession {
  id: string;
  project?: string;
  sort_updated_at_ms?: number;
}

export interface ProjectGroup<TSession extends GroupableSession> {
  key: string;
  project: string;
  sessions: TSession[];
}

export const FALLBACK_PROJECT_NAME = "未分组项目";

/**
 * 按项目路径对会话进行稳定分组。
 *
 * 项目和项目内会话都按最新修改时间倒序排列，便于优先处理最近活动。
 */
export function buildProjectGroups<TSession extends GroupableSession>(
  sessions: TSession[],
): ProjectGroup<TSession>[] {
  const groups: ProjectGroup<TSession>[] = [];
  const groupByKey = new Map<string, ProjectGroup<TSession>>();

  for (const session of sessions) {
    const project = session.project?.trim() || FALLBACK_PROJECT_NAME;
    let group = groupByKey.get(project);

    if (!group) {
      group = {
        key: project,
        project,
        sessions: [],
      };
      groupByKey.set(project, group);
      groups.push(group);
    }

    group.sessions.push(session);
  }

  for (const group of groups) {
    group.sessions.sort(compareSessionsByNewest);
  }

  return groups.sort((left, right) => {
    const byNewest = newestInGroup(right) - newestInGroup(left);
    if (byNewest !== 0) return byNewest;
    return left.project.localeCompare(right.project);
  });
}

function compareSessionsByNewest(left: GroupableSession, right: GroupableSession) {
  return sortValue(right) - sortValue(left);
}

function newestInGroup(group: ProjectGroup<GroupableSession>) {
  return group.sessions.reduce(
    (newest, session) => Math.max(newest, sortValue(session)),
    Number.NEGATIVE_INFINITY,
  );
}

function sortValue(session: GroupableSession) {
  return session.sort_updated_at_ms ?? Number.NEGATIVE_INFINITY;
}
