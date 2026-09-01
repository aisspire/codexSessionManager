export type InstanceRuntime =
  | { kind: "native" }
  | {
      kind: "wsl";
      distribution: string;
      user: string;
      codex_home: string;
      host_path: string;
      architecture: string;
    };

export type InstanceAvailability = "unknown" | "available" | "unavailable";

export interface ManagedInstance {
  id: number;
  path: string;
  display_name?: string | null;
  availability: InstanceAvailability;
  availability_error?: string | null;
  runtime: InstanceRuntime;
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

export interface WslDiscoveryError {
  distribution: string;
  error: string;
}

export interface WslDiscoveryReport {
  instances: ManagedInstance[];
  errors: WslDiscoveryError[];
}

export interface WslStatus {
  supported: boolean;
  installed: boolean;
  distributions: string[];
  error?: string | null;
}

export interface InstanceSyncPlan {
  id: number;
  name: string;
  source_instance_id: number;
  target_instance_ids: number[];
  config_paths: string[][];
  project_selections: Array<string | null>;
  created_at_unix: number;
  updated_at_unix: number;
}

export interface InstanceSyncSelection {
  sourceInstanceId: number | null;
  targetInstanceIds: number[];
  configPathKeys: string[];
  projectSelections: Array<string | null>;
  sessionIds: string[];
}

export const automaticNonRootDiffPlanId = -1;
export const automaticNonRootDiffPlanLabel = "自动：同步所有差异的非根配置";

export interface InstanceSyncTargetResultLike {
  sessions_added: string[];
  sessions_skipped: string[];
  session_conflicts: Array<{ session_id: string; reason: string }>;
  config_paths_applied: number;
  error?: string | null;
}

export function instanceDisplayName(instance: ManagedInstance) {
  return (
    instance.display_name?.trim() ||
    (instance.runtime.kind === "wsl"
      ? `${instance.runtime.distribution} · ${instance.runtime.user}`
      : instanceDefaultName(instance.path))
  );
}

export function instanceAvailability(instance: ManagedInstance) {
  if (instance.availability === "available") {
    return {
      label: "可用",
      detail: instance.runtime.kind === "wsl" ? "发行版内已检测到 config.toml" : "已检测到 config.toml",
    };
  }
  if (instance.availability === "unknown") {
    return {
      label: "未检测",
      detail: "尚未实时检查 WSL 发行版、用户和 config.toml",
    };
  }
  return {
    label: "不可用",
    detail: instance.availability_error || "配置文件、实例目录或 WSL 发行版不可用",
  };
}

export function instanceRuntimeLabel(instance: ManagedInstance) {
  return instance.runtime.kind === "wsl"
    ? `WSL · ${instance.runtime.distribution}`
    : "Windows 原生";
}

export function isUnsupportedManualCodexHome(path: string) {
  const normalized = path.trim().replace(/\\/g, "/").toLocaleLowerCase();
  return (
    normalized === "/mnt" ||
    normalized.startsWith("/mnt/") ||
    normalized === "//wsl.localhost" ||
    normalized.startsWith("//wsl.localhost/") ||
    normalized === "//wsl$" ||
    normalized.startsWith("//wsl$/") ||
    normalized === "//?/unc/wsl.localhost" ||
    normalized.startsWith("//?/unc/wsl.localhost/") ||
    normalized === "//?/unc/wsl$" ||
    normalized.startsWith("//?/unc/wsl$/")
  );
}

export const unsupportedManualCodexHomeMessage =
  "Windows 原生 Codex 主目录不支持 WSL UNC 或 /mnt 路径，请先登记 WSL 实例";

export interface ManualCodexHomeUpdate {
  accepted: boolean;
  value: string;
  message: string | null;
}

export function manualCodexHomeUpdate(value: string, currentValue: string): ManualCodexHomeUpdate {
  if (isUnsupportedManualCodexHome(value)) {
    return {
      accepted: false,
      value: currentValue,
      message: unsupportedManualCodexHomeMessage,
    };
  }
  return { accepted: true, value, message: null };
}

export function instanceSyncSourceCandidates(instances: ManagedInstance[]) {
  return instances.filter((instance) => instance.availability === "available");
}

export function instanceSyncInstanceLabel(instance: ManagedInstance) {
  const name = instanceDisplayName(instance);
  if (instance.runtime.kind === "wsl") {
    return `${name} · WSL · ${instance.runtime.distribution} · ${instance.runtime.user} · ${instance.runtime.codex_home}`;
  }
  return `${name} · Windows 原生 · ${instance.path}`;
}

export function instanceSyncInstancesCompatible(source: ManagedInstance, target: ManagedInstance) {
  if (source.runtime.kind === "native" || target.runtime.kind === "native") {
    return source.runtime.kind === "native" && target.runtime.kind === "native";
  }
  return (
    source.runtime.distribution.toLowerCase() === target.runtime.distribution.toLowerCase() &&
    source.runtime.user === target.runtime.user &&
    normalizeWslArchitecture(source.runtime.architecture) ===
      normalizeWslArchitecture(target.runtime.architecture) &&
    normalizeLinuxCodexHome(source.runtime.codex_home) !==
      normalizeLinuxCodexHome(target.runtime.codex_home)
  );
}

export function instanceSyncTargetFilterDescription(source: ManagedInstance | null) {
  if (!source) return "选择源实例后显示兼容目标";
  if (source.runtime.kind === "native") return "仅显示其他可用的 Windows 原生实例";
  return `仅显示 ${source.runtime.distribution} / ${source.runtime.user} 的其他 WSL 实例`;
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
  const source = instances.find((instance) => instance.id === sourceInstanceId) || null;
  if (!source) return [];
  return instanceSyncSourceCandidates(instances).filter(
    (instance) =>
      instance.id !== source.id && instanceSyncInstancesCompatible(source, instance),
  );
}

export interface ReconciledInstanceSyncAvailability {
  selection: InstanceSyncSelection;
  sourceAvailable: boolean;
  sourceRemoved: boolean;
  removedTargetCount: number;
}

export function reconcileInstanceSyncAvailability(
  instances: ManagedInstance[],
  selection: InstanceSyncSelection,
): ReconciledInstanceSyncAvailability {
  const source =
    selection.sourceInstanceId == null
      ? null
      : instanceSyncSourceCandidates(instances).find(
          (instance) => instance.id === selection.sourceInstanceId,
        ) || null;
  if (!source) {
    return {
      selection: {
        sourceInstanceId: null,
        targetInstanceIds: [],
        configPathKeys: [],
        projectSelections: [],
        sessionIds: [],
      },
      sourceAvailable: false,
      sourceRemoved: selection.sourceInstanceId != null,
      removedTargetCount: selection.targetInstanceIds.length,
    };
  }
  const compatibleTargetIds = new Set(
    availableInstanceSyncTargets(instances, source.id).map((instance) => instance.id),
  );
  const targetInstanceIds = selection.targetInstanceIds.filter((id) =>
    compatibleTargetIds.has(id),
  );
  return {
    selection: { ...selection, targetInstanceIds },
    sourceAvailable: true,
    sourceRemoved: false,
    removedTargetCount: selection.targetInstanceIds.length - targetInstanceIds.length,
  };
}

export function reconcileInstanceSyncPlan(
  plan: InstanceSyncPlan,
  instances: ManagedInstance[],
): ReconciledInstanceSyncAvailability {
  return reconcileInstanceSyncAvailability(instances, applyInstanceSyncPlan(plan));
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

export function isAutomaticNonRootDiffPlan(planId: number | null) {
  return planId === automaticNonRootDiffPlanId;
}

export function automaticNonRootDiffPlanExecutionBlockMessage(
  planId: number | null,
  selectionInFlight: boolean,
) {
  return isAutomaticNonRootDiffPlan(planId) && selectionInFlight
    ? "正在计算非根配置差异，请等待自动选择完成"
    : null;
}

export function automaticNonRootDiffConfigPathKeys(configPaths: string[][]) {
  return configPaths.map(configPathKey);
}

export function isCurrentAutomaticNonRootDiffContext(
  requestedPlanId: number | null,
  requestedSourceInstanceId: number,
  requestedTargetInstanceIds: number[],
  currentPlanId: number | null,
  currentSourceInstanceId: number | null,
  currentTargetInstanceIds: number[],
) {
  return (
    requestedPlanId === currentPlanId &&
    requestedSourceInstanceId === currentSourceInstanceId &&
    requestedTargetInstanceIds.length === currentTargetInstanceIds.length &&
    requestedTargetInstanceIds.every((targetId, index) => targetId === currentTargetInstanceIds[index])
  );
}

export function applyInstanceSyncPlan(plan: InstanceSyncPlan): InstanceSyncSelection {
  return {
    sourceInstanceId: plan.source_instance_id,
    targetInstanceIds: [...plan.target_instance_ids],
    configPathKeys: plan.config_paths.map(configPathKey),
    projectSelections: [...plan.project_selections],
    sessionIds: [],
  };
}

export function instanceSyncSelectionAfterSourceChange(
  sourceInstanceId: number | null,
): InstanceSyncSelection {
  return {
    sourceInstanceId,
    targetInstanceIds: [],
    configPathKeys: [],
    projectSelections: [],
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
  if (
    selection.sessionIds.length === 0 &&
    selection.projectSelections.length === 0 &&
    selection.configPathKeys.length === 0
  ) {
    return "请至少选择一个会话、项目或配置项";
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

function normalizeWslArchitecture(value: string) {
  const normalized = value.trim().toLowerCase();
  if (normalized === "amd64") return "x86_64";
  if (normalized === "arm64") return "aarch64";
  return normalized;
}

function normalizeLinuxCodexHome(value: string) {
  const normalized = value.replace(/\/+$/, "");
  return normalized || "/";
}
