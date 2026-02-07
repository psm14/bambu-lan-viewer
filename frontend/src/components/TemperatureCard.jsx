import { useEffect, useRef, useState } from "react";
import { formatTemp } from "../utils/format";

const NOZZLE_MIN_C = 0;
const NOZZLE_MAX_C = 320;
const BED_MIN_C = 0;
const BED_MAX_C = 120;

function formatTargetText(value) {
  if (value == null || Number.isNaN(value)) {
    return "Target --";
  }
  if (value <= 0) {
    return "Target Off";
  }
  return `Target ${formatTemp(value)}`;
}

function initialTargetInput(target, fallbackCurrent) {
  if (target != null && !Number.isNaN(target) && target > 0) {
    return String(Math.round(target));
  }
  if (fallbackCurrent != null && !Number.isNaN(fallbackCurrent) && fallbackCurrent > 0) {
    return String(Math.round(fallbackCurrent));
  }
  return "";
}

function parseTargetInput(raw, min, max) {
  const value = Number(raw);
  if (!Number.isFinite(value)) {
    return null;
  }
  if (value < min || value > max) {
    return null;
  }
  return Math.round(value);
}

export default function TemperatureCard({
  status,
  selectedPrinterId,
  canSetTemperature,
  pendingTempAction,
  nozzleTargetLabel,
  bedTargetLabel,
  handleSetNozzleTarget,
  handleSetBedTarget,
}) {
  const [editingTarget, setEditingTarget] = useState(null);
  const [draftTarget, setDraftTarget] = useState("");
  const [editError, setEditError] = useState("");
  const inputRef = useRef(null);

  useEffect(() => {
    if (!editingTarget) {
      return;
    }
    inputRef.current?.focus();
    inputRef.current?.select();
  }, [editingTarget]);

  useEffect(() => {
    setEditingTarget(null);
    setDraftTarget("");
    setEditError("");
  }, [selectedPrinterId]);

  const startEdit = (kind) => {
    if (!selectedPrinterId || !canSetTemperature) {
      return;
    }
    const target =
      kind === "nozzle" ? status?.nozzleTargetC : status?.bedTargetC;
    const current = kind === "nozzle" ? status?.nozzleC : status?.bedC;
    setEditingTarget(kind);
    setDraftTarget(initialTargetInput(target, current));
    setEditError("");
  };

  const cancelEdit = () => {
    setEditingTarget(null);
    setDraftTarget("");
    setEditError("");
  };

  const submitEdit = async (kind) => {
    const isNozzle = kind === "nozzle";
    const min = isNozzle ? NOZZLE_MIN_C : BED_MIN_C;
    const max = isNozzle ? NOZZLE_MAX_C : BED_MAX_C;
    const parsedTarget = parseTargetInput(draftTarget, min, max);
    if (parsedTarget == null) {
      setEditError(`Enter a target between ${min} and ${max}°C`);
      return;
    }
    const setter = isNozzle ? handleSetNozzleTarget : handleSetBedTarget;
    const ok = await setter?.(parsedTarget);
    if (ok) {
      cancelEdit();
    }
  };

  const nozzleEditing = editingTarget === "nozzle";
  const bedEditing = editingTarget === "bed";
  const pendingNozzle = pendingTempAction === "nozzle";
  const pendingBed = pendingTempAction === "bed";

  return (
    <div className="card temperature-card">
      <div className="temp-row">
        <div className="temp-item" role="group" aria-label="Nozzle temperature">
          <svg className="temp-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M9 3h6v6h-2v3.5l3.5 4.5H7.5L11 12.5V9H9V3Z"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinejoin="round"
            />
            <path
              d="M8 17h8M7 20h10"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinecap="round"
            />
          </svg>
          <span className="temp-value mono">
            {formatTemp(status?.nozzleC)}
          </span>
          {nozzleEditing ? (
            <form
              className="temp-target-editor"
              onSubmit={(event) => {
                event.preventDefault();
                submitEdit("nozzle");
              }}
            >
              <input
                ref={inputRef}
                type="number"
                inputMode="decimal"
                enterKeyHint="done"
                min={NOZZLE_MIN_C}
                max={NOZZLE_MAX_C}
                step="1"
                value={draftTarget}
                onChange={(event) => {
                  setDraftTarget(event.target.value);
                  setEditError("");
                }}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    cancelEdit();
                  }
                }}
                aria-label="Nozzle target temperature in Celsius"
                disabled={pendingNozzle}
              />
              <div className="temp-target-actions">
                <button
                  type="submit"
                  disabled={pendingNozzle}
                >
                  {pendingNozzle ? "Setting..." : "Set"}
                </button>
                <button
                  type="button"
                  onClick={cancelEdit}
                  disabled={pendingNozzle}
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : (
            <button
              type="button"
              className="temp-target-button"
              onClick={() => startEdit("nozzle")}
              disabled={!selectedPrinterId || !canSetTemperature}
              title="Set nozzle target temperature"
            >
              <span>{formatTargetText(status?.nozzleTargetC)}</span>
              <span className="temp-target-action-label">{nozzleTargetLabel}</span>
            </button>
          )}
        </div>
        <div className="temp-item" role="group" aria-label="Bed temperature">
          <svg className="temp-icon" viewBox="0 0 24 24" aria-hidden="true">
            <rect
              x="4"
              y="6"
              width="16"
              height="6"
              rx="2"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
            />
            <path
              d="M6 16h12M8 20h8"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinecap="round"
            />
          </svg>
          <span className="temp-value mono">{formatTemp(status?.bedC)}</span>
          {bedEditing ? (
            <form
              className="temp-target-editor"
              onSubmit={(event) => {
                event.preventDefault();
                submitEdit("bed");
              }}
            >
              <input
                ref={inputRef}
                type="number"
                inputMode="decimal"
                enterKeyHint="done"
                min={BED_MIN_C}
                max={BED_MAX_C}
                step="1"
                value={draftTarget}
                onChange={(event) => {
                  setDraftTarget(event.target.value);
                  setEditError("");
                }}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    cancelEdit();
                  }
                }}
                aria-label="Bed target temperature in Celsius"
                disabled={pendingBed}
              />
              <div className="temp-target-actions">
                <button type="submit" disabled={pendingBed}>
                  {pendingBed ? "Setting..." : "Set"}
                </button>
                <button
                  type="button"
                  onClick={cancelEdit}
                  disabled={pendingBed}
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : (
            <button
              type="button"
              className="temp-target-button"
              onClick={() => startEdit("bed")}
              disabled={!selectedPrinterId || !canSetTemperature}
              title="Set bed target temperature"
            >
              <span>{formatTargetText(status?.bedTargetC)}</span>
              <span className="temp-target-action-label">{bedTargetLabel}</span>
            </button>
          )}
        </div>
        <div
          className="temp-item"
          role="group"
          aria-label="Chamber temperature"
        >
          <svg className="temp-icon" viewBox="0 0 24 24" aria-hidden="true">
            <rect
              x="5"
              y="4"
              width="14"
              height="16"
              rx="2"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
            />
            <path
              d="M9 8h6M8 12h8M9 16h6"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.4"
              strokeLinecap="round"
            />
          </svg>
          <span className="temp-value mono">
            {formatTemp(status?.chamberC)}
          </span>
          <span className="temp-target-readonly">Ambient only</span>
        </div>
      </div>
      {editError && <p className="error">{editError}</p>}
    </div>
  );
}
