function formatHumidity(value) {
  if (value == null || Number.isNaN(Number(value))) {
    return "--";
  }
  const humidity = Math.max(0, Math.min(100, Math.round(Number(value))));
  return `${humidity}%`;
}

function normalizeColor(value) {
  if (typeof value !== "string") {
    return null;
  }
  const hex = value.trim().replace(/^#/, "").toUpperCase();
  if (!/^[0-9A-F]{6}([0-9A-F]{2})?$/.test(hex)) {
    return null;
  }
  return `#${hex.slice(0, 6)}`;
}

export default function AmsFilamentCard({ status }) {
  const amsUnits = Array.isArray(status?.ams) ? status.ams : [];

  if (!amsUnits.length) {
    return (
      <div className="card ams-card">
        <p className="helper">No AMS data reported yet.</p>
      </div>
    );
  }

  return (
    <>
      {amsUnits.map((unit, unitIndex) => {
        const trays = Array.isArray(unit?.trays) ? unit.trays : [];
        const amsLabel = `AMS ${unit?.id != null ? Number(unit.id) : unitIndex + 1}`;
        const traysBySlot = new Map();

        trays.forEach((tray, trayIndex) => {
          const rawId = Number(tray?.id);
          const slotId = Number.isInteger(rawId) ? rawId : trayIndex;
          if (slotId < 0 || slotId > 3 || traysBySlot.has(slotId)) {
            return;
          }
          traysBySlot.set(slotId, tray);
        });

        const orderedSlots = Array.from({ length: 4 }, (_, slotId) => {
          const tray = traysBySlot.get(slotId);
          const filamentType =
            typeof tray?.filamentType === "string" &&
            tray.filamentType.trim() !== ""
              ? tray.filamentType.trim()
              : "Empty";
          const rawColor = typeof tray?.color === "string" ? tray.color.trim() : "";
          const colorHex = normalizeColor(rawColor);

          return {
            slotId,
            filamentType,
            colorHex,
            colorLabel: (colorHex ?? rawColor) || "--",
          };
        });

        return (
          <div key={`${amsLabel}-${unitIndex}`} className="card ams-card">
            <div className="ams-header">
              <span className="ams-title">{amsLabel}</span>
              <span className="ams-humidity mono">
                Humidity {formatHumidity(unit?.humidityRaw)}
              </span>
            </div>

            <div className="ams-slots">
              {orderedSlots.map((slot) => {
                const isEmpty = slot.filamentType === "Empty";
                return (
                  <div
                    key={`${amsLabel}-slot-${slot.slotId}`}
                    className={`ams-tray ${isEmpty ? "empty" : ""}`}
                  >
                    <span className="ams-type">{slot.filamentType}</span>
                    <span
                      className="ams-color"
                      title={slot.colorLabel === "--" ? "Color unknown" : slot.colorLabel}
                      aria-label={
                        slot.colorLabel === "--"
                          ? "Color unknown"
                          : `Color ${slot.colorLabel}`
                      }
                    >
                      <span
                        className="ams-swatch"
                        aria-hidden="true"
                        style={slot.colorHex ? { backgroundColor: slot.colorHex } : undefined}
                      />
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        );
      })}
    </>
  );
}
