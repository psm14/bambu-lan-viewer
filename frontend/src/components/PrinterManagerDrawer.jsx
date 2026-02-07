import { useState } from "react";

const EMPTY_FORM = {
  id: null,
  name: "",
  host: "",
  serial: "",
  accessCode: "",
  rtspUrl: "",
};

export default function PrinterManagerDrawer({
  apiBase,
  printers,
  loadingPrinters,
  selectedPrinterId,
  setSelectedPrinterId,
  loadPrinters,
  onClose,
  onError,
}) {
  const [formState, setFormState] = useState(EMPTY_FORM);
  const [formError, setFormError] = useState("");
  const [savingPrinter, setSavingPrinter] = useState(false);

  const closeManager = () => {
    onClose();
    setFormState(EMPTY_FORM);
    setFormError("");
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
        ? `${apiBase}/api/printers/${formState.id}`
        : `${apiBase}/api/printers`;
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
      setFormError(
        err instanceof Error ? err.message : "Unable to save printer",
      );
    } finally {
      setSavingPrinter(false);
    }
  };

  const handleDeletePrinter = async (printerId) => {
    const confirmDelete = window.confirm("Delete this printer configuration?");
    if (!confirmDelete) {
      return;
    }
    try {
      const response = await fetch(`${apiBase}/api/printers/${printerId}`, {
        method: "DELETE",
      });
      if (!response.ok && response.status !== 204) {
        throw new Error("Unable to delete printer");
      }
      await loadPrinters();
    } catch (err) {
      onError?.(err instanceof Error ? err.message : "Unable to delete printer");
    }
  };

  return (
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
                  setFormState({
                    ...formState,
                    rtspUrl: event.target.value,
                  })
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
              <button type="button" onClick={() => setFormState(EMPTY_FORM)}>
                Reset
              </button>
            </div>
          </form>
        </div>
      </aside>
    </div>
  );
}
