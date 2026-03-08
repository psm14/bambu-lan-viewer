import { useVideoStream } from "../hooks/useVideoStream";

const PLAYLIST_LABEL = "Chunked CMAF (MSE)";

function formatOverlayValue(value, suffix = "") {
  if (value == null || Number.isNaN(Number(value))) {
    return "--";
  }
  return `${Math.round(Number(value))}${suffix}`;
}

export default function VideoCard({
  apiBase,
  selectedPrinterId,
  selectedPrinter,
  connected,
  statusLabel,
  progressPercent,
  layerDisplay,
  remainingDisplay,
  nozzleTemp,
  bedTemp,
}) {
  const {
    videoRef,
    videoError,
    videoKey,
    showVideoMenu,
    handleVideoPointerDown,
    reloadVideo,
  } = useVideoStream({ apiBase, selectedPrinterId });

  const progressLabel =
    progressPercent != null && !Number.isNaN(progressPercent)
      ? `${Math.round(progressPercent)}%`
      : "--";

  return (
    <div className="card video-card eva-video-card">
      <div className={`video-shell eva-video-shell ${showVideoMenu ? "show-menu" : ""}`}>
        <video
          key={videoKey}
          ref={videoRef}
          autoPlay
          muted
          playsInline
          className="video"
          onPointerDown={handleVideoPointerDown}
        />
        <div className="video-scanlines eva-scanlines" aria-hidden="true" />
        <div className="eva-video-grid" aria-hidden="true" />
        <div className="video-overlay eva-video-overlay">
          <div className="video-hud-top eva-video-top">
            <div className="video-badge eva-badge mono">
              {selectedPrinter ? `${selectedPrinter.name} // PRIMARY FEED` : "PRIMARY FEED // STANDBY"}
            </div>
            <div className={`video-status-chip eva-status-chip ${connected ? "ok" : "warn"} mono`}>
              {connected ? "TARGET LOCK" : "NO HANDSHAKE"}
            </div>
          </div>

          <div className="eva-targeting" aria-hidden="true">
            <span className="eva-target-ring outer" />
            <span className="eva-target-ring inner" />
            <span className="eva-target-cross horizontal" />
            <span className="eva-target-cross vertical" />
          </div>

          <div className="video-corners eva-video-corners" aria-hidden="true">
            <span />
            <span />
            <span />
            <span />
          </div>

          <div className="eva-video-side-readouts mono">
            <div>
              <span>PRG</span>
              <strong>{progressLabel}</strong>
            </div>
            <div>
              <span>LAYER</span>
              <strong>{layerDisplay || "--"}</strong>
            </div>
            <div>
              <span>ETA</span>
              <strong>{remainingDisplay || "--"}</strong>
            </div>
          </div>

          <div className="video-hud-bottom eva-video-bottom">
            <div className="video-readout eva-video-readout mono">
              <span>Route</span>
              <strong>{selectedPrinter?.host ?? "No uplink"}</strong>
            </div>
            <div className="video-readout eva-video-readout mono">
              <span>Status</span>
              <strong>{statusLabel}</strong>
            </div>
            <div className="video-readout eva-video-readout mono">
              <span>Feed</span>
              <strong>{PLAYLIST_LABEL}</strong>
            </div>
            <div className="video-readout eva-video-readout mono">
              <span>Thermals</span>
              <strong>{formatOverlayValue(nozzleTemp, "°")} / {formatOverlayValue(bedTemp, "°")}</strong>
            </div>
          </div>

          <details className="video-menu eva-video-menu">
            <summary className="video-menu-toggle eva-video-menu-toggle" aria-label="Video options">
              <span aria-hidden="true">SYS</span>
            </summary>
            <div className="video-menu-panel eva-video-menu-panel">
              <button
                type="button"
                onClick={reloadVideo}
                disabled={!selectedPrinterId}
              >
                Reload Video
              </button>
              <p className="video-menu-text">
                {selectedPrinter
                  ? `Streaming ${selectedPrinter.name} via RTSPS → ${PLAYLIST_LABEL}.`
                  : "Add a printer to start streaming."}
              </p>
            </div>
          </details>
          {videoError && <div className="video-error">{videoError}</div>}
        </div>
      </div>
    </div>
  );
}
