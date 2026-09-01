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
  instanceSyncInstanceLabel,
  instanceSyncInstancesCompatible,
  instanceSyncSelectionAfterSourceChange,
  instanceSyncSourceCandidates,
  instanceSyncTargetFilterDescription,
  instanceSyncTargetSummary,
  managedInstanceDeleteConfirmation,
  managedInstanceIgnoreConfirmation,
  instanceDisplayName,
  instanceRuntimeLabel,
  instanceScanSummary,
  isUnsupportedManualCodexHome,
  manualCodexHomeUpdate,
  reconcileInstanceSyncAvailability,
  reconcileInstanceSyncPlan,
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
import {
  instanceSyncConfigDifferenceTreeState,
  isCurrentInstanceSyncConfigDifferenceSummaryContext,
} from "./instanceSyncConfigSummary.js";
import {
  buildInstanceSyncProjectGroups,
  instanceSyncProjectSelectionFromKey,
  instanceSyncProjectSelectionKey,
  isInstanceSyncSessionSelected,
  reconcileInstanceSyncSessionSelection,
  selectedInstanceSyncSessionIds,
  setInstanceSyncProjectSelection,
  setInstanceSyncSessionSelection,
} from "./instanceSyncSelection.js";

function expectEqual<T>(actual: T, expected: T, message: string) {
  // 对象键序不参与相等性判断，仅数组保留顺序语义。
  const stable = (value: unknown): unknown => {
    if (Array.isArray(value)) return value.map(stable);
    if (value && typeof value === "object") {
      return Object.fromEntries(
        Object.entries(value as Record<string, unknown>)
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([key, nested]) => [key, stable(nested)]),
      );
    }
    return value;
  };
  const actualJson = JSON.stringify(stable(actual));
  const expectedJson = JSON.stringify(stable(expected));
  if (actualJson !== expectedJson) {
    throw new Error(`${message}\nactual: ${actualJson}\nexpected: ${expectedJson}`);
  }
}

const availableInstance = {
  id: 1,
  path: "E:\\codex\\office",
  display_name: "办公账号",
  availability: "available" as const,
  runtime: { kind: "native" as const },
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
  instanceAvailability({ ...availableInstance, availability: "unavailable" }),
  { label: "不可用", detail: "配置文件、实例目录或 WSL 发行版不可用" },
  "describes unavailable instances without relying on color alone",
);
expectEqual(
  instanceAvailability({ ...availableInstance, availability: "unknown" }),
  { label: "未检测", detail: "尚未实时检查 WSL 发行版、用户和 config.toml" },
  "describes an unprobed WSL instance without treating it as unavailable",
);
expectEqual(
  [
    isUnsupportedManualCodexHome("\\\\wsl.localhost\\Ubuntu\\home\\dev\\.codex"),
    isUnsupportedManualCodexHome("//wsl$/Ubuntu/home/dev/.codex"),
    isUnsupportedManualCodexHome("\\\\wsl.localhost"),
    isUnsupportedManualCodexHome("\\\\?\\UNC\\wsl$\\Ubuntu"),
    isUnsupportedManualCodexHome("/mnt/c/Users/dev/.codex"),
    isUnsupportedManualCodexHome("C:\\Users\\dev\\.codex"),
  ],
  [true, true, true, true, true, false],
  "rejects direct WSL UNC and /mnt paths in the manual Windows profile field",
);
const previousManualCodexHome = "C:\\Users\\dev\\.codex";
const rejectedManualCodexHome = manualCodexHomeUpdate(
  "\\\\?\\UNC\\wsl.localhost\\Ubuntu\\home\\dev\\.codex",
  previousManualCodexHome,
);
expectEqual(
  rejectedManualCodexHome,
  {
    accepted: false,
    value: previousManualCodexHome,
    message: "Windows 原生 Codex 主目录不支持 WSL UNC 或 /mnt 路径，请先登记 WSL 实例",
  },
  "keeps the effective manual profile path so a rejected paste can be rendered back",
);
expectEqual(
  manualCodexHomeUpdate("C:\\Users\\dev\\new-codex", previousManualCodexHome),
  { accepted: true, value: "C:\\Users\\dev\\new-codex", message: null },
  "accepts a native Windows path in the manual profile field",
);
const wslInstance = {
  ...availableInstance,
  id: 9,
  path: "\\\\wsl.localhost\\Ubuntu\\home\\dev\\.codex",
  display_name: null,
  runtime: {
    kind: "wsl" as const,
    distribution: "Ubuntu",
    user: "dev",
    codex_home: "/home/dev/.codex",
    host_path: "\\\\wsl.localhost\\Ubuntu\\home\\dev\\.codex",
    architecture: "x86_64",
  },
};
expectEqual(instanceDisplayName(wslInstance), "Ubuntu · dev", "uses WSL identity as the default name");
expectEqual(instanceRuntimeLabel(wslInstance), "WSL · Ubuntu", "labels the WSL runtime explicitly");
expectEqual(
  instanceSyncInstanceLabel(wslInstance),
  "Ubuntu · dev · WSL · Ubuntu · dev · /home/dev/.codex",
  "shows runtime identity and the full Linux Codex home in sync selectors",
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
    [availableInstance, secondAvailableInstance, { ...availableInstance, id: 3, availability: "unavailable" }],
    availableInstance.id,
  ).map((instance) => instance.id),
  [secondAvailableInstance.id],
  "only available instances other than the source can be selected as sync targets",
);
expectEqual(
  availableInstanceSyncTargets([availableInstance, secondAvailableInstance, wslInstance], availableInstance.id).map(
    (instance) => instance.id,
  ),
  [secondAvailableInstance.id],
  "keeps Windows and WSL instances in separate synchronization groups",
);
const sameWslGroupTarget = {
  ...wslInstance,
  id: 10,
  path: "\\\\wsl.localhost\\ubuntu\\home\\dev\\.codex-work",
  runtime: {
    ...wslInstance.runtime,
    distribution: "ubuntu",
    codex_home: "/home/dev/.codex-work",
    host_path: "\\\\wsl.localhost\\ubuntu\\home\\dev\\.codex-work",
    architecture: "amd64",
  },
};
const otherWslUser = {
  ...sameWslGroupTarget,
  id: 11,
  runtime: { ...sameWslGroupTarget.runtime, user: "Dev" },
};
const otherWslDistribution = {
  ...sameWslGroupTarget,
  id: 12,
  runtime: { ...sameWslGroupTarget.runtime, distribution: "Debian" },
};
expectEqual(
  availableInstanceSyncTargets(
    [wslInstance, sameWslGroupTarget, otherWslUser, otherWslDistribution, availableInstance],
    wslInstance.id,
  ).map((instance) => instance.id),
  [sameWslGroupTarget.id],
  "matches WSL distributions case-insensitively while requiring an exact Linux user",
);
expectEqual(
  [
    instanceSyncInstancesCompatible(wslInstance, sameWslGroupTarget),
    instanceSyncInstancesCompatible(wslInstance, otherWslUser),
    instanceSyncInstancesCompatible(wslInstance, otherWslDistribution),
  ],
  [true, false, false],
  "applies the complete WSL compatibility matrix",
);
expectEqual(
  instanceSyncSourceCandidates([availableInstance, wslInstance]).map((instance) => instance.id),
  [availableInstance.id, wslInstance.id],
  "offers both Windows and WSL instances as synchronization sources",
);
expectEqual(
  instanceSyncSourceCandidates([{ ...wslInstance, availability: "unknown" }]).length,
  0,
  "keeps unprobed WSL instances out of synchronization selectors until they are available",
);
expectEqual(
  instanceSyncTargetFilterDescription(wslInstance),
  "仅显示 Ubuntu / dev 的其他 WSL 实例",
  "describes the active WSL target filter",
);
expectEqual(
  instanceSyncSelectionAfterSourceChange(wslInstance.id),
  {
    sourceInstanceId: wslInstance.id,
    targetInstanceIds: [],
    configPathKeys: [],
    projectSelections: [],
    sessionIds: [],
  },
  "switching the source immediately clears old targets, sessions, projects, and config selections",
);
expectEqual(
  applyInstanceSyncPlan({
    id: 4,
    name: "办公室同步",
    source_instance_id: availableInstance.id,
    target_instance_ids: [secondAvailableInstance.id],
    config_paths: [["model"], ["model_providers", "office", "api_key"]],
    project_selections: ["e:/work/office", null],
    created_at_unix: 1,
    updated_at_unix: 2,
  }),
  {
    sourceInstanceId: availableInstance.id,
    targetInstanceIds: [secondAvailableInstance.id],
    configPathKeys: [configPathKey(["model"]), configPathKey(["model_providers", "office", "api_key"])],
    projectSelections: ["e:/work/office", null],
    sessionIds: [],
  },
  "loading a sync plan restores project selections but never prior single-session choices",
);
expectEqual(
  reconcileInstanceSyncPlan(
    {
      id: 5,
      name: "旧 WSL 方案",
      source_instance_id: wslInstance.id,
      target_instance_ids: [sameWslGroupTarget.id, otherWslUser.id, 999],
      config_paths: [["model"]],
      project_selections: [],
      created_at_unix: 1,
      updated_at_unix: 2,
    },
    [wslInstance, sameWslGroupTarget, otherWslUser],
  ),
  {
    selection: {
      sourceInstanceId: wslInstance.id,
      targetInstanceIds: [sameWslGroupTarget.id],
      configPathKeys: [configPathKey(["model"])],
      projectSelections: [],
      sessionIds: [],
    },
    removedTargetCount: 2,
    sourceAvailable: true,
    sourceRemoved: false,
  },
  "loading a saved WSL plan removes missing or incompatible targets and reports their count",
);
expectEqual(
  reconcileInstanceSyncPlan(
    {
      id: 6,
      name: "失效源方案",
      source_instance_id: 42,
      target_instance_ids: [availableInstance.id, secondAvailableInstance.id],
      config_paths: [["model"], ["features", "enabled"]],
      project_selections: ["e:/work/office", null],
      created_at_unix: 1,
      updated_at_unix: 2,
    },
    [availableInstance, secondAvailableInstance],
  ),
  {
    selection: {
      sourceInstanceId: null,
      targetInstanceIds: [],
      configPathKeys: [],
      projectSelections: [],
      sessionIds: [],
    },
    removedTargetCount: 2,
    sourceAvailable: false,
    sourceRemoved: true,
  },
  "a plan whose source disappeared loads with every source-dependent choice cleared",
);
expectEqual(
  reconcileInstanceSyncAvailability(
    [
      availableInstance,
      secondAvailableInstance,
      { ...availableInstance, id: 3, availability: "unavailable" },
    ],
    {
      sourceInstanceId: availableInstance.id,
      targetInstanceIds: [secondAvailableInstance.id, 3, 999],
      configPathKeys: [configPathKey(["model"])],
      projectSelections: ["e:/work/office", null],
      sessionIds: ["session-1"],
    },
  ),
  {
    selection: {
      sourceInstanceId: availableInstance.id,
      targetInstanceIds: [secondAvailableInstance.id],
      configPathKeys: [configPathKey(["model"])],
      projectSelections: ["e:/work/office", null],
      sessionIds: ["session-1"],
    },
    removedTargetCount: 2,
    sourceAvailable: true,
    sourceRemoved: false,
  },
  "an available source keeps project, session, and config choices while dropping failed targets",
);
expectEqual(
  reconcileInstanceSyncAvailability(
    [availableInstance],
    {
      sourceInstanceId: null,
      targetInstanceIds: [],
      configPathKeys: [],
      projectSelections: [],
      sessionIds: [],
    },
  ),
  {
    selection: {
      sourceInstanceId: null,
      targetInstanceIds: [],
      configPathKeys: [],
      projectSelections: [],
      sessionIds: [],
    },
    removedTargetCount: 0,
    sourceAvailable: false,
    sourceRemoved: false,
  },
  "an untouched empty selection is not reported as a removed source",
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
    projectSelections: [],
    sessionIds: [],
    configPathKeys: [],
  }),
  "请至少选择一个会话、项目或配置项",
  "rejects a sync request with neither sessions, projects, nor configuration paths",
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
  instanceSyncConfigDifferenceTreeState(
    { config_path: ["model"], different_target_count: 2, readable_target_count: 3 },
    1,
  ),
  { tone: "difference", className: "has-difference", label: "与 2 个目标不同" },
  "maps any readable configuration difference to the muted red tree class",
);
expectEqual(
  instanceSyncConfigDifferenceTreeState(
    { config_path: ["model"], different_target_count: 0, readable_target_count: 3 },
    1,
  ),
  { tone: "none", className: "" },
  "does not manufacture a difference when readable targets match",
);
expectEqual(
  instanceSyncConfigDifferenceTreeState(
    { config_path: ["model"], different_target_count: 0, readable_target_count: 0 },
    2,
  ),
  { tone: "warning", className: "has-read-warning", label: "全部 2 个目标无法读取" },
  "maps only unreadable targets to a warning instead of a difference",
);
expectEqual(
  isCurrentInstanceSyncConfigDifferenceSummaryContext(
    4,
    availableInstance.id,
    [secondAvailableInstance.id],
    4,
    availableInstance.id,
    [secondAvailableInstance.id],
  ),
  true,
  "accepts a configuration summary response for the active plan context",
);
expectEqual(
  isCurrentInstanceSyncConfigDifferenceSummaryContext(
    4,
    availableInstance.id,
    [secondAvailableInstance.id],
    null,
    availableInstance.id,
    [secondAvailableInstance.id],
  ),
  false,
  "rejects a configuration summary response after the plan changes",
);
expectEqual(
  isCurrentInstanceSyncConfigDifferenceSummaryContext(
    4,
    availableInstance.id,
    [secondAvailableInstance.id, 3],
    4,
    availableInstance.id,
    [3, secondAvailableInstance.id],
  ),
  false,
  "rejects a configuration summary response after the targets change",
);
expectEqual(
  isCurrentInstanceSyncConfigDifferenceSummaryContext(
    4,
    availableInstance.id,
    [secondAvailableInstance.id],
    4,
    3,
    [secondAvailableInstance.id],
  ),
  false,
  "rejects a configuration summary response after the source changes",
);

const instanceSyncSelectionSessions = [
  { id: "alpha-old", project: "E:\\code\\alpha", sort_updated_at_ms: 20, updated_at: "2026-07-01" },
  { id: "alpha-new", project: "e:/code/alpha/", sort_updated_at_ms: 80, updated_at: "2026-07-03" },
  { id: "beta", project: "E:\\code\\beta", sort_updated_at_ms: 100, updated_at: "2026-07-04" },
  { id: "ungrouped", sort_updated_at_ms: 60, updated_at: "2026-07-02" },
];
const instanceSyncProjectGroups = buildInstanceSyncProjectGroups(instanceSyncSelectionSessions);
expectEqual(
  instanceSyncProjectGroups.map((group) => [group.project, group.sessions.map((session) => session.id)]),
  [
    ["e:/code/beta", ["beta"]],
    ["e:/code/alpha", ["alpha-new", "alpha-old"]],
    [null, ["ungrouped"]],
  ],
  "groups source sessions by normalized project and sorts each project by recency",
);
expectEqual(
  [
    instanceSyncProjectSelectionKey(null),
    instanceSyncProjectSelectionFromKey(instanceSyncProjectSelectionKey("E:\\code\\alpha\\")),
  ],
  ["null", "e:/code/alpha"],
  "uses null rather than the display label as the ungrouped project identity",
);

let instanceSyncSelection = {
  projectSelections: new Set<string | null>(),
  sessionIds: new Set<string>(),
};
instanceSyncSelection = setInstanceSyncProjectSelection(
  instanceSyncSelectionSessions,
  instanceSyncSelection,
  "e:/code/alpha",
  true,
);
expectEqual(
  selectedInstanceSyncSessionIds(instanceSyncSelectionSessions, instanceSyncSelection),
  ["alpha-old", "alpha-new"],
  "a project-level selection includes all currently loaded sessions in that project",
);
instanceSyncSelection = setInstanceSyncSessionSelection(
  instanceSyncSelectionSessions,
  instanceSyncSelection,
  instanceSyncSelectionSessions[0],
  false,
);
expectEqual(
  {
    projectSelections: [...instanceSyncSelection.projectSelections],
    sessionIds: [...instanceSyncSelection.sessionIds],
  },
  { projectSelections: [], sessionIds: ["alpha-new"] },
  "deselecting one project-selected session converts the project to explicit current-run sessions",
);
expectEqual(
  isInstanceSyncSessionSelected(instanceSyncSelectionSessions[1], instanceSyncSelection),
  true,
  "the remaining project session stays selected after conversion",
);
expectEqual(
  isInstanceSyncSessionSelected(instanceSyncSelectionSessions[0], instanceSyncSelection),
  false,
  "the deselected project session stays excluded after conversion",
);
const reconciledProjectSelection = reconcileInstanceSyncSessionSelection(
  [],
  {
    projectSelections: new Set(["e:/work/future", null]),
    sessionIds: new Set(["removed-session"]),
  },
);
expectEqual(
  {
    projectSelections: [...reconciledProjectSelection.projectSelections],
    sessionIds: [...reconciledProjectSelection.sessionIds],
  },
  { projectSelections: ["e:/work/future", null], sessionIds: [] },
  "keeps saved project conditions when the source currently has no matching sessions",
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
