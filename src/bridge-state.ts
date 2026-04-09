import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import {
  formatError,
  normalizeFilter,
  pickSource,
  runExportFiles,
  runPreview,
} from "./bridge-conversion";
import {
  appendPushProgressDetail,
  formatPushProgressStatus,
  listenPushProgress,
} from "./push-progress";

const HEALTH_INITIAL = "等待检查 Sub2Api 连接";
const UPDATE_INITIAL = "等待检查应用更新";
const PREVIEW_INITIAL = "等待执行来源预览";
const PUSH_INITIAL = "等待推送到 Sub2Api";

type PushField = "baseUrl" | "email" | "password";
type SourceKind = "directory" | "file";
type PushSetter = Dispatch<SetStateAction<PushFormState>>;
type StringSetter = Dispatch<SetStateAction<string>>;
type PushRequestOptions = { base_url: string; email: string; password: string };

export type SourceSelection = { kind: SourceKind; path: string };
export type PreviewAccount = { name?: string; platform?: string; type?: string; extra?: { email?: string }; [key: string]: unknown };
export type PreviewExport = { exported_at: string; proxies: unknown[]; accounts: PreviewAccount[] };
export type ConversionPreview = { scanned_files: number; converted_files: number; skipped_files: number; export: PreviewExport };
export type ExportAccountsResult = { exported_files: number; file_path: string };
export type PushSummary = { total: number; success: number; failure: number; skipped?: number; canceled: boolean };
export type PushFormState = { baseUrl: string; email: string; password: string };
export type AppUpdateStatus = {
  configured: boolean;
  available: boolean;
  current_version: string;
  latest_version?: string | null;
  notes?: string | null;
  pub_date?: string | null;
  message: string;
};
export type PushProgressView = {
  current: number;
  total: number;
  percent: number;
  visible: boolean;
};

const DEFAULT_PUSH_FORM: PushFormState = {
  baseUrl: "http://127.0.0.1:8080",
  email: "",
  password: "",
};

const EMPTY_PUSH_PROGRESS: PushProgressView = {
  current: 0,
  total: 0,
  percent: 0,
  visible: false,
};

// 统一维护页面上的来源、预览、连接和推送状态。
export function useBridgeState() {
  const [source, setSource] = useState<SourceSelection | null>(null);
  const [typeFilter, setTypeFilter] = useState("");
  const [appVersion, setAppVersion] = useState("");
  const [health, setHealth] = useState(HEALTH_INITIAL);
  const [updateStatus, setUpdateStatus] = useState(UPDATE_INITIAL);
  const [previewStatus, setPreviewStatus] = useState(PREVIEW_INITIAL);
  const [previewExport, setPreviewExport] = useState<PreviewExport | null>(null);
  const [exportLoading, setExportLoading] = useState(false);
  const [pushStatus, setPushStatus] = useState(PUSH_INITIAL);
  const [pushDetails, setPushDetails] = useState("");
  const [pushProgress, setPushProgress] = useState<PushProgressView>(EMPTY_PUSH_PROGRESS);
  const [loading, setLoading] = useState(false);
  const [updateLoading, setUpdateLoading] = useState(false);
  const [pushLoading, setPushLoading] = useState(false);
  const [cancelPushLoading, setCancelPushLoading] = useState(false);
  const [push, setPush] = useState(DEFAULT_PUSH_FORM);
  const onPushChange = (field: PushField, value: string) => {
    updatePushField(field, value, setPush, setHealth);
  };

  useEffect(() => {
    if (!pushLoading) setCancelPushLoading(false);
  }, [pushLoading]);

  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => setAppVersion(""));
  }, []);

  return {
    appVersion,
    source,
    typeFilter,
    health,
    updateStatus,
    previewStatus,
    previewJson: previewExport,
    exportLoading,
    pushStatus,
    pushDetails,
    pushProgress,
    loading,
    updateLoading,
    pushLoading,
    cancelPushLoading,
    push,
    onTypeFilterChange: setTypeFilter,
    onPickDirectory: () => pickSource("directory", setSource, setPreviewStatus, setPreviewExport),
    onPickFile: () => pickSource("file", setSource, setPreviewStatus, setPreviewExport),
    onPushChange,
    onHealthCheck: () => runHealthCheck(push, setHealth),
    onCheckUpdate: () => runCheckUpdate(setUpdateLoading, setUpdateStatus),
    onPreview: () => runPreview(source, typeFilter, setLoading, setPreviewStatus, setPreviewExport),
    onExportFiles: () => runExportFiles(source, previewExport, setExportLoading, setPreviewStatus),
    onPush: () => runPush(source, typeFilter, push, setPushLoading, setPushStatus, setPushDetails, setPushProgress),
    onCancelPush: () => runCancelPush(pushLoading, setCancelPushLoading, setPushStatus),
  };
}

function updatePushField(
  field: PushField,
  value: string,
  setPush: PushSetter,
  setHealth: (value: string) => void,
) {
  setPush((current) => ({ ...current, [field]: value }));
  setHealth("连接参数已变更，请重新检查 Sub2Api。");
}

// 只校验 Sub2Api 登录是否成功，不拉取额外业务数据。
async function runHealthCheck(push: PushFormState, setHealth: (value: string) => void) {
  try {
    await invoke("check_sub2api_connection", {
      options: buildConnection(push),
    });
    setHealth("连接成功，登录校验通过");
  } catch (error) {
    setHealth(`连接失败: ${formatError(error)}`);
  }
}

async function runCheckUpdate(
  setUpdateLoading: (value: boolean) => void,
  setUpdateStatus: (value: string) => void,
) {
  setUpdateLoading(true);
  setUpdateStatus("正在检查更新...");

  try {
    const result = await invoke<AppUpdateStatus>("check_app_update");
    setUpdateStatus(result.message);
    if (!result.available) return;

    const nextVersion = result.latest_version ? `v${result.latest_version}` : "新版本";
    const installConfirmed = window.confirm(
      `发现 ${nextVersion}。\n\n是否立即下载安装并重启应用？`,
    );
    if (!installConfirmed) {
      setUpdateStatus(`已发现 ${nextVersion}，你可以稍后再次点击“检查更新”安装。`);
      return;
    }

    setUpdateStatus(`正在下载安装 ${nextVersion}...`);
    await invoke("install_app_update");
  } catch (error) {
    setUpdateStatus(`检查更新失败: ${formatError(error)}`);
  } finally {
    setUpdateLoading(false);
  }
}

async function runPush(
  source: SourceSelection | null,
  typeFilter: string,
  push: PushFormState,
  setPushLoading: (value: boolean) => void,
  setPushStatus: (value: string) => void,
  setPushDetails: StringSetter,
  setPushProgress: (value: PushProgressView) => void,
) {
  if (!source) return resetPush(setPushStatus, setPushDetails, setPushProgress, "请先选择 CPA 目录或文件");
  let stopListening = () => {};
  setPushLoading(true);
  setPushStatus("准备推送...");
  setPushDetails("");
  setPushProgress({ ...EMPTY_PUSH_PROGRESS, visible: true });

  try {
    stopListening = await listenPushProgress((event) => {
      setPushStatus(formatPushProgressStatus(event));
      setPushDetails((current) => appendPushProgressDetail(current, event));
      setPushProgress(buildPushProgress(
        event.stage === "started" ? event.index - 1 : event.index,
        event.total,
      ));
    });

    // 真正的上传工作在 Rust 侧完成，这里只负责发起和更新 UI。
    const result = await invoke<PushSummary>("push_cpa_source_to_sub2api", {
      sourcePath: source.path,
      sourceKind: source.kind,
      typeFilter: normalizeFilter(typeFilter),
      options: buildConnection(push),
    });

    const skipped = result.skipped ?? 0;
    const skippedPart = skipped > 0 ? ` / ${skipped} 条已存在` : "";

    if (result.canceled) {
      const canceledMessage = `已停止推送 · 成功 ${result.success} 条 / 失败 ${result.failure} 条${skippedPart} / 总共 ${result.total} 条`;
      setPushStatus(canceledMessage);
      setPushDetails((current) => {
        return current ? `${current}\n已停止推送，未继续后续账号` : "已停止推送，未继续后续账号";
      });
      return;
    }

    setPushStatus(`成功 ${result.success} 条 / 失败 ${result.failure} 条${skippedPart} / 总共 ${result.total} 条`);
  } catch (error) {
    const message = `推送失败: ${formatError(error)}`;
    setPushStatus(message);
    setPushDetails((current) => (current ? `${current}\n${message}` : message));
  } finally {
    stopListening();
    setPushLoading(false);
    setPushProgress(EMPTY_PUSH_PROGRESS);
  }
}

async function runCancelPush(
  pushLoading: boolean,
  setCancelPushLoading: (value: boolean) => void,
  setPushStatus: (value: string) => void,
) {
  if (!pushLoading) return;
  setCancelPushLoading(true);
  setPushStatus("正在停止推送...");

  try {
    await invoke("cancel_cpa_push");
  } catch (error) {
    setCancelPushLoading(false);
    setPushStatus(`停止推送失败: ${formatError(error)}`);
  }
}

function resetPush(
  setPushStatus: (value: string) => void,
  setPushDetails: StringSetter,
  setPushProgress: (value: PushProgressView) => void,
  status: string,
) {
  setPushStatus(status);
  setPushDetails("");
  setPushProgress(EMPTY_PUSH_PROGRESS);
}

function buildConnection(push: PushFormState): PushRequestOptions {
  const baseUrl = push.baseUrl.trim();
  const email = push.email.trim();
  if (!baseUrl) throw new Error("请输入接口地址");
  if (!email) throw new Error("请输入管理员邮箱");
  if (!push.password) throw new Error("请输入管理员密码");
  return { base_url: baseUrl, email, password: push.password };
}

// 进度条只展示“实际上传中的账号数”，已存在账号由汇总文案体现。
function buildPushProgress(current: number, total: number): PushProgressView {
  if (total <= 0) return EMPTY_PUSH_PROGRESS;
  const safeCurrent = Math.max(0, Math.min(current, total));
  return {
    current: safeCurrent,
    total,
    percent: Math.round((safeCurrent / total) * 100),
    visible: true,
  };
}







