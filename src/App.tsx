import "./app.css";
import { BridgeDashboard } from "./bridge-dashboard";
import { useBridgeState } from "./bridge-state";

export default function App() {
  const state = useBridgeState();
  return <BridgeDashboard {...state} />;
}
