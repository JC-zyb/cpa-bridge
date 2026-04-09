import { PushFormState } from "./bridge-state";
import {
  ConversionPanel,
  ConversionPanelProps,
  PushPanel,
  PushPanelProps,
} from "./dashboard-panels";
import appMark from "./assets/app-mark.svg";

export type BridgeDashboardProps = ConversionPanelProps & {
  appVersion: string;
  health: string;
  updateStatus: string;
  pushStatus: string;
  pushDetails: string;
  pushProgress: { current: number; total: number; percent: number; visible: boolean };
  updateLoading: boolean;
  pushLoading: boolean;
  cancelPushLoading: boolean;
  push: PushFormState;
  onHealthCheck: () => void;
  onCheckUpdate: () => void;
  onPushChange: PushPanelProps["onChange"];
  onPush: () => void;
  onCancelPush: () => void;
};

function Topbar(
  props: Pick<BridgeDashboardProps, "appVersion" | "health" | "updateStatus" | "updateLoading" | "onHealthCheck" | "onCheckUpdate">,
) {
  return (
    <header className="topbar">
      <div className="brand-section">
        <img src={appMark} alt="cpa-bridge" className="brand-mark" />
        <div className="brand-block">
          <p className="brand-label">CPA → Sub2Api</p>
          <div className="brand-title-row">
            <h1 className="brand-title">cpa-bridge</h1>
            {props.appVersion ? <span className="brand-version">v{props.appVersion}</span> : null}
          </div>
        </div>
      </div>
      <div className="status-panel">
        <div className="status-card">
          <div className="status-row">
            <p className="status-label">连接状态</p>
            <p className="status-text compact-status">{props.health}</p>
            <button type="button" className="toolbar-button" onClick={props.onHealthCheck}>
              检查连接
            </button>
          </div>
          <div className="status-row">
            <p className="status-label">更新状态</p>
            <p className="status-text compact-status">{props.updateStatus}</p>
            <button type="button" className="toolbar-button" onClick={props.onCheckUpdate} disabled={props.updateLoading}>
              {props.updateLoading ? "检查中..." : "检查更新"}
            </button>
          </div>
        </div>
      </div>
    </header>
  );
}

export function BridgeDashboard(props: BridgeDashboardProps) {
  return (
    <main className="app-shell">
      <section className="app-frame">
        <Topbar
          appVersion={props.appVersion}
          health={props.health}
          updateStatus={props.updateStatus}
          updateLoading={props.updateLoading}
          onHealthCheck={props.onHealthCheck}
          onCheckUpdate={props.onCheckUpdate}
        />
        <section className="workspace-grid">
          <ConversionPanel
            source={props.source}
            typeFilter={props.typeFilter}
            previewStatus={props.previewStatus}
            previewJson={props.previewJson}
            exportLoading={props.exportLoading}
            loading={props.loading}
            onTypeFilterChange={props.onTypeFilterChange}
            onPickDirectory={props.onPickDirectory}
            onPickFile={props.onPickFile}
            onPreview={props.onPreview}
            onExportFiles={props.onExportFiles}
          />
          <PushPanel
            push={props.push}
            status={props.pushStatus}
            details={props.pushDetails}
            progress={props.pushProgress}
            loading={props.pushLoading}
            cancelLoading={props.cancelPushLoading}
            onChange={props.onPushChange}
            onPush={props.onPush}
            onCancelPush={props.onCancelPush}
          />
        </section>
      </section>
    </main>
  );
}

