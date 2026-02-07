import { useVideoStream } from "../hooks/useVideoStream";

const PLAYLIST_LABEL = "Chunked CMAF (MSE)";

export default function VideoCard({ apiBase, selectedPrinterId, selectedPrinter }) {
  const {
    videoRef,
    videoError,
    videoKey,
    showVideoMenu,
    handleVideoPointerDown,
    reloadVideo,
  } = useVideoStream({ apiBase, selectedPrinterId });

  return (
    <div className="card video-card">
      <div className={`video-shell ${showVideoMenu ? "show-menu" : ""}`}>
        <video
          key={videoKey}
          ref={videoRef}
          autoPlay
          muted
          playsInline
          className="video"
          onPointerDown={handleVideoPointerDown}
        />
        <div className="video-overlay">
          <details className="video-menu">
            <summary className="video-menu-toggle" aria-label="Video options">
              <span aria-hidden="true">⋯</span>
            </summary>
            <div className="video-menu-panel">
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
