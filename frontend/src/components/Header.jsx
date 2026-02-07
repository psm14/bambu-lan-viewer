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
}) {
  const handleSelectChange = (event) => {
    const value = event.target.value;
    onSelectPrinter(value ? Number(value) : null);
  };

  return (
    <header className="hero">
      <div className="title-row">
        <h1 className="printer-title">
          {selectedPrinter?.name ?? "Select a printer"}
        </h1>
        <details className="printer-picker">
          <summary className="printer-toggle" aria-label="Change printer">
            <span aria-hidden="true">▾</span>
          </summary>
          <div className="picker-panel">
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
              <p className="printer-meta">
                {selectedPrinter.host} • {selectedPrinter.serial}
              </p>
            )}
          </div>
        </details>
      </div>
      <div className="hero-side">
        <div className="user-row">
          {userEmail && <div className="user-email">{userEmail}</div>}
          <span
            className={`status-indicator ${connected ? "ok" : "warn"}`}
            data-label={statusLabel}
            aria-label={statusLabel}
            role="button"
            tabIndex={0}
          />
        </div>
      </div>
    </header>
  );
}
