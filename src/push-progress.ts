import { listen } from "@tauri-apps/api/event";

const PUSH_PROGRESS_EVENT = "sub2api://push-progress";

type PushProgressStage = "started" | "succeeded" | "failed";

export type PushProgressEvent = {
  index: number;
  total: number;
  account_name: string;
  success: number;
  failure: number;
  stage: PushProgressStage;
  reason?: string | null;
};

export async function listenPushProgress(
  onProgress: (event: PushProgressEvent) => void,
) {
  return listen<PushProgressEvent>(PUSH_PROGRESS_EVENT, (event) => onProgress(event.payload));
}

export function formatPushProgressStatus(event: PushProgressEvent) {
  const summary = `成功 ${event.success} 条 / 失败 ${event.failure} 条 / 总共 ${event.total} 条`;
  const prefix = `[${event.index}/${event.total}]`;
  const name = event.account_name || "未命名账号";
  if (event.stage === "started") return `正在推送 ${prefix} ${name} · ${summary}`;
  if (event.stage === "succeeded") return `刚完成 ${prefix} ${name} · ${summary}`;
  return `推送失败 ${prefix} ${name} · ${summary}`;
}

export function appendPushProgressDetail(
  current: string,
  event: PushProgressEvent,
) {
  const line = formatDetailLine(event);
  return current ? `${current}\n${line}` : line;
}

function formatDetailLine(event: PushProgressEvent) {
  const prefix = `[${event.index}/${event.total}]`;
  const name = event.account_name || "未命名账号";
  if (event.stage === "started") return `${prefix} 正在推送: ${name}`;
  if (event.stage === "succeeded") return `${prefix} 推送成功: ${name}`;
  return `${prefix} 推送失败: ${name} - ${event.reason || "未知错误"}`;
}
