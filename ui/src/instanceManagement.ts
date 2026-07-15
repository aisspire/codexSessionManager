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
  already_managed: number;
  skipped: number;
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
  return report
    ? `最近扫描：新增 ${report.added} 个 · 已存在 ${report.already_managed} 个 · 跳过 ${report.skipped} 个`
    : "扫描只会登记路径，不会切换当前 Codex 主目录或修改实例配置。";
}

function instanceDefaultName(path: string) {
  const segments = path.replace(/[\\/]+$/, "").split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] || path;
}
