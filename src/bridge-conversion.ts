import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  ConversionPreview,
  ExportAccountsResult,
  PreviewExport,
  SourceSelection,
} from "./bridge-state";

type PreviewSetter = (value: PreviewExport | null) => void;
type SourceSetter = (value: SourceSelection | null) => void;
type StatusSetter = (value: string) => void;
type LoadingSetter = (value: boolean) => void;

export async function pickSource(
  kind: SourceSelection["kind"],
  setSource: SourceSetter,
  setPreviewStatus: StatusSetter,
  setPreviewExport: PreviewSetter,
) {
  try {
    const selected = await openSourceDialog(kind);
    if (typeof selected !== "string") return;
    setSource({ kind, path: selected });
    setPreviewStatus(`已选择${kind === "directory" ? "目录" : "文件"}，可以开始预览或推送`);
    setPreviewExport(null);
  } catch (error) {
    resetPreview(setPreviewStatus, setPreviewExport, `选择${kind === "directory" ? "目录" : "文件"}失败: ${formatError(error)}`);
  }
}

export async function runPreview(
  source: SourceSelection | null,
  typeFilter: string,
  setLoading: LoadingSetter,
  setPreviewStatus: StatusSetter,
  setPreviewExport: PreviewSetter,
) {
  if (!source) {
    resetPreview(setPreviewStatus, setPreviewExport, "请先选择 CPA 目录或文件");
    return;
  }
  setLoading(true);
  try {
    const result = await invoke<ConversionPreview>("preview_cpa_source", {
      sourcePath: source.path,
      sourceKind: source.kind,
      typeFilter: normalizeFilter(typeFilter),
    });
    setPreviewStatus(`扫描 ${result.scanned_files} 个文件，转换 ${result.converted_files} 个，跳过 ${result.skipped_files} 个`);
    setPreviewExport(result.export);
  } catch (error) {
    resetPreview(setPreviewStatus, setPreviewExport, `转换失败: ${formatError(error)}`);
  } finally {
    setLoading(false);
  }
}

export async function runExportFiles(
  source: SourceSelection | null,
  previewExport: PreviewExport | null,
  setExportLoading: LoadingSetter,
  setPreviewStatus: StatusSetter,
) {
  if (!previewExport) {
    setPreviewStatus("请先执行来源预览");
    return;
  }
  if (!source) {
    setPreviewStatus("请先选择 CPA 目录或文件");
    return;
  }
  try {
    const targetFile = await save({
      filters: [{ name: "JSON", extensions: ["json"] }],
      defaultPath: buildExportFileName(source, previewExport),
    });
    if (typeof targetFile !== "string") return;
    setExportLoading(true);
    const result = await invoke<ExportAccountsResult>("export_cpa_preview_accounts", {
      exportData: previewExport,
      targetFile,
    });
    setPreviewStatus(`已导出 ${result.exported_files} 个文件到 ${result.file_path}`);
  } catch (error) {
    setPreviewStatus(`导出失败: ${formatError(error)}`);
  } finally {
    setExportLoading(false);
  }
}

export function normalizeFilter(typeFilter: string) {
  return typeFilter.trim() || null;
}

export function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function resetPreview(
  setPreviewStatus: StatusSetter,
  setPreviewExport: PreviewSetter,
  status: string,
) {
  setPreviewStatus(status);
  setPreviewExport(null);
}

function buildExportFileName(
  source: SourceSelection | null,
  previewExport: PreviewExport | null,
) {
  const stamp = formatExportStamp(previewExport?.exported_at);
  const fallback = `sub2api-account-${stamp}.json`;
  if (!source) return fallback;
  const basePath = source.kind === "directory"
    ? source.path
    : source.path.replace(/[^\\/]+$/, "");
  const separator = basePath.endsWith("\\") || basePath.endsWith("/") ? "" : "\\";
  return `${basePath}${separator}${fallback}`;
}

function formatExportStamp(exportedAt?: string) {
  const digits = (exportedAt ?? "").replace(/[^0-9]/g, "").slice(0, 14);
  if (digits.length === 14) return digits;
  const date = new Date();
  const pad = (value: number) => value.toString().padStart(2, "0");
  return `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
}

function openSourceDialog(kind: SourceSelection["kind"]) {
  return kind === "directory"
    ? open({ directory: true, multiple: false })
    : open({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
}
