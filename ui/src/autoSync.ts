export interface AutoSyncPollState {
  autoSyncInFlight: boolean;
}

export interface AutoSyncPollOptions<Profile, SyncReport> {
  state: AutoSyncPollState;
  enabled: () => boolean;
  getProfile: () => Profile;
  getRequestId: () => number;
  isCurrentRequest: (requestId: number) => boolean;
  invoke: <T>(command: string, args: { profile: Profile }) => Promise<T>;
  wasRunning: () => boolean | null;
  setWasRunning: (running: boolean) => void;
  onStatus: (status: string) => void;
  onSync: (report: SyncReport, profile: Profile, requestId: number) => Promise<void>;
  onError: (error: unknown, requestId: number) => void;
}

export async function pollAutoSync<Profile, SyncReport>(
  options: AutoSyncPollOptions<Profile, SyncReport>,
): Promise<boolean> {
  if (!options.enabled() || options.state.autoSyncInFlight) return false;

  options.state.autoSyncInFlight = true;
  const requestId = options.getRequestId();
  const profile = options.getProfile();
  try {
    const running = await options.invoke<boolean>("detect_codex_running", { profile });
    if (!options.isCurrentRequest(requestId)) return true;

    if (options.wasRunning() === true && !running) {
      const report = await options.invoke<SyncReport>("apply_database_sync_from_local", { profile });
      if (!options.isCurrentRequest(requestId)) return true;
      await options.onSync(report, profile, requestId);
      if (!options.isCurrentRequest(requestId)) return true;
    } else {
      options.onStatus(running ? "Codex 运行中，等待停止后同步" : "Codex 未运行");
    }
    options.setWasRunning(running);
    return true;
  } catch (error) {
    if (options.isCurrentRequest(requestId)) {
      options.onError(error, requestId);
    }
    return true;
  } finally {
    options.state.autoSyncInFlight = false;
  }
}
