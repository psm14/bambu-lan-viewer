import { useEffect, useState } from "react";
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
  const [busyCommand, setBusyCommand] = useState("");

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
