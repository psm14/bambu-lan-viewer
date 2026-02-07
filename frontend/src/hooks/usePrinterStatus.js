import { useEffect, useState } from "react";

const DEFAULT_POLL_MS = 3000;

export function usePrinterStatus({
  apiBase,
  selectedPrinterId,
  pollMs = DEFAULT_POLL_MS,
  onError,
}) {
  const [status, setStatus] = useState(null);

  useEffect(() => {
    let isActive = true;
    let eventSource = null;
    let pollTimer = null;

    if (!selectedPrinterId) {
      setStatus(null);
      onError?.("");
      return () => {};
    }

    const statusUrl = `${apiBase}/api/printers/${selectedPrinterId}/status`;
    const streamUrl = `${apiBase}/api/printers/${selectedPrinterId}/status/stream`;

    const fetchStatus = async () => {
      try {
        const response = await fetch(statusUrl);
        if (!response.ok) {
          throw new Error("status fetch failed");
        }
        const data = await response.json();
        if (isActive) {
          setStatus(data);
          onError?.("");
        }
      } catch (err) {
        if (isActive) {
          onError?.("Unable to reach backend");
        }
      }
    };

    const handleStatus = (data) => {
      if (isActive) {
        setStatus(data);
        onError?.("");
      }
    };

    if (typeof EventSource === "undefined") {
      fetchStatus();
      pollTimer = setInterval(fetchStatus, pollMs);
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
        onError?.("Unable to reach backend");
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
  }, [apiBase, onError, pollMs, selectedPrinterId]);

  return status;
}
