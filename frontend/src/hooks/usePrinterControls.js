import { useCallback, useEffect, useRef, useState } from "react";
import {
  formatHoursMinutes,
  formatJobState,
  formatLayer,
  normalizeJobState,
  normalizeLight,
} from "../utils/format";

const JOB_TIMEOUT_MS = 5000;
const LIGHT_TIMEOUT_MS = 3000;
const MOTION_HOME_TIMEOUT_MS = 3500;
const MOTION_JOG_TIMEOUT_MS = 700;
const TEMP_SET_TIMEOUT_MS = 2000;
const EXTRUDER_TIMEOUT_MS = 900;
const XY_JOG_STEP_MM = 5;
const Z_JOG_STEP_MM = 1;
const XY_FEED_RATE = 3000;
const Z_FEED_RATE = 600;
const EXTRUDER_STEP_MM = 5;
const EXTRUDER_FEED_RATE = 240;

export function usePrinterControls({ apiBase, selectedPrinterId, status, onError }) {
  const [pendingJobAction, setPendingJobAction] = useState(null);
  const [lightOverride, setLightOverride] = useState(null);
  const [pendingLightToken, setPendingLightToken] = useState(null);
  const [pendingMotionAction, setPendingMotionAction] = useState(null);
  const [pendingTempAction, setPendingTempAction] = useState(null);
  const [pendingExtruderAction, setPendingExtruderAction] = useState(null);

  const pendingJobTokenRef = useRef(null);
  const pendingJobTimeoutRef = useRef(null);
  const pendingLightTokenRef = useRef(null);
  const pendingLightTimeoutRef = useRef(null);
  const pendingMotionTokenRef = useRef(null);
  const pendingMotionTimeoutRef = useRef(null);
  const pendingTempTokenRef = useRef(null);
  const pendingTempTimeoutRef = useRef(null);
  const pendingExtruderTokenRef = useRef(null);
  const pendingExtruderTimeoutRef = useRef(null);

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
  const canHome = connected && pendingMotionAction == null;
  const canJog = connected && pendingMotionAction == null;
  const canSetTemperature = connected && pendingTempAction == null;
  const canExtrude =
    connected &&
    pendingExtruderAction == null &&
    pendingMotionAction == null;

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
  const homeLabel = pendingMotionAction === "home" ? "Homing..." : "Home";
  const nozzleTargetLabel =
    pendingTempAction === "nozzle" ? "Setting..." : "Set target";
  const bedTargetLabel =
    pendingTempAction === "bed" ? "Setting..." : "Set target";

  const progressPercent =
    status?.percent != null && !Number.isNaN(status.percent)
      ? Math.min(100, Math.max(0, Number(status.percent)))
      : null;
  const layerDisplay = formatLayer(status?.layerNum, status?.totalLayerNum);
  const remainingDisplay = formatHoursMinutes(status?.remainingMinutes);

  const jobStateDisplay = selectedPrinterId
    ? formatJobState(jobState, normalizedJobState)
    : "--";
  const statusLabel = selectedPrinterId
    ? connected
      ? "MQTT Connected"
      : "Offline"
    : "No Printer";

  const clearPendingJob = useCallback(() => {
    setPendingJobAction(null);
    pendingJobTokenRef.current = null;
    if (pendingJobTimeoutRef.current) {
      clearTimeout(pendingJobTimeoutRef.current);
      pendingJobTimeoutRef.current = null;
    }
  }, []);

  const schedulePendingJob = useCallback(
    (action) => {
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
    },
    [clearPendingJob],
  );

  const clearPendingLight = useCallback(() => {
    pendingLightTokenRef.current = null;
    setPendingLightToken(null);
    setLightOverride(null);
    if (pendingLightTimeoutRef.current) {
      clearTimeout(pendingLightTimeoutRef.current);
      pendingLightTimeoutRef.current = null;
    }
  }, []);

  const clearPendingMotion = useCallback(() => {
    setPendingMotionAction(null);
    pendingMotionTokenRef.current = null;
    if (pendingMotionTimeoutRef.current) {
      clearTimeout(pendingMotionTimeoutRef.current);
      pendingMotionTimeoutRef.current = null;
    }
  }, []);

  const clearPendingTemp = useCallback(() => {
    setPendingTempAction(null);
    pendingTempTokenRef.current = null;
    if (pendingTempTimeoutRef.current) {
      clearTimeout(pendingTempTimeoutRef.current);
      pendingTempTimeoutRef.current = null;
    }
  }, []);

  const clearPendingExtruder = useCallback(() => {
    setPendingExtruderAction(null);
    pendingExtruderTokenRef.current = null;
    if (pendingExtruderTimeoutRef.current) {
      clearTimeout(pendingExtruderTimeoutRef.current);
      pendingExtruderTimeoutRef.current = null;
    }
  }, []);

  const scheduleLightTimeout = useCallback(
    () => {
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
    },
    [clearPendingLight],
  );

  const schedulePendingMotion = useCallback(
    (action, timeoutMs) => {
      setPendingMotionAction(action);
      const token = Date.now() + Math.random();
      pendingMotionTokenRef.current = token;
      if (pendingMotionTimeoutRef.current) {
        clearTimeout(pendingMotionTimeoutRef.current);
      }
      pendingMotionTimeoutRef.current = setTimeout(() => {
        if (pendingMotionTokenRef.current === token) {
          clearPendingMotion();
        }
      }, timeoutMs);
    },
    [clearPendingMotion],
  );

  const schedulePendingTemp = useCallback(
    (action) => {
      setPendingTempAction(action);
      const token = Date.now() + Math.random();
      pendingTempTokenRef.current = token;
      if (pendingTempTimeoutRef.current) {
        clearTimeout(pendingTempTimeoutRef.current);
      }
      pendingTempTimeoutRef.current = setTimeout(() => {
        if (pendingTempTokenRef.current === token) {
          clearPendingTemp();
        }
      }, TEMP_SET_TIMEOUT_MS);
    },
    [clearPendingTemp],
  );

  const schedulePendingExtruder = useCallback(
    (action) => {
      setPendingExtruderAction(action);
      const token = Date.now() + Math.random();
      pendingExtruderTokenRef.current = token;
      if (pendingExtruderTimeoutRef.current) {
        clearTimeout(pendingExtruderTimeoutRef.current);
      }
      pendingExtruderTimeoutRef.current = setTimeout(() => {
        if (pendingExtruderTokenRef.current === token) {
          clearPendingExtruder();
        }
      }, EXTRUDER_TIMEOUT_MS);
    },
    [clearPendingExtruder],
  );

  const sendCommand = useCallback(
    async (payload) => {
      if (!selectedPrinterId) {
        onError?.("Select a printer first");
        return false;
      }
      try {
        const response = await fetch(
          `${apiBase}/api/printers/${selectedPrinterId}/command`,
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
        onError?.("");
        return true;
      } catch (err) {
        onError?.(err instanceof Error ? err.message : "command failed");
        return false;
      }
    },
    [apiBase, onError, selectedPrinterId],
  );

  const handlePauseResume = useCallback(async () => {
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
  }, [
    canPauseResume,
    clearPendingJob,
    isPaused,
    isPrinting,
    schedulePendingJob,
    sendCommand,
  ]);

  const handleStop = useCallback(async () => {
    if (!canStop) {
      return;
    }
    const confirmStop = window.confirm(
      "Stop the current print? This cannot be undone.",
    );
    if (!confirmStop) {
      return;
    }
    schedulePendingJob("stop");
    const ok = await sendCommand({ type: "stop" });
    if (!ok) {
      clearPendingJob();
    }
  }, [canStop, clearPendingJob, schedulePendingJob, sendCommand]);

  const handleLightToggle = useCallback(async () => {
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
  }, [
    canToggleLight,
    clearPendingLight,
    lightIsOn,
    scheduleLightTimeout,
    sendCommand,
  ]);

  const handleHome = useCallback(async () => {
    if (!canHome) {
      return;
    }
    schedulePendingMotion("home", MOTION_HOME_TIMEOUT_MS);
    const ok = await sendCommand({ type: "home" });
    if (!ok) {
      clearPendingMotion();
    }
  }, [canHome, clearPendingMotion, schedulePendingMotion, sendCommand]);

  const handleJog = useCallback(
    async (axis, direction) => {
      if (!canJog) {
        return;
      }
      const isZ = axis === "z";
      const step = isZ ? Z_JOG_STEP_MM : XY_JOG_STEP_MM;
      const feedRate = isZ ? Z_FEED_RATE : XY_FEED_RATE;
      const distance = direction === "positive" ? step : -step;
      schedulePendingMotion(
        `move-${axis}-${direction}`,
        MOTION_JOG_TIMEOUT_MS,
      );
      const ok = await sendCommand({
        type: "move",
        axis,
        distance,
        feed_rate: feedRate,
      });
      if (!ok) {
        clearPendingMotion();
      }
    },
    [canJog, clearPendingMotion, schedulePendingMotion, sendCommand],
  );

  const setTargetTemperature = useCallback(
    async (kind, target) => {
      if (!canSetTemperature) {
        return false;
      }
      const commandType =
        kind === "nozzle" ? "set_nozzle_temp" : "set_bed_temp";
      schedulePendingTemp(kind);
      const ok = await sendCommand({
        type: commandType,
        target_c: target,
      });
      if (!ok) {
        clearPendingTemp();
      }
      return ok;
    },
    [canSetTemperature, clearPendingTemp, schedulePendingTemp, sendCommand],
  );

  const handleSetNozzleTarget = useCallback(
    async (target) => setTargetTemperature("nozzle", target),
    [setTargetTemperature],
  );

  const handleSetBedTarget = useCallback(
    async (target) => setTargetTemperature("bed", target),
    [setTargetTemperature],
  );

  const handleExtrude = useCallback(
    async (direction) => {
      if (!canExtrude) {
        return;
      }
      const amount = direction === "forward" ? EXTRUDER_STEP_MM : -EXTRUDER_STEP_MM;
      schedulePendingExtruder(direction);
      const ok = await sendCommand({
        type: "extrude",
        amount_mm: amount,
        feed_rate: EXTRUDER_FEED_RATE,
      });
      if (!ok) {
        clearPendingExtruder();
      }
    },
    [canExtrude, clearPendingExtruder, schedulePendingExtruder, sendCommand],
  );

  useEffect(() => {
    if (!pendingJobAction) {
      return;
    }
    switch (pendingJobAction) {
      case "pause":
        if (normalizedJobState === "paused") {
          clearPendingJob();
        }
        break;
      case "resume":
        if (normalizedJobState === "printing") {
          clearPendingJob();
        }
        break;
      case "stop":
        if (
          normalizedJobState === "idle" ||
          normalizedJobState === "finished" ||
          normalizedJobState === "error"
        ) {
          clearPendingJob();
        }
        break;
      default:
        break;
    }
  }, [clearPendingJob, normalizedJobState, pendingJobAction]);

  useEffect(() => {
    if (status?.light == null) {
      return;
    }
    clearPendingLight();
  }, [clearPendingLight, status?.light]);

  useEffect(() => {
    clearPendingJob();
    clearPendingLight();
    clearPendingMotion();
    clearPendingTemp();
    clearPendingExtruder();
  }, [
    clearPendingExtruder,
    clearPendingJob,
    clearPendingLight,
    clearPendingMotion,
    clearPendingTemp,
    selectedPrinterId,
  ]);

  useEffect(() => {
    return () => {
      if (pendingJobTimeoutRef.current) {
        clearTimeout(pendingJobTimeoutRef.current);
      }
      if (pendingLightTimeoutRef.current) {
        clearTimeout(pendingLightTimeoutRef.current);
      }
      if (pendingMotionTimeoutRef.current) {
        clearTimeout(pendingMotionTimeoutRef.current);
      }
      if (pendingTempTimeoutRef.current) {
        clearTimeout(pendingTempTimeoutRef.current);
      }
      if (pendingExtruderTimeoutRef.current) {
        clearTimeout(pendingExtruderTimeoutRef.current);
      }
    };
  }, []);

  return {
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
    xyJogStepMm: XY_JOG_STEP_MM,
    zJogStepMm: Z_JOG_STEP_MM,
    extruderStepMm: EXTRUDER_STEP_MM,
  };
}
