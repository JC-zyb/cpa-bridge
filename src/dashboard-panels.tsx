import { useEffect, useRef } from "react";
import { type PreviewExport, type PushProgressView, PushFormState, SourceSelection } from "./bridge-state";

type PushChangeField = "baseUrl" | "email" | "password";

export type ConversionPanelProps = {
  source: SourceSelection | null;
  typeFilter: string;
  previewStatus: string;
  previewJson: PreviewExport | null;
  exportLoading: boolean;
  loading: boolean;
  onTypeFilterChange: (value: string) => void;
  onPickDirectory: () => void;
  onPickFile: () => void;
  onPreview: () => void;
  onExportFiles: () => void;
};

export type PushPanelProps = {
  push: PushFormState;
  status: string;
  details: string;
  progress: PushProgressView;
  loading: boolean;
  cancelLoading: boolean;
  onChange: (field: PushChangeField, value: string) => void;
  onPush: () => void;
  onCancelPush: () => void;
};

const DETAIL_FOLLOW_THRESHOLD = 24;
const EMPTY_PREVIEW_TEXT = "这里显示转换后的 sub2api 账号列表";
const GROUP_NOTICE = "当前上传使用总文件导入接口，不会自动绑定分组；请导入后在 Sub2Api 后台手动绑定。";

type SectionHeaderProps = { title: string; description: string };
type SourceActionButtonProps = { label: string; hint: string; onClick: () => void };
type InputProps = {
  label: string;
  value: string;
  placeholder?: string;
  type?: string;
  onChange: (value: string) => void;
};

function SectionHeader(props: SectionHeaderProps) {
  return (
    <div className="section-header">
      <h2>{props.title}</h2>
      <p>{props.description}</p>
    </div>
  );
}

function SourceActionButton(props: SourceActionButtonProps) {
  return (
    <button type="button" className="secondary-button source-action-button" onClick={props.onClick}>
      <span className="source-action-label">{props.label}</span>
      <span className="source-action-hint">{props.hint}</span>
    </button>
  );
}

function SourceSummary(props: { source: SourceSelection | null }) {
  const badge = props.source?.kind === "file" ? "单文件" : "目录";
  const text = props.source?.path ?? "未选择 CPA 目录或 JSON 文件";
  return (
    <div className="source-summary surface-panel">
      <div className="summary-topline">
        <span className="source-badge">{props.source ? badge : "未选择"}</span>
        <span className="subsection-label">当前来源</span>
      </div>
      <p className="source-path">{text}</p>
    </div>
  );
}

function LabeledInput(props: InputProps) {
  return (
    <label>
      <span>{props.label}</span>
      <input
        type={props.type ?? "text"}
        value={props.value}
        placeholder={props.placeholder}
        onChange={(event) => props.onChange(event.target.value)}
      />
    </label>
  );
}

function StatusRow(props: { label: string; value: string }) {
  return (
    <div className="panel-status-row">
      <span className="subsection-label">{props.label}</span>
      <p className="status-text">{props.value}</p>
    </div>
  );
}

function PreviewTextBox(props: { previewJson: PreviewExport | null }) {
  const content = props.previewJson
    ? JSON.stringify(props.previewJson, null, 2)
    : EMPTY_PREVIEW_TEXT;
  return <pre className="preview-box preview-text-box preview-surface">{content}</pre>;
}

function PushStatusArea(
  props: Pick<
    PushPanelProps,
    "status" | "details" | "progress" | "loading" | "cancelLoading" | "onPush" | "onCancelPush"
  >,
) {
  const detailRef = useRef<HTMLTextAreaElement | null>(null);
  const autoFollowRef = useRef(true);

  useEffect(() => {
    const detailNode = detailRef.current;
    if (!detailNode) return;
    if (!props.details) autoFollowRef.current = true;
    if (!autoFollowRef.current) return;
    detailNode.scrollTop = detailNode.scrollHeight;
  }, [props.details]);

  const handleDetailScroll = () => {
    const detailNode = detailRef.current;
    if (!detailNode) return;
    const distanceToBottom = detailNode.scrollHeight - detailNode.scrollTop - detailNode.clientHeight;
    autoFollowRef.current = distanceToBottom <= DETAIL_FOLLOW_THRESHOLD;
  };

  const handleCancelPush = () => {
    if (!window.confirm("确认停止当前批量推送吗？")) return;
    props.onCancelPush();
  };

  return (
    <section className="push-section push-footer surface-panel">
      <div className="section-stack">
        <span className="subsection-label">上传结果</span>
        {props.loading ? (
          <div className="push-grid-two">
            <button type="button" className="primary-button push-submit" disabled>
              推送中...
            </button>
            <button
              type="button"
              className="secondary-button push-submit"
              onClick={handleCancelPush}
              disabled={props.cancelLoading}
            >
              {props.cancelLoading ? "停止中..." : "停止推送"}
            </button>
          </div>
        ) : (
          <button type="button" className="primary-button push-submit" onClick={props.onPush}>
            开始推送
          </button>
        )}
      </div>
      <StatusRow label="推送状态" value={props.status} />
      {props.progress.visible ? (
        <div
          className="push-progress-inline"
          aria-label={
            props.progress.total > 0
              ? `推送进度 ${props.progress.current}/${props.progress.total}`
              : "推送进度准备中"
          }
          title={
            props.progress.total > 0
              ? `推送进度 ${props.progress.current}/${props.progress.total}`
              : "正在准备推送"
          }
        >
          <div className="progress-track progress-track-thin">
            <div
              className={`progress-fill${props.progress.total > 0 ? "" : " progress-fill-indeterminate"}`}
              style={props.progress.total > 0 ? { width: `${props.progress.percent}%` } : undefined}
            />
          </div>
        </div>
      ) : null}
      <textarea
        ref={detailRef}
        className="preview-box detail-box"
        readOnly
        value={props.details}
        placeholder="失败明细会显示在这里"
        onScroll={handleDetailScroll}
      />
    </section>
  );
}

export function ConversionPanel(props: ConversionPanelProps) {
  return (
    <section className="workspace-panel conversion-panel">
      <SectionHeader title="数据来源" description="先选来源，再预览转换结果。" />
      <div className="source-toolbar">
        <SourceActionButton label="选择目录" hint="批量扫描 JSON" onClick={props.onPickDirectory} />
        <SourceActionButton label="选择文件" hint="处理单个 JSON" onClick={props.onPickFile} />
      </div>
      <SourceSummary source={props.source} />
      <div className="conversion-tools surface-panel">
        <div className="section-stack">
          <span className="subsection-label">筛选与预览</span>
          <LabeledInput
            label="类型过滤"
            value={props.typeFilter}
            placeholder="可选，如 codex"
            onChange={props.onTypeFilterChange}
          />
        </div>
        <button
          type="button"
          className="primary-button preview-button"
          onClick={props.onPreview}
          disabled={props.loading}
        >
          {props.loading ? "预览中..." : "开始预览"}
        </button>
      </div>
      <StatusRow label="预览状态" value={props.previewStatus} />
      <PreviewTextBox previewJson={props.previewJson} />
      <div className="surface-panel section-stack">
        <span className="subsection-label">导出总文件</span>
        <button
          type="button"
          className="primary-button export-button"
          data-busy={props.exportLoading ? "true" : "false"}
          aria-busy={props.exportLoading ? "true" : "false"}
          onClick={props.onExportFiles}
          disabled={props.exportLoading || !props.previewJson}
        >
          {props.exportLoading ? "导出中..." : "导出总文件"}
        </button>
      </div>
    </section>
  );
}

export function PushPanel(props: PushPanelProps) {
  return (
    <section className="workspace-panel push-panel">
      <SectionHeader title="推送配置" description="填写连接信息后直接上传到 Sub2Api。" />
      <section className="push-section surface-panel">
        <span className="subsection-label">连接信息</span>
        <LabeledInput
          label="接口地址"
          value={props.push.baseUrl}
          placeholder="例如 http://127.0.0.1:8080"
          onChange={(value) => props.onChange("baseUrl", value)}
        />
        <div className="push-grid-two">
          <LabeledInput
            label="管理员邮箱"
            value={props.push.email}
            placeholder="请输入管理员邮箱"
            onChange={(value) => props.onChange("email", value)}
          />
          <LabeledInput
            label="管理员密码"
            type="password"
            value={props.push.password}
            placeholder="请输入管理员密码"
            onChange={(value) => props.onChange("password", value)}
          />
        </div>
      </section>
      <section className="push-section surface-panel">
        <span className="subsection-label">分组说明</span>
        <p className="meta-text">{GROUP_NOTICE}</p>
      </section>
      <PushStatusArea
        status={props.status}
        details={props.details}
        progress={props.progress}
        loading={props.loading}
        cancelLoading={props.cancelLoading}
        onPush={props.onPush}
        onCancelPush={props.onCancelPush}
      />
    </section>
  );
}
