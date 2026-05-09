export interface GroupableSession {
  id: string;
  project?: string;
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
 * 这里保持“首次出现”的项目顺序，而不是重新排序，是为了让后端已经按更新时间
 * 排好的结果不被前端二次打乱；同一项目下的会话也保留原始列表顺序。
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

  return groups;
}
