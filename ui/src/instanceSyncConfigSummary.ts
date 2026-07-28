export interface InstanceSyncConfigDifferenceSummaryPathLike {
  config_path: string[];
  different_target_count: number;
  readable_target_count: number;
}

export type InstanceSyncConfigDifferenceTreeState =
  | { tone: "none"; className: "" }
  | { tone: "difference"; className: "has-difference"; label: string }
  | { tone: "warning"; className: "has-read-warning"; label: string };

export function instanceSyncConfigDifferenceTreeState(
  path: InstanceSyncConfigDifferenceSummaryPathLike | undefined,
  unreadableTargetCount: number,
): InstanceSyncConfigDifferenceTreeState {
  if (!path) return { tone: "none", className: "" };
  if (path.different_target_count > 0) {
    return {
      tone: "difference",
      className: "has-difference",
      label: `与 ${path.different_target_count} 个目标不同`,
    };
  }
  if (path.readable_target_count === 0 && unreadableTargetCount > 0) {
    return {
      tone: "warning",
      className: "has-read-warning",
      label: `全部 ${unreadableTargetCount} 个目标无法读取`,
    };
  }
  return { tone: "none", className: "" };
}

export function isCurrentInstanceSyncConfigDifferenceSummaryContext(
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
