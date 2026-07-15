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
