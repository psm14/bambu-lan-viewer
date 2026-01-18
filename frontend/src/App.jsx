import { useEffect, useRef, useState } from "react";
import Hls from "hls.js";
import "./App.css";

const API_BASE = import.meta.env.VITE_API_BASE ?? "";
const POLL_MS = 3000;

const commandLabels = {
  pause: "Pause",
  resume: "Resume",
  stop: "Stop",
  lightOn: "Light On",
  lightOff: "Light Off",
};

function formatTemp(value) {
  if (value == null || Number.isNaN(value)) {
    return "--";
  }
  return `${value.toFixed(1)} C`;
}

function formatMinutes(value) {
  if (value == null || Number.isNaN(value)) {
    return "--";
  }
  return `${value} min`;
}

function formatPercent(value) {
  if (value == null || Number.isNaN(value)) {
    return "--";
  }
  return `${value}%`;
}

export default function App() {
  const [status, setStatus] = useState(null);
  const [error, setError] = useState("");
  const [videoError, setVideoError] = useState("");
  const [busyCommand, setBusyCommand] = useState("");
  const [videoReload, setVideoReload] = useState(0);
  const videoRef = useRef(null);

  const hlsUrl = `${API_BASE}/hls/stream.m3u8`;

  useEffect(() => {
    let isActive = true;

    const fetchStatus = async () => {
      try {
        const response = await fetch(`${API_BASE}/api/status`);
        if (!response.ok) {
          throw new Error("status fetch failed");
        }
        const data = await response.json();
        if (isActive) {
          setStatus(data);
          setError("");
        }
      } catch (err) {
        if (isActive) {
          setError("Unable to reach backend");
        }
      }
    };

    fetchStatus();
    const timer = setInterval(fetchStatus, POLL_MS);
    return () => {
      isActive = false;
      clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) {
      return;
    }
    setVideoError("");

    const onVideoError = () => {
      setVideoError("Video element error");
    };
    video.addEventListener("error", onVideoError);

    if (video.canPlayType("application/vnd.apple.mpegurl")) {
      video.src = hlsUrl;
      return () => {
        video.removeEventListener("error", onVideoError);
        video.removeAttribute("src");
        video.load();
      };
    }

    if (Hls.isSupported()) {
      const hls = new Hls({
        enableWorker: true,
        backBufferLength: 0,
      });
      hls.on(Hls.Events.MANIFEST_PARSED, () => {
        video.play().catch(() => {});
      });
      hls.on(Hls.Events.ERROR, (_, data) => {
        const message = `HLS error: ${data.type} ${data.details} fatal=${data.fatal}`;
        console.error(message, data);
        if (data.fatal) {
          setVideoError(message);
          switch (data.type) {
            case Hls.ErrorTypes.NETWORK_ERROR:
              hls.startLoad();
              break;
            case Hls.ErrorTypes.MEDIA_ERROR:
              hls.recoverMediaError();
              break;
            default:
              hls.destroy();
              break;
          }
        }
      });
      hls.loadSource(hlsUrl);
      hls.attachMedia(video);
      return () => {
        video.removeEventListener("error", onVideoError);
        hls.destroy();
      };
    }

    setVideoError("HLS is not supported in this browser");
    return () => {
      video.removeEventListener("error", onVideoError);
      video.removeAttribute("src");
      video.load();
    };
  }, [hlsUrl, videoReload]);

  const connected = status?.connected === true;
  const jobState = status?.jobState ?? "UNKNOWN";
  const lastUpdate = status?.lastUpdate ? new Date(status.lastUpdate) : null;
  const staleSeconds =
    lastUpdate && !Number.isNaN(lastUpdate.getTime())
      ? Math.floor((Date.now() - lastUpdate.getTime()) / 1000)
      : null;
  const isStale = staleSeconds != null && staleSeconds > 15;

  const sendCommand = async (payload, label) => {
    setBusyCommand(label);
    try {
      const response = await fetch(`${API_BASE}/api/command`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      const data = await response.json().catch(() => ({}));
      if (!response.ok || data.ok === false) {
        throw new Error(data.error || "command failed");
      }
      setError("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "command failed");
    } finally {
      setBusyCommand("");
    }
  };

  return (
    <div className="app">
      <header className="hero">
        <div>
          <p className="eyebrow">Bambu LAN Viewer</p>
          <h1>Printer Status + Controls</h1>
          <p className="subhead">
            MQTT status polling with direct control commands. Video streaming
            comes next.
          </p>
        </div>
        <div className={`pill ${connected ? "ok" : "warn"}`}>
          {connected ? "MQTT Connected" : "Offline"}
        </div>
      </header>

      <section className="grid">
        <div className="card video-card">
          <div className="video-header">
            <h2>Camera</h2>
            <button
              type="button"
              onClick={() => setVideoReload((value) => value + 1)}
            >
              Reload Video
            </button>
          </div>
          <video
            key={videoReload}
            ref={videoRef}
            controls
            muted
            playsInline
            className="video"
          />
          <p className="helper">Streaming via RTSPS -> HLS.</p>
          {videoError && <p className="error">{videoError}</p>}
        </div>
        <div className="card">
          <h2>Status</h2>
          <div className="stat">
            <span>Job</span>
            <strong>{jobState}</strong>
          </div>
          <div className="stat">
            <span>Progress</span>
            <strong className="mono">{formatPercent(status?.percent)}</strong>
          </div>
          <div className="stat">
            <span>Remaining</span>
            <strong className="mono">
              {formatMinutes(status?.remainingMinutes)}
            </strong>
          </div>
          <div className="stat">
            <span>Light</span>
            <strong>{status?.light ?? "--"}</strong>
          </div>
          <div className="stat">
            <span>Last Update</span>
            <strong className={`mono ${isStale ? "stale" : ""}`}>
              {lastUpdate ? lastUpdate.toLocaleTimeString() : "--"}
            </strong>
          </div>
          {isStale && (
            <p className="stale-note">No updates for {staleSeconds}s.</p>
          )}
        </div>

        <div className="card">
          <h2>Temperatures</h2>
          <div className="stat">
            <span>Nozzle</span>
            <strong className="mono">{formatTemp(status?.nozzleC)}</strong>
          </div>
          <div className="stat">
            <span>Bed</span>
            <strong className="mono">{formatTemp(status?.bedC)}</strong>
          </div>
          <div className="stat">
            <span>Chamber</span>
            <strong className="mono">{formatTemp(status?.chamberC)}</strong>
          </div>
        </div>

        <div className="card">
          <h2>Controls</h2>
          <div className="controls">
            <button
              type="button"
              disabled={!connected || busyCommand === commandLabels.pause}
              onClick={() =>
                sendCommand({ type: "pause" }, commandLabels.pause)
              }
            >
              {commandLabels.pause}
            </button>
            <button
              type="button"
              disabled={!connected || busyCommand === commandLabels.resume}
              onClick={() =>
                sendCommand({ type: "resume" }, commandLabels.resume)
              }
            >
              {commandLabels.resume}
            </button>
            <button
              type="button"
              className="danger"
              disabled={!connected || busyCommand === commandLabels.stop}
              onClick={() => sendCommand({ type: "stop" }, commandLabels.stop)}
            >
              {commandLabels.stop}
            </button>
            <button
              type="button"
              disabled={!connected || busyCommand === commandLabels.lightOn}
              onClick={() =>
                sendCommand({ type: "light", on: true }, commandLabels.lightOn)
              }
            >
              {commandLabels.lightOn}
            </button>
            <button
              type="button"
              disabled={!connected || busyCommand === commandLabels.lightOff}
              onClick={() =>
                sendCommand(
                  { type: "light", on: false },
                  commandLabels.lightOff,
                )
              }
            >
              {commandLabels.lightOff}
            </button>
          </div>
          {error && <p className="error">{error}</p>}
        </div>
      </section>
    </div>
  );
}
