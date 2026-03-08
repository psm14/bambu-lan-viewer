import { useMemo, useState } from "react";
import "./App.css";
import Header from "./components/Header";
import PrinterManagerDrawer from "./components/PrinterManagerDrawer";
import StatusControls from "./components/StatusControls";
import TemperatureCard from "./components/TemperatureCard";
import AmsFilamentCard from "./components/AmsFilamentCard";
import VideoCard from "./components/VideoCard";
import { usePrinterControls } from "./hooks/usePrinterControls";
import { usePrinters } from "./hooks/usePrinters";
import { usePrinterStatus } from "./hooks/usePrinterStatus";
import { useSession } from "./hooks/useSession";

function formatMetricValue(value, suffix = "") {
  if (value == null || Number.isNaN(Number(value))) {
    return "--";
  }
  return `${Math.round(Number(value))}${suffix}`;
}

const API_BASE = import.meta.env.VITE_API_BASE ?? "";
const POLL_MS = 3000;

export default function App() {
  const [error, setError] = useState("");
  const [showManager, setShowManager] = useState(false);

  const {
    printers,
    selectedPrinterId,
    setSelectedPrinterId,
    loadingPrinters,
    loadPrinters,
  } = usePrinters({ apiBase: API_BASE, onError: setError });

  const selectedPrinter =
    printers.find((printer) => printer.id === selectedPrinterId) ?? null;

  const status = usePrinterStatus({
    apiBase: API_BASE,
    selectedPrinterId,
    pollMs: POLL_MS,
    onError: setError,
  });

  const userEmail = useSession({ apiBase: API_BASE });

  const {
    connected,
    isPaused,
    jobStateDisplay,
    statusLabel,
    progressPercent,
    layerDisplay,
    remainingDisplay,
    lightIsOn,
    pauseResumeLabel,
    stopLabel,
    homeLabel,
    canPauseResume,
    canStop,
    canToggleLight,
    canHome,
    canJog,
    canSetTemperature,
    canExtrude,
    pendingTempAction,
    pendingExtruderAction,
    nozzleTargetLabel,
    bedTargetLabel,
    handlePauseResume,
    handleStop,
    handleLightToggle,
    handleHome,
    handleJog,
    handleSetNozzleTarget,
    handleSetBedTarget,
    handleExtrude,
    xyJogStepMm,
    zJogStepMm,
    extruderStepMm,
  } = usePrinterControls({
    apiBase: API_BASE,
    selectedPrinterId,
    status,
    onError: setError,
  });

  const telemetry = useMemo(
    () => [
      {
        label: "Link",
        value: connected ? "Online" : "Offline",
        accent: connected ? "ok" : "warn",
      },
      {
        label: "Job",
        value: jobStateDisplay || "Standby",
        accent: connected ? "hot" : "warn",
      },
      {
        label: "Progress",
        value:
          progressPercent != null && !Number.isNaN(progressPercent)
            ? `${Math.round(progressPercent)}%`
            : "--",
      },
      { label: "Layer", value: layerDisplay || "--" },
      { label: "ETA", value: remainingDisplay || "--" },
      { label: "Nozzle", value: formatMetricValue(status?.nozzleC, "°") },
      { label: "Bed", value: formatMetricValue(status?.bedC, "°") },
      { label: "Chamber", value: formatMetricValue(status?.chamberC, "°") },
    ],
    [
      connected,
      jobStateDisplay,
      layerDisplay,
      progressPercent,
      remainingDisplay,
      status?.bedC,
      status?.chamberC,
      status?.nozzleC,
    ],
  );

  return (
    <div className="app">
      <div className="app-shell shell-eva">
        <section className="mission-layout">
          <div className="mission-video">
            <VideoCard
              apiBase={API_BASE}
              selectedPrinterId={selectedPrinterId}
              selectedPrinter={selectedPrinter}
              connected={connected}
              statusLabel={statusLabel}
              progressPercent={progressPercent}
              layerDisplay={layerDisplay}
              remainingDisplay={remainingDisplay}
              nozzleTemp={status?.nozzleC}
              bedTemp={status?.bedC}
            />
          </div>

          <div className="mission-side">
            <Header
              printers={printers}
              selectedPrinterId={selectedPrinterId}
              loadingPrinters={loadingPrinters}
              selectedPrinter={selectedPrinter}
              onSelectPrinter={setSelectedPrinterId}
              onOpenManager={() => setShowManager(true)}
              userEmail={userEmail}
              connected={connected}
              statusLabel={statusLabel}
              jobStateDisplay={jobStateDisplay}
            />

            <section className="telemetry-strip telemetry-grid-eva" aria-label="Printer telemetry overview">
              {telemetry.map((item) => (
                <div
                  key={item.label}
                  className={`telemetry-chip telemetry-frame ${item.accent ? `telemetry-chip-${item.accent}` : ""}`}
                >
                  <span className="telemetry-label mono">{item.label}</span>
                  <strong>{item.value}</strong>
                </div>
              ))}
            </section>
          </div>
        </section>

        <section className="control-grid">
          <div className="control-primary">
            <StatusControls
              jobStateDisplay={jobStateDisplay}
              lightIsOn={lightIsOn}
              canToggleLight={canToggleLight}
              handleLightToggle={handleLightToggle}
              progressPercent={progressPercent}
              pauseResumeLabel={pauseResumeLabel}
              stopLabel={stopLabel}
              homeLabel={homeLabel}
              canPauseResume={canPauseResume}
              canStop={canStop}
              handlePauseResume={handlePauseResume}
              handleStop={handleStop}
              canHome={canHome}
              canJog={canJog}
              canExtrude={canExtrude}
              pendingExtruderAction={pendingExtruderAction}
              handleHome={handleHome}
              handleJog={handleJog}
              handleExtrude={handleExtrude}
              xyJogStepMm={xyJogStepMm}
              zJogStepMm={zJogStepMm}
              extruderStepMm={extruderStepMm}
              isPaused={isPaused}
              layerDisplay={layerDisplay}
              remainingDisplay={remainingDisplay}
              selectedPrinterId={selectedPrinterId}
              error={error}
              connected={connected}
            />
          </div>

          <div className="systems-stack">
            <TemperatureCard
              status={status}
              selectedPrinterId={selectedPrinterId}
              canSetTemperature={canSetTemperature}
              pendingTempAction={pendingTempAction}
              nozzleTargetLabel={nozzleTargetLabel}
              bedTargetLabel={bedTargetLabel}
              handleSetNozzleTarget={handleSetNozzleTarget}
              handleSetBedTarget={handleSetBedTarget}
            />
            <AmsFilamentCard status={status} />
          </div>
        </section>
      </div>

      {showManager && (
        <PrinterManagerDrawer
          apiBase={API_BASE}
          printers={printers}
          loadingPrinters={loadingPrinters}
          selectedPrinterId={selectedPrinterId}
          setSelectedPrinterId={setSelectedPrinterId}
          loadPrinters={loadPrinters}
          onClose={() => setShowManager(false)}
          onError={setError}
        />
      )}
    </div>
  );
}
