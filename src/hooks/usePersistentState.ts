import { useCallback, useEffect, useState } from "react";

type PersistentStateMerge<T> = (current: T, incoming: T) => T;

export function usePersistentState<T>(key: string, initialValue: T, merge?: PersistentStateMerge<T>) {
  const [value, setValue] = useState<T>(() => {
    try {
      const saved = localStorage.getItem(key);
      return saved ? (JSON.parse(saved) as T) : initialValue;
    } catch {
      return initialValue;
    }
  });

  useEffect(() => {
    const syncFromAnotherWindow = (event: StorageEvent) => {
      if (event.key !== key || event.newValue === null) return;
      try {
        const incoming = JSON.parse(event.newValue) as T;
        setValue((current) => {
          const resolved = merge ? merge(current, incoming) : incoming;
          if (merge) {
            const serialized = JSON.stringify(resolved);
            if (serialized !== event.newValue) localStorage.setItem(key, serialized);
          }
          return resolved;
        });
      } catch {
        // Ignore malformed values and keep the last valid local state.
      }
    };
    window.addEventListener("storage", syncFromAnotherWindow);
    return () => window.removeEventListener("storage", syncFromAnotherWindow);
  }, [key, merge]);

  const updateValue = useCallback((next: T | ((current: T) => T)) => {
    setValue((current) => {
      let base = current;
      if (merge) {
        try {
          const saved = localStorage.getItem(key);
          if (saved) base = merge(current, JSON.parse(saved) as T);
        } catch {
          // Keep the in-memory value when the stored value is malformed or unavailable.
        }
      }
      const candidate = typeof next === "function" ? (next as (current: T) => T)(base) : next;
      const resolved = merge ? merge(base, candidate) : candidate;
      localStorage.setItem(key, JSON.stringify(resolved));
      return resolved;
    });
  }, [key, merge]);

  return [value, updateValue] as const;
}
