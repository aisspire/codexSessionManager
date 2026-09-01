import {
  pathFieldMarkup,
  pathPickerDirectory,
  pickSinglePath,
  registeredCodexHomePickerMarkup,
  type NativeDialogOpen,
} from "./pathPicker.js";

function expectEqual<T>(actual: T, expected: T, message: string) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`);
  }
}

async function expectSinglePath(directory: boolean, expectedPath: string) {
  let receivedOptions: { multiple: false; directory: boolean } | undefined;
  const open: NativeDialogOpen = async (options) => {
    receivedOptions = options;
    return expectedPath;
  };

  expectEqual(await pickSinglePath(open, directory), expectedPath, "returns the selected path");
  expectEqual(
    receivedOptions,
    { multiple: false, directory },
    "opens a single-selection dialog for the requested path kind",
  );
}

await expectSinglePath(true, "C:\\Users\\me\\.codex");
await expectSinglePath(false, "C:\\Users\\me\\AppData\\Roaming\\npm\\codex.cmd");

const cancelled: NativeDialogOpen = async () => null;
expectEqual(await pickSinglePath(cancelled, true), null, "keeps the current value when the dialog is cancelled");

const multipleResults: NativeDialogOpen = async () => ["C:\\one", "C:\\two"];
expectEqual(await pickSinglePath(multipleResults, false), null, "ignores unexpected multiple selections");

expectEqual(pathPickerDirectory("codex-home"), true, "Codex home uses a directory picker");
expectEqual(pathPickerDirectory("instance-scan-path"), true, "instance scan uses a directory picker");
expectEqual(pathPickerDirectory("edit-project"), true, "project editing uses a directory picker");
expectEqual(pathPickerDirectory("setting-codex-cli"), false, "Codex CLI uses a file picker");

const fieldMarkup = pathFieldMarkup({
  target: "codex-home",
  label: "Codex 主目录",
  value: "C:\\Users\\me\\.codex",
  escapeHtml: (value) => value,
});
if (!fieldMarkup.includes('<label for="codex-home">Codex 主目录</label>')) {
  throw new Error("path field should use an explicit label association");
}
if (fieldMarkup.indexOf("</label>") > fieldMarkup.indexOf("<button")) {
  throw new Error("path picker button must not be nested inside the input label");
}
if (!fieldMarkup.includes("选择文件夹")) {
  throw new Error("directory field should describe the picker action");
}

const registeredPickerMarkup = registeredCodexHomePickerMarkup({
  target: "codex-home",
  label: "Codex 主目录",
  value: "E:\\Codex\\office",
  escapeHtml: (value) => value,
  selectedInstanceId: 1,
  instances: [
    { id: 1, label: "办公室", path: "E:\\Codex\\office", availability: "available" },
    { id: 2, label: "已失效", path: "E:\\Codex\\missing", availability: "unavailable" },
    { id: 4, label: "尚未检测", path: "\\\\wsl.localhost\\Ubuntu\\home\\dev\\.codex", availability: "unknown" },
  ],
});
if (!registeredPickerMarkup.includes('id="registered-codex-home"')) {
  throw new Error("registered Codex home picker should include the instance selector");
}
if (!registeredPickerMarkup.includes('value="1" selected')) {
  throw new Error("registered Codex home picker should select the active instance");
}
if (registeredPickerMarkup.includes("已失效")) {
  throw new Error("registered Codex home picker should omit unavailable instances");
}
if (!registeredPickerMarkup.includes("尚未检测")) {
  throw new Error("registered Codex home picker should keep unknown instances selectable");
}

const selectedUnavailableMarkup = registeredCodexHomePickerMarkup({
  target: "codex-home",
  label: "Codex 主目录",
  value: "E:\\Codex\\missing",
  escapeHtml: (value) => value,
  selectedInstanceId: 2,
  instances: [{ id: 2, label: "已失效", path: "E:\\Codex\\missing", availability: "unavailable" }],
});
if (!selectedUnavailableMarkup.includes('value="2" selected disabled')) {
  throw new Error("the selected unavailable instance should stay visible and disabled until switching manually");
}

const wslPickerMarkup = registeredCodexHomePickerMarkup({
  target: "codex-home",
  label: "Codex 主目录",
  value: "\\\\wsl.localhost\\Ubuntu\\home\\dev\\.codex",
  escapeHtml: (value) => value,
  selectedInstanceId: 3,
  readonly: true,
  hidePicker: true,
  instances: [
    {
      id: 3,
      label: "Ubuntu · dev",
      runtimeLabel: "WSL · Ubuntu",
      path: "\\\\wsl.localhost\\Ubuntu\\home\\dev\\.codex",
      availability: "available",
    },
  ],
});
if (!wslPickerMarkup.includes("readonly") || wslPickerMarkup.includes("选择文件夹")) {
  throw new Error("selected WSL Codex home should be read-only until manual input is selected");
}
