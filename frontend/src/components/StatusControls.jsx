import { formatPercent } from "../utils/format";

function RingGauge({ progressPercent }) {
  const normalized = Math.max(0, Math.min(100, Number(progressPercent) || 0));
  const radius = 58;
  const circumference = 2 * Math.PI * radius;
  const dashOffset = circumference - (normalized / 100) * circumference;

  return (
    <div className="progress-dial eva-progress-dial" aria-hidden="true">
      <svg viewBox="0 0 160 160">
        <defs>
          <linearGradient id="evaDialGradient" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#ff6b3d" />
            <stop offset="55%" stopColor="#ffd25a" />
            <stop offset="100%" stopColor="#8ef6ff" />
          </linearGradient>
        </defs>
        <circle className="dial-track" cx="80" cy="80" r={radius} />
        <circle className="eva-dial-tick-ring" cx="80" cy="80" r="68" />
        <circle
          className="dial-progress eva-dial-progress"
          cx="80"
          cy="80"
          r={radius}
          style={{ strokeDasharray: circumference, strokeDashoffset: dashOffset }}
        />
        <path d="M80 24 118 80 80 136 42 80Z" className="eva-dial-diamond" />
        <circle className="dial-core eva-dial-core" cx="80" cy="80" r="34" />
      </svg>
      <div className="dial-label eva-dial-label">
        <span className="mono">Sync ratio</span>
        <strong>{formatPercent(progressPercent)}</strong>
        <small className="mono">Magi route</small>
      </div>
    </div>
  );
}

function MotionPad({ canJog, handleJog, xyJogStepMm, zJogStepMm }) {
  return (
    <div className="motion-panel eva-motion-panel">
      <div className="motion-pad-shell eva-motion-pad-shell">
        <svg className="motion-pad-art eva-motion-pad-art" viewBox="0 0 320 320" aria-hidden="true">
          <path
            d="M160 18 220 78 220 122 264 122 302 160 264 198 220 198 220 242 160 302 100 242 100 198 56 198 18 160 56 122 100 122 100 78Z"
            className="motion-pad-outline eva-motion-pad-outline"
          />
          <path d="M160 42v236M42 160h236" className="motion-pad-grid eva-motion-pad-grid" />
          <path d="M160 78 214 160 160 242 106 160Z" className="eva-motion-pad-inner" />
          <circle cx="160" cy="160" r="26" className="motion-pad-core eva-motion-pad-core" />
          <circle cx="160" cy="160" r="54" className="eva-motion-target" />
        </svg>

        <button
          type="button"
          className="pad-button eva-pad-button pad-up"
          disabled={!canJog}
          onClick={() => handleJog("y", "positive")}
          title={`Move Y +${xyJogStepMm}mm`}
        >
          Y+
        </button>
        <button
          type="button"
          className="pad-button eva-pad-button pad-right"
          disabled={!canJog}
          onClick={() => handleJog("x", "positive")}
          title={`Move X +${xyJogStepMm}mm`}
        >
          X+
        </button>
        <button
          type="button"
          className="pad-button eva-pad-button pad-down"
          disabled={!canJog}
          onClick={() => handleJog("y", "negative")}
          title={`Move Y -${xyJogStepMm}mm`}
        >
          Y-
        </button>
        <button
          type="button"
          className="pad-button eva-pad-button pad-left"
          disabled={!canJog}
          onClick={() => handleJog("x", "negative")}
          title={`Move X -${xyJogStepMm}mm`}
        >
          X-
        </button>
        <div className="pad-center eva-pad-center mono">AXIS</div>
      </div>

      <div className="z-rail eva-z-rail" role="group" aria-label="Z motion controls">
        <div className="eva-z-cap mono">ELEVATION</div>
        <button
          type="button"
          className="pad-button eva-pad-button z-up"
          disabled={!canJog}
          onClick={() => handleJog("z", "negative")}
          title={`Move Z +${zJogStepMm}mm`}
        >
          Z+
        </button>
        <div className="z-rail-track eva-z-rail-track mono">STEP {zJogStepMm}MM</div>
        <button
          type="button"
          className="pad-button eva-pad-button z-down"
          disabled={!canJog}
          onClick={() => handleJog("z", "positive")}
          title={`Move Z -${zJogStepMm}mm`}
        >
          Z-
        </button>
      </div>
    </div>
  );
}

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
  connected,
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
    <div className="card status-controls eva-status-controls">
      <div className="status-command-head eva-command-head">
        <div>
          <span className="section-kicker mono">Command lattice</span>
          <div className="status-title eva-status-title">{jobStateDisplay}</div>
        </div>
        <button
          type="button"
          className={`icon-button light eva-light-button ${lightIsOn ? "on" : "off"}`}
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

      <div className="command-overview eva-command-overview">
        <RingGauge progressPercent={progressPercent} />

        <div className="command-stats eva-command-stats">
          <div className="command-stat-tile eva-stat-tile">
            <span className="mono">Layer trace</span>
            <strong>{layerDisplay}</strong>
          </div>
          <div className="command-stat-tile eva-stat-tile">
            <span className="mono">Time vector</span>
            <strong>{remainingDisplay}</strong>
          </div>
          <div className={`command-stat-tile eva-stat-tile ${connected ? "is-online" : "is-offline"}`}>
            <span className="mono">Signal</span>
            <strong>{connected ? "Synced" : "Lost"}</strong>
          </div>
        </div>
      </div>

      <div className="transport-controls eva-transport-controls">
        <button
          type="button"
          className="transport-button eva-transport-button"
          aria-label={pauseResumeLabel}
          disabled={!canPauseResume}
          onClick={handlePauseResume}
          title={pauseResumeLabel}
        >
          <span className="transport-icon" aria-hidden="true">
            {isPaused ? (
              <svg viewBox="0 0 24 24">
                <path d="M8 6l10 6-10 6V6Z" fill="currentColor" />
              </svg>
            ) : (
              <svg viewBox="0 0 24 24">
                <rect x="6" y="5" width="4" height="14" rx="1" />
                <rect x="14" y="5" width="4" height="14" rx="1" />
              </svg>
            )}
          </span>
          <span>{pauseResumeLabel}</span>
        </button>

        <button
          type="button"
          className="transport-button eva-transport-button danger"
          aria-label={stopLabel}
          disabled={!canStop}
          onClick={handleStop}
          title={stopLabel}
        >
          <span className="transport-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <rect x="6" y="6" width="12" height="12" rx="2" />
            </svg>
          </span>
          <span>{stopLabel}</span>
        </button>

        <button
          type="button"
          className="transport-button eva-transport-button"
          disabled={!canHome}
          onClick={handleHome}
          title={homeLabel}
        >
          <span className="transport-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <path
                d="M4 11.5 12 5l8 6.5M7 10v8h10v-8"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.6"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </span>
          <span>{homeLabel}</span>
        </button>
      </div>

      <div className="motion-controls eva-motion-controls">
        <div className="motion-header command-section-header">
          <span className="section-kicker mono">Vector controls</span>
          <span className="motion-title">XY step {xyJogStepMm}mm • Z step {zJogStepMm}mm</span>
        </div>
        <MotionPad
          canJog={canJog}
          handleJog={handleJog}
          xyJogStepMm={xyJogStepMm}
          zJogStepMm={zJogStepMm}
        />

        <div className="extruder-controls eva-extruder-controls" role="group" aria-label="Extruder controls">
          <div className="command-section-header">
            <span className="section-kicker mono">Material drive</span>
            <span className="motion-title">Pulse {extruderStepMm}mm</span>
          </div>
          <div className="extruder-actions eva-extruder-actions">
            <button
              type="button"
              className="jog-button eva-jog-button"
              disabled={!canExtrude}
              onClick={() => handleExtrude("reverse")}
              title={extrudeReverseLabel}
            >
              {extrudeReverseLabel}
            </button>
            <button
              type="button"
              className="jog-button eva-jog-button"
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
        <p className="helper">Select a printer to unlock command routing.</p>
      )}
      {error && <p className="error">{error}</p>}
    </div>
  );
}
