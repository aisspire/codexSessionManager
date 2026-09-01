import { pollAutoSync } from "./autoSync.js";

function expectEqual<T>(actual: T, expected: T, message: string) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`);
  }
}

function optionsFor(
  state: { autoSyncInFlight: boolean },
  invoke: <T>(command: string, args: { profile: string }) => Promise<T>,
  request: { current: number; profile: string; wasRunning: boolean | null },
  callbacks: {
    statuses: string[];
    errors: unknown[];
    synced: string[];
  },
) {
  return {
    state,
    enabled: () => true,
    getProfile: () => request.profile,
    getRequestId: () => request.current,
    isCurrentRequest: (requestId: number) => requestId === request.current,
    invoke,
    wasRunning: () => request.wasRunning,
    setWasRunning: (running: boolean) => {
      request.wasRunning = running;
    },
    onStatus: (status: string) => callbacks.statuses.push(status),
    onSync: async (_report: unknown, profile: string) => {
      callbacks.synced.push(profile);
    },
    onError: (error: unknown) => {
      callbacks.errors.push(error);
    },
  };
}

const state = { autoSyncInFlight: false };
const request = { current: 1, profile: "old-profile", wasRunning: null as boolean | null };
const callbacks = { statuses: [] as string[], errors: [] as unknown[], synced: [] as string[] };
let resolveDetection!: (value: boolean) => void;
let detectionCalls = 0;
const firstDetection = new Promise<boolean>((resolve) => {
  resolveDetection = resolve;
});
const invoke = async <T>(command: string) => {
  if (command === "detect_codex_running") {
    detectionCalls += 1;
    return (await firstDetection) as T;
  }
  throw new Error(`unexpected command: ${command}`);
};

const firstPoll = pollAutoSync(optionsFor(state, invoke, request, callbacks));
expectEqual(state.autoSyncInFlight, true, "locks the complete detection-to-refresh cycle immediately");
expectEqual(
  await pollAutoSync(optionsFor(state, invoke, request, callbacks)),
  false,
  "skips a timer tick while the previous detection is still pending",
);
expectEqual(detectionCalls, 1, "does not start a second detection during an in-flight poll");

resolveDetection(true);
expectEqual(await firstPoll, true, "completes the first poll after detection finishes");
expectEqual(state.autoSyncInFlight, false, "releases the poll lock after the full cycle finishes");

const transitionState = { autoSyncInFlight: false };
const transitionRequest = { current: 10, profile: "transition-profile", wasRunning: true as boolean | null };
const transitionCallbacks = { statuses: [] as string[], errors: [] as unknown[], synced: [] as string[] };
let resolveTransitionSync!: (value: unknown) => void;
const transitionSync = new Promise<unknown>((resolve) => {
  resolveTransitionSync = resolve;
});
let transitionSyncStarted = false;
let transitionRefreshStarted = false;
let resolveTransitionRefresh!: () => void;
const transitionRefresh = new Promise<void>((resolve) => {
  resolveTransitionRefresh = resolve;
});
const transitionOptions = optionsFor(
  transitionState,
  <T>(command: string) => {
    if (command === "detect_codex_running") return Promise.resolve(false as T);
    if (command === "apply_database_sync_from_local") {
      transitionSyncStarted = true;
      return transitionSync as Promise<T>;
    }
    throw new Error(`unexpected command: ${command}`);
  },
  transitionRequest,
  transitionCallbacks,
);
transitionOptions.onSync = async (_report, profile) => {
  transitionSyncStarted = true;
  transitionRefreshStarted = true;
  await transitionRefresh;
  transitionCallbacks.synced.push(profile);
};
const transitionPoll = pollAutoSync(transitionOptions);
await Promise.resolve();
expectEqual(transitionSyncStarted, true, "keeps the poll lock while synchronization is pending");
expectEqual(
  await pollAutoSync(transitionOptions),
  false,
  "skips a timer tick while synchronization and result refresh are pending",
);
resolveTransitionSync({ applied_items: 1 });
await Promise.resolve();
expectEqual(transitionRefreshStarted, true, "keeps the same cycle open through result refresh");
resolveTransitionRefresh();
expectEqual(await transitionPoll, true, "completes the stop transition cycle");
expectEqual(transitionState.autoSyncInFlight, false, "releases the lock only after synchronization and refresh");

const staleSyncCallbacks = { statuses: [] as string[], errors: [] as unknown[], synced: [] as string[] };
const staleSyncRequest = { current: 20, profile: "old-profile", wasRunning: true as boolean | null };
const staleSyncState = { autoSyncInFlight: false };
let resolveStaleSync!: (value: unknown) => void;
const staleSync = new Promise<unknown>((resolve) => {
  resolveStaleSync = resolve;
});
const staleSyncOptions = optionsFor(
  staleSyncState,
  <T>(command: string) => {
    if (command === "detect_codex_running") return Promise.resolve(false as T);
    if (command === "apply_database_sync_from_local") return staleSync as Promise<T>;
    throw new Error(`unexpected command: ${command}`);
  },
  staleSyncRequest,
  staleSyncCallbacks,
);
const staleSyncPoll = pollAutoSync(staleSyncOptions);
await Promise.resolve();
staleSyncRequest.current = 21;
staleSyncRequest.profile = "new-profile";
resolveStaleSync({ applied_items: 1 });
expectEqual(await staleSyncPoll, true, "completes a synchronization started for the old profile");
expectEqual(staleSyncCallbacks.synced, [], "does not apply an old synchronization result after switching profiles");
expectEqual(staleSyncCallbacks.statuses, [], "does not apply an old synchronization status to the new profile");

let failureCalls = 0;
const failureCallbacks = { statuses: [] as string[], errors: [] as unknown[], synced: [] as string[] };
const failureRequest = { current: 2, profile: "failure-profile", wasRunning: null as boolean | null };
const failureResult = await pollAutoSync(
  optionsFor(
    state,
    async <T>() => {
      failureCalls += 1;
      throw new Error("probe failed");
    },
    failureRequest,
    failureCallbacks,
  ),
);
expectEqual(failureResult, true, "handles a failed background detection without opening a dialog");
expectEqual(failureCalls, 1, "attempts the failed detection once");
expectEqual(failureCallbacks.errors.length, 1, "reports the detection failure to the non-modal status callback");
expectEqual(failureCallbacks.synced.length, 0, "does not start synchronization after detection fails");
expectEqual(state.autoSyncInFlight, false, "releases the lock after a failed detection");

const staleCallbacks = { statuses: [] as string[], errors: [] as unknown[], synced: [] as string[] };
const staleRequest = { current: 3, profile: "old-profile", wasRunning: true as boolean | null };
let resolveStaleDetection!: (value: boolean) => void;
const staleDetection = new Promise<boolean>((resolve) => {
  resolveStaleDetection = resolve;
});
const stalePoll = pollAutoSync(
  optionsFor(
    state,
    async <T>(command: string) => {
      if (command !== "detect_codex_running") throw new Error(`unexpected command: ${command}`);
      return (await staleDetection) as T;
    },
    staleRequest,
    staleCallbacks,
  ),
);
staleRequest.current = 4;
staleRequest.profile = "new-profile";
resolveStaleDetection(false);
expectEqual(await stalePoll, true, "completes an old poll after the profile changes");
expectEqual(staleCallbacks.statuses, [], "does not apply an old poll status to the new profile");
expectEqual(staleCallbacks.synced, [], "does not synchronize an old profile after switching");
expectEqual(staleCallbacks.errors, [], "does not report an old profile failure to the new profile");
