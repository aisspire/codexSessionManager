export interface DisplaySession {
  id: string;
  title?: string;
  first_user_message?: string;
  provider?: string;
  model?: string;
  source?: string;
  archived: boolean;
  updated_at?: string;
}

export type SessionStateTone = "active" | "archived";

export function sessionTitle(session: DisplaySession) {
  return session.title || session.first_user_message || session.id;
}

export function buildSessionMetaItems(session: DisplaySession) {
  return [
    session.provider,
    session.model,
    session.source,
    session.updated_at,
    `ID ${shortSessionId(session.id)}`,
  ].filter((item): item is string => Boolean(item));
}

export function sessionStateDisplay(session: DisplaySession): { label: string; tone: SessionStateTone } {
  return session.archived ? { label: "已归档", tone: "archived" } : { label: "活动", tone: "active" };
}

function shortSessionId(id: string) {
  return id.length > 12 ? id.slice(0, 12) : id;
}
