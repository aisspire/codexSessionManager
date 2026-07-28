export interface NativeDialogOpenOptions {
  multiple: false;
  directory: boolean;
}

export type PathPickerTarget =
  | "codex-home"
  | "instance-scan-path"
  | "edit-project"
  | "setting-codex-cli";

export type NativeDialogOpen = (
  options: NativeDialogOpenOptions,
) => Promise<string | string[] | null>;

export interface PathFieldMarkupOptions {
  target: PathPickerTarget;
  label: string;
  value: string;
  escapeHtml: (value: string) => string;
  placeholder?: string;
  disabled?: boolean;
}

export interface RegisteredCodexHomeOption {
  id: number;
  label: string;
  path: string;
  available: boolean;
}

export interface RegisteredCodexHomePickerMarkupOptions extends PathFieldMarkupOptions {
  instances: RegisteredCodexHomeOption[];
  selectedInstanceId: number | null;
}

export async function pickSinglePath(open: NativeDialogOpen, directory: boolean) {
  const selectedPath = await open({ multiple: false, directory });
  return typeof selectedPath === "string" ? selectedPath : null;
}

export function pathPickerDirectory(target: PathPickerTarget) {
  return target !== "setting-codex-cli";
}

export function pathFieldMarkup(options: PathFieldMarkupOptions) {
  const disabled = options.disabled ? "disabled" : "";
  const placeholder = options.placeholder
    ? ` placeholder="${options.escapeHtml(options.placeholder)}"`
    : "";
  const buttonLabel = pathPickerDirectory(options.target) ? "选择文件夹" : "选择文件";
  return `
    <div class="path-field">
      <label for="${options.target}">${options.escapeHtml(options.label)}</label>
      <span class="path-input-control">
        <input id="${options.target}"${placeholder} value="${options.escapeHtml(options.value)}" ${disabled} />
        <button type="button" data-pick-path="${options.target}" ${disabled}>${buttonLabel}</button>
      </span>
    </div>
  `;
}

export function registeredCodexHomePickerMarkup(options: RegisteredCodexHomePickerMarkupOptions) {
  const disabled = options.disabled ? "disabled" : "";
  const selectedInstanceId = options.selectedInstanceId;
  const availableInstances = options.instances.filter((instance) => instance.available);
  return `
    <div class="registered-codex-home-picker">
      <label class="registered-codex-home-select" for="registered-codex-home">
        已登记实例
        <select id="registered-codex-home" ${disabled}>
          <option value="" ${selectedInstanceId == null ? "selected" : ""}>手动输入目录</option>
          ${availableInstances
            .map(
              (instance) =>
                `<option value="${instance.id}" ${instance.id === selectedInstanceId ? "selected" : ""}>${options.escapeHtml(instance.label)} · ${options.escapeHtml(instance.path)}</option>`,
            )
            .join("")}
        </select>
      </label>
      ${pathFieldMarkup(options)}
    </div>
  `;
}
