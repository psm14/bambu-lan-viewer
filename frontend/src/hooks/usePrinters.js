import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "selectedPrinterId";

export function usePrinters({ apiBase, onError }) {
  const [printers, setPrinters] = useState([]);
  const [selectedPrinterId, setSelectedPrinterId] = useState(() => {
    if (typeof window === "undefined") {
      return null;
    }
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (!stored) {
      return null;
    }
    const value = Number(stored);
    return Number.isNaN(value) ? null : value;
  });
  const [loadingPrinters, setLoadingPrinters] = useState(true);

  const loadPrinters = useCallback(async () => {
    setLoadingPrinters(true);
    try {
      const response = await fetch(`${apiBase}/api/printers`);
      if (!response.ok) {
        throw new Error("printer list fetch failed");
      }
      const data = await response.json();
      const list = Array.isArray(data) ? data : [];
      setPrinters(list);
      setSelectedPrinterId((currentId) => {
        const found = list.find((printer) => printer.id === currentId);
        if (found) {
          return currentId;
        }
        return list[0]?.id ?? null;
      });
    } catch (err) {
      onError?.("Unable to reach backend");
    } finally {
      setLoadingPrinters(false);
    }
  }, [apiBase, onError]);

  useEffect(() => {
    loadPrinters();
  }, [loadPrinters]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    if (selectedPrinterId == null) {
      window.localStorage.removeItem(STORAGE_KEY);
      return;
    }
    window.localStorage.setItem(STORAGE_KEY, String(selectedPrinterId));
  }, [selectedPrinterId]);

  return {
    printers,
    selectedPrinterId,
    setSelectedPrinterId,
    loadingPrinters,
    loadPrinters,
  };
}
