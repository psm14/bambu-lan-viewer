export default function Header({
  printers,
  selectedPrinterId,
  loadingPrinters,
  selectedPrinter,
  onSelectPrinter,
  onOpenManager,
  userEmail,
  connected,
  statusLabel,
  jobStateDisplay,
}) {
  const handleSelectChange = (event) => {
    const value = event.target.value;
    onSelectPrinter(value ? Number(value) : null);
  };

  return (
    <header className="hero eva-bridge-header">
      <div className="eva-header-grid">
        <div className="eva-title-block">
          <div className="hero-badge-row eva-badge-row">
            <p className="hero-kicker mono">BAMBU LAN VIEWER // MAGI CONTROL</p>
            <span className={`eva-alert-pill mono ${connected ? "ok" : "warn"}`}>
              {connected ? "SYNC ONLINE" : "UPLINK LOST"}
            </span>
          </div>

          <div className="eva-title-row">
            <div className="eva-sigil" aria-hidden="true">
              <svg viewBox="0 0 180 180">
                <circle cx="90" cy="90" r="70" className="sigil-ring outer" />
                <circle cx="90" cy="90" r="48" className="sigil-ring inner" />
                <path d="M27 90h126M90 27v126" className="sigil-axis" />
                <path d="M90 42 125 90 90 138 55 90Z" className="sigil-diamond" />
                <circle cx="90" cy="90" r="10" className="sigil-core" />
                <circle cx="138" cy="90" r="4" className="sigil-node" />
                <circle cx="42" cy="90" r="4" className="sigil-node muted" />
              </svg>
            </div>

            <div className="hero-title-copy eva-title-copy">
              <div className="title-row eva-title-line">
                <h1 className="printer-title eva-printer-title">
                  {selectedPrinter?.name ?? "Select a printer"}
                </h1>
                <span className="eva-state-lock mono">{jobStateDisplay}</span>
              </div>

              <p className="hero-subtitle mono eva-subtitle">
                {selectedPrinter
                  ? `CASING NODE ${selectedPrinter.serial} // HOST ${selectedPrinter.host}`
                  : "Awaiting printer uplink"}
              </p>

              <div className="eva-inline-meta">
                <div className="hero-readout eva-readout slim">
                  <span className="mono">Session</span>
                  <strong>{userEmail ?? "Local operator"}</strong>
                </div>
                <div className="hero-readout eva-readout slim">
                  <span className="mono">Signal</span>
                  <strong>{statusLabel}</strong>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="eva-command-column">
          <details className="printer-picker eva-printer-picker">
            <summary className="printer-toggle eva-toggle" aria-label="Change printer">
              <span aria-hidden="true">NODE</span>
            </summary>
            <div className="picker-panel eva-picker-panel">
              <div className="picker-header">
                <span>Active Printer</span>
                <button type="button" onClick={onOpenManager}>
                  Manage
                </button>
              </div>
              <select
                value={selectedPrinterId ?? ""}
                onChange={handleSelectChange}
                disabled={!printers.length || loadingPrinters}
              >
                {!printers.length && <option value="">No printers yet</option>}
                {printers.map((printer) => (
                  <option key={printer.id} value={printer.id}>
                    {printer.name}
                  </option>
                ))}
              </select>
              {selectedPrinter && (
                <p className="printer-meta mono">
                  {selectedPrinter.host} • {selectedPrinter.serial}
                </p>
              )}
            </div>
          </details>

          <div className="eva-status-stack">
            <div className={`status-pill eva-status-pill ${connected ? "ok" : "warn"}`}>
              <span
                className={`status-indicator ${connected ? "ok" : "warn"}`}
                data-label={statusLabel}
                aria-label={statusLabel}
                role="button"
                tabIndex={0}
              />
              <span className="mono">{connected ? "LCL LINK STABLE" : "SYSTEM WARNING"}</span>
            </div>
            <button type="button" className="eva-manage-button" onClick={onOpenManager}>
              Open printer manager
            </button>
          </div>
        </div>
      </div>
    </header>
  );
}
