import {
  applyInstanceSyncPlan,
  automaticNonRootDiffConfigPathKeys,
  automaticNonRootDiffPlanExecutionBlockMessage,
  automaticNonRootDiffPlanId,
  availableInstanceSyncTargets,
  configPathKey,
  isAutomaticNonRootDiffPlan,
  isCurrentAutomaticNonRootDiffContext,
  instanceAvailability,
  instanceSyncTargetSummary,
  managedInstanceDeleteConfirmation,
  managedInstanceIgnoreConfirmation,
  instanceDisplayName,
  instanceScanSummary,
  validateInstanceSyncSelection,
} from "./instanceManagement.js";
import {
  DelayedInstanceSyncPreview,
  ExpiringInstanceSyncPreviewCache,
  InstanceSyncPreviewInputMode,
  configDiffTargetDisplay,
  instanceSyncConfigDiffCacheKey,
  restoreInstanceSyncScroll,
  snapshotInstanceSyncScroll,
} from "./instanceSyncPreview.js";

function expectEqual<T>(actual: T, expected: T, message: string) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`);
  }
}

const availableInstance = {
  id: 1,
  path: "E:\\codex\\office",
  display_name: "办公账号",
  available: true,
  added_at_unix: 1,
  last_seen_at_unix: 2,
};

expectEqual(instanceDisplayName(availableInstance), "办公账号", "uses the application-only display name");
expectEqual(
  instanceDisplayName({ ...availableInstance, display_name: null }),
  "office",
  "uses the final path segment when an application-only name is absent",
);
expectEqual(
  instanceAvailability({ ...availableInstance, available: false }),
  { label: "已失效", detail: "配置文件或实例目录已缺失" },
  "describes unavailable instances without relying on color alone",
);
expectEqual(
  instanceScanSummary({ added: 2, reactivated: 0, ignored: 0, already_managed: 3, skipped: 1 }),
  "最近扫描：新增 2 个 · 已存在 3 个 · 跳过 1 个",
  "summarizes scan results for the management page",
);
expectEqual(
  instanceScanSummary({ added: 2, reactivated: 1, ignored: 0, already_managed: 3, skipped: 1 }),
  "最近扫描：新增 2 个 · 重新登记 1 个 · 已存在 3 个 · 跳过 1 个",
  "includes automatic re-registration in the scan summary",
);
expectEqual(
  instanceScanSummary({ added: 2, reactivated: 1, ignored: 4, already_managed: 3, skipped: 1 }),
  "最近扫描：新增 2 个 · 重新登记 1 个 · 永久忽略 4 个 · 已存在 3 个 · 跳过 1 个",
  "includes permanently ignored instances in the scan summary",
);
expectEqual(
  managedInstanceDeleteConfirmation(availableInstance),
  "删除“办公账号”的登记记录？此操作不会删除文件夹或 config.toml。",
  "confirms that deleting an instance only removes its application record",
);
expectEqual(
  managedInstanceIgnoreConfirmation(availableInstance),
  "永久忽略“办公账号”的登记记录？此操作不会删除文件夹或 config.toml，且以后扫描不会自动重新添加。",
  "confirms that permanently ignored instances stay hidden from future scans",
);

const secondAvailableInstance = {
  ...availableInstance,
  id: 2,
  path: "E:\\codex\\home",
  display_name: "家庭账号",
};

expectEqual(
  availableInstanceSyncTargets(
    [availableInstance, secondAvailableInstance, { ...availableInstance, id: 3, available: false }],
    availableInstance.id,
  ).map((instance) => instance.id),
  [secondAvailableInstance.id],
  "only available instances other than the source can be selected as sync targets",
);
expectEqual(
  applyInstanceSyncPlan({
    id: 4,
    name: "办公室同步",
    source_instance_id: availableInstance.id,
    target_instance_ids: [secondAvailableInstance.id],
    config_paths: [["model"], ["model_providers", "office", "api_key"]],
    created_at_unix: 1,
    updated_at_unix: 2,
  }),
  {
    sourceInstanceId: availableInstance.id,
    targetInstanceIds: [secondAvailableInstance.id],
    configPathKeys: [configPathKey(["model"]), configPathKey(["model_providers", "office", "api_key"])],
    sessionIds: [],
  },
  "loading a sync plan restores only instance and config choices, never prior session choices",
);
expectEqual(
  isAutomaticNonRootDiffPlan(automaticNonRootDiffPlanId),
  true,
  "recognizes the built-in non-root difference plan",
);
expectEqual(
  isAutomaticNonRootDiffPlan(null),
  false,
  "keeps the empty plan distinct from the built-in plan",
);
expectEqual(
  automaticNonRootDiffPlanExecutionBlockMessage(automaticNonRootDiffPlanId, true),
  "正在计算非根配置差异，请等待自动选择完成",
  "blocks preview and execution while the built-in plan is still calculating",
);
expectEqual(
  automaticNonRootDiffPlanExecutionBlockMessage(automaticNonRootDiffPlanId, false),
  null,
  "allows preview and execution after the built-in plan finishes calculating",
);
expectEqual(
  automaticNonRootDiffPlanExecutionBlockMessage(null, true),
  null,
  "does not block manually selected sync configurations",
);
expectEqual(
  automaticNonRootDiffConfigPathKeys([
    ["model_providers", "office", "api_key"],
    ["features", "enabled"],
  ]),
  [
    configPathKey(["model_providers", "office", "api_key"]),
    configPathKey(["features", "enabled"]),
  ],
  "converts backend-selected paths into config checkbox keys",
);
expectEqual(
  isCurrentAutomaticNonRootDiffContext(
    automaticNonRootDiffPlanId,
    availableInstance.id,
    [secondAvailableInstance.id, 3],
    automaticNonRootDiffPlanId,
    availableInstance.id,
    [3, secondAvailableInstance.id],
  ),
  false,
  "rejects an automatic selection response after target order changes",
);
expectEqual(
  isCurrentAutomaticNonRootDiffContext(
    automaticNonRootDiffPlanId,
    availableInstance.id,
    [secondAvailableInstance.id],
    automaticNonRootDiffPlanId,
    availableInstance.id,
    [secondAvailableInstance.id],
  ),
  true,
  "accepts the latest matching automatic selection response",
);
expectEqual(
  validateInstanceSyncSelection({
    sourceInstanceId: availableInstance.id,
    targetInstanceIds: [secondAvailableInstance.id],
    sessionIds: [],
    configPathKeys: [],
  }),
  "请至少选择一个会话或配置项",
  "rejects a sync request with neither sessions nor configuration paths",
);
expectEqual(
  instanceSyncTargetSummary({
    sessions_added: ["a"],
    sessions_skipped: ["b"],
    session_conflicts: [{ session_id: "c", reason: "冲突" }],
    config_paths_applied: 2,
    error: null,
  }),
  "新增 1 · 相同跳过 1 · 冲突 1 · 配置 2 项",
  "summarizes each target sync result for display",
);

const syncScrollContainers = [
  { dataset: { instanceSyncScroll: "targets" }, scrollLeft: 3, scrollTop: 12 },
  { dataset: { instanceSyncScroll: "sessions" }, scrollLeft: 5, scrollTop: 24 },
  { dataset: { instanceSyncScroll: "config" }, scrollLeft: 7, scrollTop: 36 },
  { dataset: { instanceSyncScroll: "unknown" }, scrollLeft: 99, scrollTop: 99 },
];
const syncScrollSnapshot = snapshotInstanceSyncScroll(syncScrollContainers);
expectEqual(
  syncScrollSnapshot,
  {
    targets: { left: 3, top: 12 },
    sessions: { left: 5, top: 24 },
    config: { left: 7, top: 36 },
  },
  "captures every known instance-sync list independently",
);
syncScrollContainers.forEach((container) => {
  container.scrollLeft = 0;
  container.scrollTop = 0;
});
restoreInstanceSyncScroll(syncScrollContainers, syncScrollSnapshot);
expectEqual(
  syncScrollContainers.map(({ scrollLeft, scrollTop }) => ({ scrollLeft, scrollTop })),
  [
    { scrollLeft: 3, scrollTop: 12 },
    { scrollLeft: 5, scrollTop: 24 },
    { scrollLeft: 7, scrollTop: 36 },
    { scrollLeft: 0, scrollTop: 0 },
  ],
  "restores scroll positions by stable list identifier instead of DOM order",
);

let nextTimerId = 1;
let scheduledDelay = 0;
const scheduledTimers = new Map<number, () => void>();
const delayedPreview = new DelayedInstanceSyncPreview({
  setTimeout(callback, delay) {
    const timerId = nextTimerId++;
    scheduledDelay = delay;
    scheduledTimers.set(timerId, callback);
    return timerId;
  },
  clearTimeout(timerId) {
    scheduledTimers.delete(timerId);
  },
});
const previewEvents: string[] = [];
const cancelledRequest = delayedPreview.schedule((requestId) => {
  previewEvents.push(`cancelled-${requestId}`);
});
expectEqual(scheduledDelay, 500, "waits 500ms before opening a pointer preview");
delayedPreview.cancel();
expectEqual([...scheduledTimers.values()].length, 0, "cancels a pending preview when pointer leaves");
const activeRequest = delayedPreview.schedule((requestId) => {
  previewEvents.push(`active-${requestId}`);
});
const activeTimer = nextTimerId - 1;
scheduledTimers.get(activeTimer)?.();
expectEqual(previewEvents, [`active-${activeRequest}`], "only opens the latest hovered row");
expectEqual(delayedPreview.isCurrent(cancelledRequest), false, "invalidates cancelled delayed requests");
const staleRequest = delayedPreview.openImmediately(() => undefined);
const latestRequest = delayedPreview.openImmediately(() => undefined);
expectEqual(
  [delayedPreview.isCurrent(staleRequest), delayedPreview.isCurrent(latestRequest)],
  [false, true],
  "allows callers to ignore an older asynchronous preview response",
);

const previewInputMode = new InstanceSyncPreviewInputMode();
previewInputMode.recordPointerInput();
expectEqual(
  previewInputMode.allowsImmediateFocusPreview(),
  false,
  "does not open an immediate preview for a mouse-triggered focus",
);
previewInputMode.recordKeyboardInput();
expectEqual(
  previewInputMode.allowsImmediateFocusPreview(),
  true,
  "opens an immediate preview after keyboard focus navigation",
);

let nextCacheTimerId = 1;
const cacheTimers = new Map<number, () => void>();
const expiringPreviewCache = new ExpiringInstanceSyncPreviewCache<string>(
  {
    setTimeout(callback) {
      const timerId = nextCacheTimerId++;
      cacheTimers.set(timerId, callback);
      return timerId;
    },
    clearTimeout(timerId) {
      cacheTimers.delete(timerId);
    },
  },
  30_000,
);
expiringPreviewCache.set("source-target-path", "sensitive-value");
expiringPreviewCache.scheduleClear();
const firstCacheTimer = nextCacheTimerId - 1;
expectEqual(cacheTimers.has(firstCacheTimer), true, "expires cached config values after a short idle period");
expectEqual(
  expiringPreviewCache.get("source-target-path"),
  "sensitive-value",
  "keeps a completed config diff while the user continues inspecting it",
);
expectEqual(cacheTimers.has(firstCacheTimer), false, "cancels expiry while a cached diff is reopened");
expiringPreviewCache.scheduleClear();
cacheTimers.get(nextCacheTimerId - 1)?.();
expectEqual(
  expiringPreviewCache.get("source-target-path"),
  undefined,
  "removes cached config values after the idle timeout",
);

expectEqual(
  configDiffTargetDisplay({ status: "changed", original_value: "\"target\"" }, "\"source\""),
  {
    statusLabel: "已变更",
    before: { label: "原值", value: "\"target\"", tone: "removed" },
    after: { label: "替换值", value: "\"source\"", tone: "added" },
  },
  "maps changed config values to labelled red and green diff data",
);
expectEqual(
  configDiffTargetDisplay({ status: "missing", original_value: null }, "\"source\""),
  {
    statusLabel: "未设置",
    before: { label: "原值", value: "未设置", tone: "removed" },
    after: { label: "替换值", value: "\"source\"", tone: "added" },
  },
  "maps a missing target value to labelled red and green diff data",
);
expectEqual(
  configDiffTargetDisplay({ status: "same", original_value: "\"source\"" }, "\"source\""),
  { statusLabel: "无变化", detail: "目标值与源值相同" },
  "does not invent red and green values when a target is unchanged",
);
expectEqual(
  [
    instanceSyncConfigDiffCacheKey(1, [2, 3], ["model"]),
    instanceSyncConfigDiffCacheKey(1, [3, 2], ["model"]),
  ],
  [
    "[1,[2,3],[\"model\"]]",
    "[1,[3,2],[\"model\"]]",
  ],
  "includes ordered targets in the config-diff cache key",
);
