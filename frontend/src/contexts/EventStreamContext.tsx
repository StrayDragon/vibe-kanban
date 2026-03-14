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
import { invalidateQueriesFromJsonPatch } from '@/contexts/eventStreamInvalidation';

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

    source.onopen = () => {
      setIsConnected(true);
      setError(null);
    };

    source.onerror = () => {
      setIsConnected(false);
      setError('Event stream disconnected');
    };

    const handleJsonPatch = (event: MessageEvent<string>) => {
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

    source.addEventListener('json_patch', handleJsonPatch);

    return () => {
      source.removeEventListener('json_patch', handleJsonPatch);
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
