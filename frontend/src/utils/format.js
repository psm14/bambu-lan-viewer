export function formatTemp(value) {
  if (value == null || Number.isNaN(value)) {
    return "--";
  }
  return `${value.toFixed(1)} \u00b0C`;
}

export function formatPercent(value) {
  if (value == null || Number.isNaN(value)) {
    return "--";
  }
  return `${value}%`;
}

export function formatHoursMinutes(totalMinutes) {
  if (totalMinutes == null || Number.isNaN(totalMinutes)) {
    return "--:--";
  }
  const minutes = Math.max(0, Math.floor(totalMinutes));
  const hours = Math.floor(minutes / 60);
  const remainder = minutes % 60;
  return `${hours}:${String(remainder).padStart(2, "0")}`;
}

export function formatLayer(current, total) {
  if (
    current == null ||
    total == null ||
    Number.isNaN(current) ||
    Number.isNaN(total)
  ) {
    return "-- / --";
  }
  return `${current} / ${total}`;
}

export function normalizeJobState(value) {
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

export function normalizeLight(value) {
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

export function formatJobState(rawValue, normalized) {
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
