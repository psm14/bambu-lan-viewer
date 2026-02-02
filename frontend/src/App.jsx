import { useEffect, useRef, useState } from "react";
import Hls from "hls.js";
import "./App.css";

const API_BASE = import.meta.env.VITE_API_BASE ?? "";
const POLL_MS = 3000;

const JOB_TIMEOUT_MS = 5000;
const LIGHT_TIMEOUT_MS = 3000;

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
  const [pendingJobAction, setPendingJobAction] = useState(null);
  const [lightOverride, setLightOverride] = useState(null);
  const [pendingLightToken, setPendingLightToken] = useState(null);
  const [videoReload, setVideoReload] = useState(0);
  const videoRef = useRef(null);
  const pendingJobTokenRef = useRef(null);
  const pendingJobTimeoutRef = useRef(null);
  const pendingLightTokenRef = useRef(null);
  const pendingLightTimeoutRef = useRef(null);

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
    return () => {
      if (pendingJobTimeoutRef.current) {
        clearTimeout(pendingJobTimeoutRef.current);
      }
      if (pendingLightTimeoutRef.current) {
        clearTimeout(pendingLightTimeoutRef.current);
      }
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
  const normalizedJobState = normalizeJobState(status?.jobState);
  const isPaused = normalizedJobState === "paused";
  const isPrinting = normalizedJobState === "printing";
  const lightFromStatus = normalizeLight(status?.light);
  const lightIsOn = lightOverride ?? lightFromStatus ?? false;
  const canPauseResume =
    connected && pendingJobAction == null && (isPrinting || isPaused);
  const canStop = connected && pendingJobAction == null && (isPrinting || isPaused);
  const canToggleLight = connected && pendingLightToken == null;
  const pauseResumeLabel = pendingJobAction
    ? pendingJobAction === "pause"
      ? "Pausing..."
      : pendingJobAction === "resume"
        ? "Resuming..."
        : "Pause"
    : isPaused
      ? "Resume"
      : "Pause";
  const stopLabel = pendingJobAction === "stop" ? "Stopping..." : "Stop";
  const lightButtonLabel = pendingLightToken
    ? "Updating..."
    : lightIsOn
      ? "Light Off"
      : "Light On";
  const lastUpdate = status?.lastUpdate ? new Date(status.lastUpdate) : null;
  const staleSeconds =
    lastUpdate && !Number.isNaN(lastUpdate.getTime())
      ? Math.floor((Date.now() - lastUpdate.getTime()) / 1000)
      : null;
  const isStale = staleSeconds != null && staleSeconds > 15;

  useEffect(() => {
    handleJobUpdate(normalizedJobState);
  }, [normalizedJobState]);

  useEffect(() => {
    if (status?.light == null) {
      return;
    }
    clearPendingLight();
  }, [status?.light]);

  const sendCommand = async (payload) => {
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
      return true;
    } catch (err) {
      setError(err instanceof Error ? err.message : "command failed");
      return false;
    } finally {
      // no-op
    }
  };

  const schedulePendingJob = (action) => {
    setPendingJobAction(action);
    const token = Date.now() + Math.random();
    pendingJobTokenRef.current = token;
    if (pendingJobTimeoutRef.current) {
      clearTimeout(pendingJobTimeoutRef.current);
    }
    pendingJobTimeoutRef.current = setTimeout(() => {
      if (pendingJobTokenRef.current === token) {
        clearPendingJob();
      }
    }, JOB_TIMEOUT_MS);
  };

  const clearPendingJob = () => {
    setPendingJobAction(null);
    pendingJobTokenRef.current = null;
    if (pendingJobTimeoutRef.current) {
      clearTimeout(pendingJobTimeoutRef.current);
      pendingJobTimeoutRef.current = null;
    }
  };

  const handleJobUpdate = (jobStateValue) => {
    if (!pendingJobAction) {
      return;
    }
    switch (pendingJobAction) {
      case "pause":
        if (jobStateValue === "paused") {
          clearPendingJob();
        }
        break;
      case "resume":
        if (jobStateValue === "printing") {
          clearPendingJob();
        }
        break;
      case "stop":
        if (
          jobStateValue === "idle" ||
          jobStateValue === "finished" ||
          jobStateValue === "error"
        ) {
          clearPendingJob();
        }
        break;
      default:
        break;
    }
  };

  const scheduleLightTimeout = () => {
    const token = Date.now() + Math.random();
    pendingLightTokenRef.current = token;
    setPendingLightToken(token);
    if (pendingLightTimeoutRef.current) {
      clearTimeout(pendingLightTimeoutRef.current);
    }
    pendingLightTimeoutRef.current = setTimeout(() => {
      if (pendingLightTokenRef.current === token) {
        clearPendingLight();
      }
    }, LIGHT_TIMEOUT_MS);
  };

  const clearPendingLight = () => {
    pendingLightTokenRef.current = null;
    setPendingLightToken(null);
    setLightOverride(null);
    if (pendingLightTimeoutRef.current) {
      clearTimeout(pendingLightTimeoutRef.current);
      pendingLightTimeoutRef.current = null;
    }
  };

  const handlePauseResume = async () => {
    if (!canPauseResume) {
      return;
    }
    if (isPaused) {
      schedulePendingJob("resume");
      const ok = await sendCommand({ type: "resume" });
      if (!ok) {
        clearPendingJob();
      }
      return;
    }
    if (isPrinting) {
      schedulePendingJob("pause");
      const ok = await sendCommand({ type: "pause" });
      if (!ok) {
        clearPendingJob();
      }
    }
  };

  const handleStop = async () => {
    if (!canStop) {
      return;
    }
    schedulePendingJob("stop");
    const ok = await sendCommand({ type: "stop" });
    if (!ok) {
      clearPendingJob();
    }
  };

  const handleLightToggle = async () => {
    if (!canToggleLight) {
      return;
    }
    const nextValue = !lightIsOn;
    setLightOverride(nextValue);
    scheduleLightTimeout();
    const ok = await sendCommand({ type: "light", on: nextValue });
    if (!ok) {
      clearPendingLight();
    }
  };

  const jobStateDisplay = formatJobState(jobState, normalizedJobState);

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
            <strong>{jobStateDisplay}</strong>
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
              disabled={!canPauseResume}
              onClick={handlePauseResume}
            >
              {pauseResumeLabel}
            </button>
            <button
              type="button"
              className="danger"
              disabled={!canStop}
              onClick={handleStop}
            >
              {stopLabel}
            </button>
            <button
              type="button"
              aria-pressed={lightIsOn}
              disabled={!canToggleLight}
              onClick={handleLightToggle}
            >
              {lightButtonLabel}
            </button>
          </div>
          {error && <p className="error">{error}</p>}
        </div>
      </section>
    </div>
  );
}

function normalizeJobState(value) {
  if (!value) {
    return "unknown";
  }
  const text = String(value).toUpperCase();
  switch (text) {
    case "RUNNING":
    case "PRINTING":
      return "printing";
    case "PAUSE":
    case "PAUSED":
      return "paused";
    case "IDLE":
    case "STOPPED":
      return "idle";
    case "FINISH":
    case "FINISHED":
      return "finished";
    case "FAILED":
      return "error";
    default:
      return "unknown";
  }
}

function normalizeLight(value) {
  if (!value) {
    return null;
  }
  const text = String(value).toLowerCase();
  if (text === "on") {
    return true;
  }
  if (text === "off") {
    return false;
  }
  return null;
}

function formatJobState(rawValue, normalized) {
  switch (normalized) {
    case "printing":
      return "Printing";
    case "paused":
      return "Paused";
    case "idle":
      return "Idle";
    case "finished":
      return "Finished";
    case "error":
      return "Error";
    default:
      return rawValue ?? "UNKNOWN";
  }
}
