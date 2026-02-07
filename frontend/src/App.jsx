import { useState } from "react";
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

  return (
    <div className="app">
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
      />

      <section className="grid">
        <VideoCard
          apiBase={API_BASE}
          selectedPrinterId={selectedPrinterId}
          selectedPrinter={selectedPrinter}
        />
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
        />
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
      </section>

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
