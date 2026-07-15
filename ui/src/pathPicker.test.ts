import {
  pathFieldMarkup,
  pathPickerDirectory,
  pickSinglePath,
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
