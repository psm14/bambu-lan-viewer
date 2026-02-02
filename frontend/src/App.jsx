import { useEffect, useRef, useState } from "react";
import Hls from "hls.js";
import "./App.css";

const API_BASE = import.meta.env.VITE_API_BASE ?? "";
const POLL_MS = 3000;

const JOB_TIMEOUT_MS = 5000;
const LIGHT_TIMEOUT_MS = 3000;
const LOW_LATENCY_DEFAULT =
  String(import.meta.env.VITE_HLS_LOW_LATENCY ?? "true").toLowerCase() ===
  "true";

const EMPTY_FORM = {
  id: null,
  name: "",
  host: "",
  serial: "",
  accessCode: "",
  rtspUrl: "",
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
  const [printers, setPrinters] = useState([]);
  const [selectedPrinterId, setSelectedPrinterId] = useState(() => {
    if (typeof window === "undefined") {
      return null;
    }
    const stored = window.localStorage.getItem("selectedPrinterId");
    if (!stored) {
      return null;
    }
    const value = Number(stored);
    return Number.isNaN(value) ? null : value;
  });
  const [status, setStatus] = useState(null);
  const [error, setError] = useState("");
  const [videoError, setVideoError] = useState("");
  const [pendingJobAction, setPendingJobAction] = useState(null);
  const [lightOverride, setLightOverride] = useState(null);
  const [pendingLightToken, setPendingLightToken] = useState(null);
  const [videoReload, setVideoReload] = useState(0);
  const [useLowLatency, setUseLowLatency] = useState(LOW_LATENCY_DEFAULT);
  const [showManager, setShowManager] = useState(false);
  const [formState, setFormState] = useState(EMPTY_FORM);
  const [formError, setFormError] = useState("");
  const [savingPrinter, setSavingPrinter] = useState(false);
  const [loadingPrinters, setLoadingPrinters] = useState(true);

  const videoRef = useRef(null);
  const pendingJobTokenRef = useRef(null);
  const pendingJobTimeoutRef = useRef(null);
  const pendingLightTokenRef = useRef(null);
  const pendingLightTimeoutRef = useRef(null);

  const selectedPrinter =
    printers.find((printer) => printer.id === selectedPrinterId) ?? null;
  const baseHlsUrl = selectedPrinterId
    ? `${API_BASE}/hls/${selectedPrinterId}/stream.m3u8`
    : "";
  const llHlsUrl = selectedPrinterId
    ? `${API_BASE}/hls/${selectedPrinterId}/stream_ll.m3u8`
    : "";
  const hlsUrl = useLowLatency ? llHlsUrl : baseHlsUrl;

  const loadPrinters = async () => {
    setLoadingPrinters(true);
    try {
      const response = await fetch(`${API_BASE}/api/printers`);
      if (!response.ok) {
        throw new Error("printer list fetch failed");
      }
      const data = await response.json();
      setPrinters(Array.isArray(data) ? data : []);
      const found = data?.find?.((printer) => printer.id === selectedPrinterId);
      if (!found) {
        const fallback = data?.[0]?.id ?? null;
        setSelectedPrinterId(fallback);
      }
    } catch (err) {
      setError("Unable to reach backend");
    } finally {
      setLoadingPrinters(false);
    }
  };

  useEffect(() => {
    loadPrinters();
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    if (selectedPrinterId == null) {
      window.localStorage.removeItem("selectedPrinterId");
      return;
    }
    window.localStorage.setItem("selectedPrinterId", String(selectedPrinterId));
  }, [selectedPrinterId]);

  useEffect(() => {
    let isActive = true;
    let eventSource = null;
    let pollTimer = null;

    if (!selectedPrinterId) {
      setStatus(null);
      setError("");
      return () => {};
    }

    const statusUrl = `${API_BASE}/api/printers/${selectedPrinterId}/status`;
    const streamUrl = `${API_BASE}/api/printers/${selectedPrinterId}/status/stream`;

    const fetchStatus = async () => {
      try {
        const response = await fetch(statusUrl);
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

    const handleStatus = (data) => {
      if (isActive) {
        setStatus(data);
        setError("");
      }
    };

    if (typeof EventSource === "undefined") {
      fetchStatus();
      pollTimer = setInterval(fetchStatus, POLL_MS);
      return () => {
        isActive = false;
        if (pollTimer) {
          clearInterval(pollTimer);
        }
      };
    }

    eventSource = new EventSource(streamUrl);
    eventSource.addEventListener("status", (event) => {
      try {
        const data = JSON.parse(event.data);
        handleStatus(data);
      } catch (err) {
        // Ignore malformed events.
      }
    });
    eventSource.onerror = () => {
      if (isActive) {
        setError("Unable to reach backend");
      }
    };

    return () => {
      isActive = false;
      if (eventSource) {
        eventSource.close();
      }
      if (pollTimer) {
        clearInterval(pollTimer);
      }
    };
  }, [selectedPrinterId]);

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

    if (!hlsUrl) {
      setVideoError("Select a printer to load video");
      video.removeAttribute("src");
      video.load();
      return () => {};
    }

    const onVideoError = () => {
      if (useLowLatency) {
        setUseLowLatency(false);
        setVideoReload((value) => value + 1);
        return;
      }
      setVideoError("Video element error");
    };
    video.addEventListener("error", onVideoError);

    const isSafari =
      typeof navigator !== "undefined" && /Apple/.test(navigator.vendor);

    if (Hls.isSupported() && !isSafari) {
      const hls = new Hls({
        enableWorker: true,
        backBufferLength: 0,
        lowLatencyMode: useLowLatency,
        liveSyncDurationCount: useLowLatency ? 1 : 3,
        liveMaxLatencyDurationCount: useLowLatency ? 3 : 6,
        maxLiveSyncPlaybackRate: useLowLatency ? 1.5 : 1.0,
      });
      hls.on(Hls.Events.MANIFEST_PARSED, () => {
        video.play().catch(() => {});
      });
      hls.on(Hls.Events.ERROR, (_, data) => {
        const message = `HLS error: ${data.type} ${data.details} fatal=${data.fatal}`;
        console.error(message, data);
        if (data.fatal) {
          if (
            useLowLatency &&
            (data.details === Hls.ErrorDetails.MANIFEST_LOAD_ERROR ||
              data.details === Hls.ErrorDetails.MANIFEST_PARSING_ERROR ||
              data.details === Hls.ErrorDetails.LEVEL_LOAD_ERROR)
          ) {
            hls.destroy();
            setUseLowLatency(false);
            setVideoReload((value) => value + 1);
            return;
          }
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

    if (video.canPlayType("application/vnd.apple.mpegurl")) {
      video.src = hlsUrl;
      return () => {
        video.removeEventListener("error", onVideoError);
        video.removeAttribute("src");
        video.load();
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
  const canStop =
    connected && pendingJobAction == null && (isPrinting || isPaused);
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

  useEffect(() => {
    clearPendingJob();
    clearPendingLight();
    setVideoReload((value) => value + 1);
  }, [selectedPrinterId]);

  const sendCommand = async (payload) => {
    if (!selectedPrinterId) {
      setError("Select a printer first");
      return false;
    }
    try {
      const response = await fetch(
        `${API_BASE}/api/printers/${selectedPrinterId}/command`,
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(payload),
        },
      );
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

  const handleSavePrinter = async (event) => {
    event.preventDefault();
    setFormError("");

    const name = formState.name.trim();
    const host = formState.host.trim();
    const serial = formState.serial.trim();
    const accessCode = formState.accessCode.trim();
    const rtspUrl = formState.rtspUrl.trim();

    if (!name || !host || !serial) {
      setFormError("Name, host, and serial are required");
      return;
    }

    if (!formState.id && !accessCode) {
      setFormError("Access code is required");
      return;
    }

    const payload = {
      name,
      host,
      serial,
      rtspUrl,
    };

    if (accessCode) {
      payload.accessCode = accessCode;
    }

    setSavingPrinter(true);
    try {
      const endpoint = formState.id
        ? `${API_BASE}/api/printers/${formState.id}`
        : `${API_BASE}/api/printers`;
      const method = formState.id ? "PUT" : "POST";
      const response = await fetch(endpoint, {
        method,
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      const data = await response.json().catch(() => ({}));
      if (!response.ok) {
        throw new Error(data.error || "Unable to save printer");
      }
      await loadPrinters();
      if (!formState.id && data?.id) {
        setSelectedPrinterId(data.id);
      }
      setFormState(EMPTY_FORM);
      setFormError("");
    } catch (err) {
      setFormError(err instanceof Error ? err.message : "Unable to save printer");
    } finally {
      setSavingPrinter(false);
    }
  };

  const beginEditPrinter = (printer) => {
    setFormState({
      id: printer.id,
      name: printer.name ?? "",
      host: printer.host ?? "",
      serial: printer.serial ?? "",
      accessCode: "",
      rtspUrl: printer.rtspUrl ?? "",
    });
    setFormError("");
    setShowManager(true);
  };

  const handleDeletePrinter = async (printerId) => {
    const confirmDelete = window.confirm(
      "Delete this printer configuration?",
    );
    if (!confirmDelete) {
      return;
    }
    try {
      const response = await fetch(`${API_BASE}/api/printers/${printerId}`, {
        method: "DELETE",
      });
      if (!response.ok && response.status !== 204) {
        throw new Error("Unable to delete printer");
      }
      await loadPrinters();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to delete printer");
    }
  };

  const closeManager = () => {
    setShowManager(false);
    setFormState(EMPTY_FORM);
    setFormError("");
  };

  const jobStateDisplay = selectedPrinterId
    ? formatJobState(jobState, normalizedJobState)
    : "--";
  const statusLabel = selectedPrinterId
    ? connected
      ? "MQTT Connected"
      : "Offline"
    : "No Printer";

  return (
    <div className="app">
      <header className="hero">
        <div>
          <p className="eyebrow">Bambu LAN Viewer</p>
          <h1>Printer Status + Controls</h1>
          <p className="subhead">
            MQTT status streamed over SSE with direct control commands. Video
            streaming is live via HLS.
          </p>
        </div>
        <div className="hero-side">
          <div className="printer-picker">
            <div className="picker-header">
              <span>Active Printer</span>
              <button type="button" onClick={() => setShowManager(true)}>
                Manage
              </button>
            </div>
            <select
              value={selectedPrinterId ?? ""}
              onChange={(event) =>
                setSelectedPrinterId(
                  event.target.value ? Number(event.target.value) : null,
                )
              }
              disabled={!printers.length || loadingPrinters}
            >
              {!printers.length && (
                <option value="">No printers yet</option>
              )}
              {printers.map((printer) => (
                <option key={printer.id} value={printer.id}>
                  {printer.name}
                </option>
              ))}
            </select>
            {selectedPrinter && (
              <p className="printer-meta">
                {selectedPrinter.host} • {selectedPrinter.serial}
              </p>
            )}
          </div>
          <div className={`pill ${connected ? "ok" : "warn"}`}>
            {statusLabel}
          </div>
        </div>
      </header>

      <section className="grid">
        <div className="card video-card">
          <div className="video-header">
            <h2>Camera</h2>
            <button
              type="button"
              onClick={() => setVideoReload((value) => value + 1)}
              disabled={!selectedPrinterId}
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
          <p className="helper">
            {selectedPrinter
              ? `Streaming ${selectedPrinter.name} via RTSPS → HLS.`
              : "Add a printer to start streaming."}
          </p>
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
          {!selectedPrinterId && (
            <p className="helper">Select a printer to view status.</p>
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

      {showManager && (
        <div className="drawer-backdrop" onClick={closeManager}>
          <aside className="drawer" onClick={(event) => event.stopPropagation()}>
            <div className="drawer-header">
              <div>
                <h2>Manage Printers</h2>
                <p className="helper">
                  Add or edit printer configs stored in the backend database.
                </p>
              </div>
              <button type="button" onClick={closeManager}>
                Close
              </button>
            </div>

            <div className="drawer-section">
              <h3>Configured Printers</h3>
              {loadingPrinters && <p className="helper">Loading printers…</p>}
              {!loadingPrinters && printers.length === 0 && (
                <p className="helper">No printers added yet.</p>
              )}
              {printers.map((printer) => (
                <div
                  key={printer.id}
                  className={`printer-row ${
                    printer.id === selectedPrinterId ? "active" : ""
                  }`}
                >
                  <div>
                    <strong>{printer.name}</strong>
                    <p className="printer-meta">
                      {printer.host} • {printer.serial}
                    </p>
                  </div>
                  <div className="row-actions">
                    <button
                      type="button"
                      onClick={() => setSelectedPrinterId(printer.id)}
                    >
                      Use
                    </button>
                    <button type="button" onClick={() => beginEditPrinter(printer)}>
                      Edit
                    </button>
                    <button
                      type="button"
                      className="danger"
                      onClick={() => handleDeletePrinter(printer.id)}
                    >
                      Delete
                    </button>
                  </div>
                </div>
              ))}
            </div>

            <div className="drawer-section">
              <h3>{formState.id ? "Edit Printer" : "Add Printer"}</h3>
              <form className="printer-form" onSubmit={handleSavePrinter}>
                <label>
                  Name
                  <input
                    value={formState.name}
                    onChange={(event) =>
                      setFormState({ ...formState, name: event.target.value })
                    }
                    placeholder="Studio X1"
                    required
                  />
                </label>
                <label>
                  Host / IP
                  <input
                    value={formState.host}
                    onChange={(event) =>
                      setFormState({ ...formState, host: event.target.value })
                    }
                    placeholder="192.168.1.10"
                    required
                  />
                </label>
                <label>
                  Serial
                  <input
                    value={formState.serial}
                    onChange={(event) =>
                      setFormState({ ...formState, serial: event.target.value })
                    }
                    placeholder="00M1234ABC"
                    required
                  />
                </label>
                <label>
                  Access Code
                  <input
                    type="password"
                    value={formState.accessCode}
                    onChange={(event) =>
                      setFormState({
                        ...formState,
                        accessCode: event.target.value,
                      })
                    }
                    placeholder={formState.id ? "Leave blank to keep" : "Required"}
                  />
                </label>
                <label>
                  RTSP URL (optional)
                  <input
                    value={formState.rtspUrl}
                    onChange={(event) =>
                      setFormState({ ...formState, rtspUrl: event.target.value })
                    }
                    placeholder="rtsps://..."
                  />
                </label>
                {formError && <p className="error">{formError}</p>}
                <div className="form-actions">
                  <button type="submit" disabled={savingPrinter}>
                    {savingPrinter
                      ? "Saving..."
                      : formState.id
                        ? "Update Printer"
                        : "Add Printer"}
                  </button>
                  <button
                    type="button"
                    onClick={() => setFormState(EMPTY_FORM)}
                  >
                    Reset
                  </button>
                </div>
              </form>
            </div>
          </aside>
        </div>
      )}
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
