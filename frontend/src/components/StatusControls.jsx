import { formatPercent } from "../utils/format";

export default function StatusControls({
  jobStateDisplay,
  lightIsOn,
  canToggleLight,
  handleLightToggle,
  progressPercent,
  pauseResumeLabel,
  stopLabel,
  homeLabel,
  canPauseResume,
  canStop,
  canHome,
  canJog,
  canExtrude,
  pendingExtruderAction,
  handlePauseResume,
  handleStop,
  handleHome,
  handleJog,
  handleExtrude,
  xyJogStepMm,
  zJogStepMm,
  extruderStepMm,
  isPaused,
  layerDisplay,
  remainingDisplay,
  selectedPrinterId,
  error,
}) {
  const extrudeForwardLabel =
    pendingExtruderAction === "forward"
      ? "Extruding..."
      : `Extrude +${extruderStepMm}mm`;
  const extrudeReverseLabel =
    pendingExtruderAction === "reverse"
      ? "Retracting..."
      : `Retract ${extruderStepMm}mm`;

  return (
    <div className="card status-controls">
      <div className="status-header">
        <div className="status-title">{jobStateDisplay}</div>
        <button
          type="button"
          className={`icon-button light ${lightIsOn ? "on" : "off"}`}
          aria-pressed={lightIsOn}
          aria-label={lightIsOn ? "Turn light off" : "Turn light on"}
          disabled={!canToggleLight}
          onClick={handleLightToggle}
          title={lightIsOn ? "Light On" : "Light Off"}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M8 21h8m-7-3h6m1-10a5 5 0 1 0-8 3.9V14a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2v-2.1A5 5 0 0 0 16 8Z"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            {lightIsOn && (
              <path
                d="M12 3v1M5.5 6.5l.7.7M18.5 6.5l-.7.7M4 12h1M19 12h1"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.4"
                strokeLinecap="round"
              />
            )}
          </svg>
        </button>
      </div>
      <div className="progress-row">
        <div
          className="progress-track"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={progressPercent ?? 0}
        >
          <div
            className="progress-fill"
            style={{
              width: progressPercent != null ? `${progressPercent}%` : "0%",
            }}
          />
          <span className="progress-label">
            {formatPercent(progressPercent)}
          </span>
        </div>
        <div className="progress-actions">
          <button
            type="button"
            className="icon-button"
            aria-label={pauseResumeLabel}
            disabled={!canPauseResume}
            onClick={handlePauseResume}
            title={pauseResumeLabel}
          >
            {isPaused ? (
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M8 6l10 6-10 6V6Z" fill="currentColor" />
              </svg>
            ) : (
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <rect x="6" y="5" width="4" height="14" rx="1" />
                <rect x="14" y="5" width="4" height="14" rx="1" />
              </svg>
            )}
          </button>
          <button
            type="button"
            className="icon-button danger"
            aria-label={stopLabel}
            disabled={!canStop}
            onClick={handleStop}
            title={stopLabel}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <rect x="6" y="6" width="12" height="12" rx="2" />
            </svg>
          </button>
        </div>
      </div>
      <div className="progress-meta">
        <span className="layer-count">{layerDisplay}</span>
        <span className="remaining">{remainingDisplay}</span>
      </div>
      <div className="motion-controls">
        <div className="motion-header">
          <span className="motion-title"></span>
          <button
            type="button"
            className="home-button"
            disabled={!canHome}
            onClick={handleHome}
            title={homeLabel}
          >
            {homeLabel}
          </button>
        </div>
        <div className="jog-cluster">
          <div className="jog-grid" role="group" aria-label="X and Y motion controls">
            <span className="jog-spacer" aria-hidden="true" />
            <button
              type="button"
              className="jog-button"
              disabled={!canJog}
              onClick={() => handleJog("y", "positive")}
              title={`Move Y +${xyJogStepMm}mm`}
            >
              Y+
            </button>
            <span className="jog-spacer" aria-hidden="true" />
            <button
              type="button"
              className="jog-button"
              disabled={!canJog}
              onClick={() => handleJog("x", "negative")}
              title={`Move X -${xyJogStepMm}mm`}
            >
              X-
            </button>
            <span className="jog-center" aria-hidden="true">
              XY
            </span>
            <button
              type="button"
              className="jog-button"
              disabled={!canJog}
              onClick={() => handleJog("x", "positive")}
              title={`Move X +${xyJogStepMm}mm`}
            >
              X+
            </button>
            <span className="jog-spacer" aria-hidden="true" />
            <button
              type="button"
              className="jog-button"
              disabled={!canJog}
              onClick={() => handleJog("y", "negative")}
              title={`Move Y -${xyJogStepMm}mm`}
            >
              Y-
            </button>
            <span className="jog-spacer" aria-hidden="true" />
          </div>
          <div className="jog-z-column" role="group" aria-label="Z motion controls">
            <button
              type="button"
              className="jog-button"
              disabled={!canJog}
              onClick={() => handleJog("z", "negative")}
              title={`Move Z +${zJogStepMm}mm`}
            >
              Z+
            </button>
            <button
              type="button"
              className="jog-button"
              disabled={!canJog}
              onClick={() => handleJog("z", "positive")}
              title={`Move Z -${zJogStepMm}mm`}
            >
              Z-
            </button>
          </div>
        </div>
        <p className="motion-note">Step: XY {xyJogStepMm}mm, Z {zJogStepMm}mm</p>
        <div className="extruder-controls" role="group" aria-label="Extruder controls">
          <span className="motion-title">Extruder</span>
          <div className="extruder-actions">
            <button
              type="button"
              className="jog-button"
              disabled={!canExtrude}
              onClick={() => handleExtrude("reverse")}
              title={extrudeReverseLabel}
            >
              {extrudeReverseLabel}
            </button>
            <button
              type="button"
              className="jog-button"
              disabled={!canExtrude}
              onClick={() => handleExtrude("forward")}
              title={extrudeForwardLabel}
            >
              {extrudeForwardLabel}
            </button>
          </div>
        </div>
      </div>
      {!selectedPrinterId && (
        <p className="helper">Select a printer to view status.</p>
      )}
      {error && <p className="error">{error}</p>}
    </div>
  );
}
