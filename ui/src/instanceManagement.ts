export interface ManagedInstance {
  id: number;
  path: string;
  display_name?: string | null;
  available: boolean;
  added_at_unix: number;
  last_seen_at_unix: number;
}

export interface InstanceScanReport {
  added: number;
  reactivated: number;
  ignored: number;
  already_managed: number;
  skipped: number;
}

export interface InstanceSyncPlan {
  id: number;
  name: string;
  source_instance_id: number;
  target_instance_ids: number[];
  config_paths: string[][];
  created_at_unix: number;
  updated_at_unix: number;
}

export interface InstanceSyncSelection {
  sourceInstanceId: number | null;
  targetInstanceIds: number[];
  configPathKeys: string[];
  sessionIds: string[];
}

export interface InstanceSyncTargetResultLike {
  sessions_added: string[];
  sessions_skipped: string[];
  session_conflicts: Array<{ session_id: string; reason: string }>;
  config_paths_applied: number;
  error?: string | null;
}

export function instanceDisplayName(instance: ManagedInstance) {
  return instance.display_name?.trim() || instanceDefaultName(instance.path);
}

export function instanceAvailability(instance: ManagedInstance) {
  return instance.available
    ? { label: "可用", detail: "已检测到 config.toml" }
    : { label: "已失效", detail: "配置文件或实例目录已缺失" };
}

export function instanceScanSummary(report: InstanceScanReport | null) {
  if (!report) {
    return "扫描只会登记路径，不会切换当前 Codex 主目录或修改实例配置。";
  }
  const reactivated = report.reactivated ? ` · 重新登记 ${report.reactivated} 个` : "";
  const ignored = report.ignored ? ` · 永久忽略 ${report.ignored} 个` : "";
  return `最近扫描：新增 ${report.added} 个${reactivated}${ignored} · 已存在 ${report.already_managed} 个 · 跳过 ${report.skipped} 个`;
}

export function managedInstanceDeleteConfirmation(instance: ManagedInstance) {
  return `删除“${instanceDisplayName(instance)}”的登记记录？此操作不会删除文件夹或 config.toml。`;
}

export function managedInstanceIgnoreConfirmation(instance: ManagedInstance) {
  return `永久忽略“${instanceDisplayName(instance)}”的登记记录？此操作不会删除文件夹或 config.toml，且以后扫描不会自动重新添加。`;
}

export function availableInstanceSyncTargets(instances: ManagedInstance[], sourceInstanceId: number | null) {
  return instances.filter((instance) => instance.available && instance.id !== sourceInstanceId);
}

export function configPathKey(path: string[]) {
  return JSON.stringify(path);
}

export function configPathFromKey(key: string) {
  try {
    const value: unknown = JSON.parse(key);
    return Array.isArray(value) && value.every((segment) => typeof segment === "string")
      ? value
      : [];
  } catch {
    return [];
  }
}

export function applyInstanceSyncPlan(plan: InstanceSyncPlan): InstanceSyncSelection {
  return {
    sourceInstanceId: plan.source_instance_id,
    targetInstanceIds: [...plan.target_instance_ids],
    configPathKeys: plan.config_paths.map(configPathKey),
    sessionIds: [],
  };
}

export function validateInstanceSyncSelection(selection: InstanceSyncSelection) {
  if (!Number.isSafeInteger(selection.sourceInstanceId)) return "请选择源实例";
  if (selection.targetInstanceIds.length === 0) return "请至少选择一个目标实例";
  if (selection.targetInstanceIds.includes(selection.sourceInstanceId as number)) {
    return "源实例不能同时作为目标实例";
  }
  if (new Set(selection.targetInstanceIds).size !== selection.targetInstanceIds.length) {
    return "目标实例不能重复";
  }
  if (selection.sessionIds.length === 0 && selection.configPathKeys.length === 0) {
    return "请至少选择一个会话或配置项";
  }
  return null;
}

export function instanceSyncTargetSummary(target: InstanceSyncTargetResultLike) {
  if (target.error) return `失败：${target.error}`;
  const parts = [
    `新增 ${target.sessions_added.length}`,
    `相同跳过 ${target.sessions_skipped.length}`,
    `冲突 ${target.session_conflicts.length}`,
    `配置 ${target.config_paths_applied} 项`,
  ];
  return parts.join(" · ");
}

function instanceDefaultName(path: string) {
  const segments = path.replace(/[\\/]+$/, "").split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] || path;
}
