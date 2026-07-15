export const instanceSyncScrollRegions = ["targets", "sessions", "config"] as const;

export type InstanceSyncScrollRegion = (typeof instanceSyncScrollRegions)[number];

export interface InstanceSyncScrollable {
  dataset: { instanceSyncScroll?: string };
  scrollLeft: number;
  scrollTop: number;
}

export interface InstanceSyncScrollPosition {
  left: number;
  top: number;
}

export type InstanceSyncScrollSnapshot = Partial<
  Record<InstanceSyncScrollRegion, InstanceSyncScrollPosition>
>;

export function snapshotInstanceSyncScroll(
  containers: Iterable<InstanceSyncScrollable>,
): InstanceSyncScrollSnapshot {
  const snapshot: InstanceSyncScrollSnapshot = {};
  for (const container of containers) {
    const region = instanceSyncScrollRegion(container.dataset.instanceSyncScroll);
    if (!region) continue;
    snapshot[region] = {
      left: container.scrollLeft,
      top: container.scrollTop,
    };
  }
  return snapshot;
}

export function restoreInstanceSyncScroll(
  containers: Iterable<InstanceSyncScrollable>,
  snapshot: InstanceSyncScrollSnapshot,
) {
  for (const container of containers) {
    const region = instanceSyncScrollRegion(container.dataset.instanceSyncScroll);
    const position = region ? snapshot[region] : undefined;
    if (!position) continue;
    container.scrollLeft = position.left;
    container.scrollTop = position.top;
  }
}

function instanceSyncScrollRegion(value: string | undefined): InstanceSyncScrollRegion | null {
  return instanceSyncScrollRegions.includes(value as InstanceSyncScrollRegion)
    ? (value as InstanceSyncScrollRegion)
    : null;
}

export interface DelayedPreviewTimer {
  setTimeout(callback: () => void, delay: number): number;
  clearTimeout(timerId: number): void;
}

const browserPreviewTimer: DelayedPreviewTimer = {
  setTimeout: (callback, delay) => globalThis.setTimeout(callback, delay),
  clearTimeout: (timerId) => globalThis.clearTimeout(timerId),
};

export class DelayedInstanceSyncPreview {
  private timerId: number | null = null;
  private requestId = 0;

  constructor(
    private readonly timer: DelayedPreviewTimer = browserPreviewTimer,
    private readonly delayMs = 500,
  ) {}

  schedule(open: (requestId: number) => void) {
    const requestId = this.beginRequest();
    this.timerId = this.timer.setTimeout(() => {
      this.timerId = null;
      if (this.isCurrent(requestId)) open(requestId);
    }, this.delayMs);
    return requestId;
  }

  openImmediately(open: (requestId: number) => void) {
    const requestId = this.beginRequest();
    open(requestId);
    return requestId;
  }

  cancel() {
    this.clearTimer();
    this.requestId += 1;
  }

  isCurrent(requestId: number) {
    return requestId === this.requestId;
  }

  private beginRequest() {
    this.clearTimer();
    this.requestId += 1;
    return this.requestId;
  }

  private clearTimer() {
    if (this.timerId == null) return;
    this.timer.clearTimeout(this.timerId);
    this.timerId = null;
  }
}

export class InstanceSyncPreviewInputMode {
  private latestInput: "keyboard" | "pointer" = "pointer";

  recordKeyboardInput() {
    this.latestInput = "keyboard";
  }

  recordPointerInput() {
    this.latestInput = "pointer";
  }

  allowsImmediateFocusPreview() {
    return this.latestInput === "keyboard";
  }
}

export class ExpiringInstanceSyncPreviewCache<T> {
  private readonly values = new Map<string, T>();
  private timerId: number | null = null;

  constructor(
    private readonly timer: DelayedPreviewTimer = browserPreviewTimer,
    private readonly lifetimeMs = 30_000,
  ) {}

  get(key: string) {
    this.retain();
    return this.values.get(key);
  }

  set(key: string, value: T) {
    this.retain();
    this.values.set(key, value);
  }

  scheduleClear() {
    if (this.values.size === 0) return;
    this.retain();
    this.timerId = this.timer.setTimeout(() => {
      this.values.clear();
      this.timerId = null;
    }, this.lifetimeMs);
  }

  clear() {
    this.retain();
    this.values.clear();
  }

  private retain() {
    if (this.timerId == null) return;
    this.timer.clearTimeout(this.timerId);
    this.timerId = null;
  }
}

export type InstanceSyncConfigDiffStatus = "changed" | "same" | "missing" | "read_error";

export interface InstanceSyncConfigDiffTargetLike {
  status: InstanceSyncConfigDiffStatus;
  original_value?: string | null;
  error?: string | null;
}

export interface InstanceSyncConfigDiffValueDisplay {
  label: "原值" | "替换值";
  value: string;
  tone: "removed" | "added";
}

export interface InstanceSyncConfigDiffDisplay {
  statusLabel: string;
  detail?: string;
  before?: InstanceSyncConfigDiffValueDisplay;
  after?: InstanceSyncConfigDiffValueDisplay;
}

export function configDiffTargetDisplay(
  target: InstanceSyncConfigDiffTargetLike,
  sourceValue: string,
): InstanceSyncConfigDiffDisplay {
  if (target.status === "changed") {
    return changedConfigDiffDisplay(target.original_value || "未设置", sourceValue, "已变更");
  }
  if (target.status === "missing") {
    return changedConfigDiffDisplay("未设置", sourceValue, "未设置");
  }
  if (target.status === "same") {
    return { statusLabel: "无变化", detail: "目标值与源值相同" };
  }
  return {
    statusLabel: "读取失败",
    detail: target.error || "无法读取目标 config.toml",
  };
}

function changedConfigDiffDisplay(
  originalValue: string,
  sourceValue: string,
  statusLabel: string,
): InstanceSyncConfigDiffDisplay {
  return {
    statusLabel,
    before: { label: "原值", value: originalValue, tone: "removed" },
    after: { label: "替换值", value: sourceValue, tone: "added" },
  };
}

export function instanceSyncConfigDiffCacheKey(
  sourceInstanceId: number,
  targetInstanceIds: number[],
  configPath: string[],
) {
  return JSON.stringify([sourceInstanceId, targetInstanceIds, configPath]);
}
