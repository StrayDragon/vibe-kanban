import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { Operation } from 'rfc6902';
import { createEventSource } from '@/lib/api';
import {
  invalidateQueriesFromHints,
  invalidateQueriesFromJsonPatch,
  type InvalidationHints,
} from '@/contexts/eventStreamInvalidation';

type EventStreamContextType = {
  isConnected: boolean;
  error: string | null;
};

const EventStreamContext = createContext<EventStreamContextType | null>(null);

export function EventStreamProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();
  const [isConnected, setIsConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const source = createEventSource('/api/events');
    const hintedEventIds = new Set<string>();

    source.onopen = () => {
      setIsConnected(true);
      setError(null);
    };

    source.onerror = () => {
      setIsConnected(false);
      setError('Event stream disconnected');
    };

    const handleJsonPatch = (event: MessageEvent<string>) => {
      if (event.lastEventId && hintedEventIds.has(event.lastEventId)) {
        // Prefer backend hints when available for the same sequenced update.
        hintedEventIds.delete(event.lastEventId);
        return;
      }

      let patch: Operation[];
      try {
        patch = JSON.parse(event.data) as Operation[];
      } catch (err) {
        console.warn('Failed to parse SSE json_patch event', err);
        setError('Failed to parse event stream update');
        return;
      }

      invalidateQueriesFromJsonPatch(queryClient, patch);
    };

    const handleInvalidate = (event: MessageEvent<string>) => {
      if (event.lastEventId) {
        hintedEventIds.add(event.lastEventId);
        // Prevent unbounded growth; keep only recent ids.
        if (hintedEventIds.size > 256) {
          const first = hintedEventIds.values().next().value;
          if (first) hintedEventIds.delete(first);
        }
      }

      let hints: InvalidationHints;
      try {
        hints = JSON.parse(event.data) as InvalidationHints;
      } catch (err) {
        console.warn('Failed to parse SSE invalidate event', err);
        setError('Failed to parse event stream update');
        return;
      }

      invalidateQueriesFromHints(queryClient, hints);
    };

    source.addEventListener('json_patch', handleJsonPatch);
    source.addEventListener('invalidate', handleInvalidate);

    return () => {
      source.removeEventListener('json_patch', handleJsonPatch);
      source.removeEventListener('invalidate', handleInvalidate);
      source.close();
    };
  }, [queryClient]);

  const value = useMemo(
    () => ({
      isConnected,
      error,
    }),
    [isConnected, error]
  );

  return (
    <EventStreamContext.Provider value={value}>
      {children}
    </EventStreamContext.Provider>
  );
}

export function useEventStream() {
  const ctx = useContext(EventStreamContext);
  if (!ctx) {
    throw new Error('useEventStream must be used within EventStreamProvider');
  }
  return ctx;
}
