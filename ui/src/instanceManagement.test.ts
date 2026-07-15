import {
  instanceAvailability,
  managedInstanceDeleteConfirmation,
  managedInstanceIgnoreConfirmation,
  instanceDisplayName,
  instanceScanSummary,
} from "./instanceManagement.js";

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
