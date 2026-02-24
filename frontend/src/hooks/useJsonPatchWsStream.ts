import { useEffect, useState, useRef, useCallback } from 'react';
import { applyPatch } from 'rfc6902';
import type { Operation } from 'rfc6902';
import { withApiTokenQuery } from '@/api/token';

type WsJsonPatchMsg = { JsonPatch: Operation[] };
type WsFinishedMsg = { finished: boolean };
type WsMsg = WsJsonPatchMsg | WsFinishedMsg;

interface UseJsonPatchStreamOptions<T> {
  /**
   * Called once when the stream starts to inject initial data
   */
  injectInitialEntry?: (data: T) => void;
  /**
   * Filter/deduplicate patches before applying them
   */
  deduplicatePatches?: (
    patches: Operation[],
    current: T | undefined
  ) => Operation[];
  /**
   * Whether to reconnect if the socket closes cleanly without a finished message.
   * Defaults to true to keep long-lived streams healthy across idle timeouts.
   */
  reconnectOnCleanClose?: boolean;
  /**
   * Whether to reconnect after an error/unclean close.
   * Defaults to true to allow recovery from transient failures.
   */
  reconnectOnError?: boolean;
}

interface UseJsonPatchStreamResult<T> {
  data: T | undefined;
  isConnected: boolean;
  error: string | null;
}

/**
 * Generic hook for consuming WebSocket streams that send JSON messages with patches
 */
export const useJsonPatchWsStream = <T extends object>(
  endpoint: string | undefined,
  enabled: boolean,
  initialData: () => T,
  options?: UseJsonPatchStreamOptions<T>
): UseJsonPatchStreamResult<T> => {
  const [data, setData] = useState<T | undefined>(undefined);
  const [isConnected, setIsConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const dataRef = useRef<T | undefined>(undefined);
  const retryTimerRef = useRef<number | null>(null);
  const retryAttemptsRef = useRef<number>(0);
  const [retryNonce, setRetryNonce] = useState(0);
  const finishedRef = useRef<boolean>(false);
  const hadErrorRef = useRef<boolean>(false);

  const injectInitialEntry = options?.injectInitialEntry;
  const deduplicatePatches = options?.deduplicatePatches;
  const reconnectOnCleanClose = options?.reconnectOnCleanClose ?? true;
  const reconnectOnError = options?.reconnectOnError ?? true;

  const scheduleReconnect = useCallback(() => {
    // Exponential backoff with cap: 1s, 2s, 4s, 8s (max), then stay at 8s
    const attempt = retryAttemptsRef.current;
    const delay = Math.min(8000, 1000 * Math.pow(2, attempt));
    retryTimerRef.current = window.setTimeout(() => {
      retryTimerRef.current = null;
      setRetryNonce((n) => n + 1);
    }, delay);
  }, [setRetryNonce]);

  const requestReconnect = useCallback(() => {
    if (retryTimerRef.current) return; // already scheduled
    retryAttemptsRef.current += 1;
    scheduleReconnect();
  }, [scheduleReconnect]);

  useEffect(() => {
    const closeWebSocket = (ws: WebSocket) => {
      ws.onmessage = null;
      ws.onerror = null;
      ws.onclose = null;

      if (ws.readyState === WebSocket.CONNECTING) {
        // Avoid closing during CONNECTING to prevent console warnings.
        ws.onopen = () => ws.close();
        return;
      }

      ws.onopen = null;
      if (
        ws.readyState === WebSocket.OPEN ||
        ws.readyState === WebSocket.CLOSING
      ) {
        ws.close();
      }
    };

    if (!enabled || !endpoint) {
      // Close connection and reset state
      if (wsRef.current) {
        closeWebSocket(wsRef.current);
        wsRef.current = null;
      }
      if (retryTimerRef.current) {
        window.clearTimeout(retryTimerRef.current);
        retryTimerRef.current = null;
      }
      retryAttemptsRef.current = 0;
      finishedRef.current = false;
      setData(undefined);
      setIsConnected(false);
      setError(null);
      dataRef.current = undefined;
      return;
    }

    // Initialize data
    if (!dataRef.current) {
      dataRef.current = initialData();

      // Inject initial entry if provided
      if (injectInitialEntry) {
        injectInitialEntry(dataRef.current);
      }
    }

    // Create WebSocket if it doesn't exist
    if (!wsRef.current) {
      // Reset finished flag for new connection
      finishedRef.current = false;

      // Convert HTTP endpoint to WebSocket endpoint
      const wsEndpoint = withApiTokenQuery(endpoint.replace(/^http/, 'ws'));
      const ws = new WebSocket(wsEndpoint);

      ws.onopen = () => {
        setError(null);
        setIsConnected(true);
        hadErrorRef.current = false;
        // Reset backoff on successful connection
        retryAttemptsRef.current = 0;
        if (retryTimerRef.current) {
          window.clearTimeout(retryTimerRef.current);
          retryTimerRef.current = null;
        }
      };

      ws.onmessage = (event) => {
        try {
          const msg: WsMsg = JSON.parse(event.data);

          // Handle JsonPatch messages (same as SSE json_patch event)
          if ('JsonPatch' in msg) {
            const patches: Operation[] = msg.JsonPatch;
            const filtered = deduplicatePatches
              ? deduplicatePatches(patches, dataRef.current)
              : patches;

            const current = dataRef.current;
            if (!filtered.length || !current) return;

            // Deep clone the current state before mutating it
            const next = structuredClone(current);

            // Apply patch (mutates the clone in place)
            applyPatch(next, filtered);

            dataRef.current = next;
            setData(next);
          }

          // Handle finished messages ({finished: true})
          // Treat finished as terminal - do NOT reconnect
          if ('finished' in msg) {
            finishedRef.current = true;
            ws.close(1000, 'finished');
            wsRef.current = null;
            setIsConnected(false);
          }
        } catch (err) {
          console.error('Failed to process WebSocket message:', err);
          setError('Failed to process stream update');
          // Force a resync on parse/patch errors.
          ws.close(1011, 'stream error');
        }
      };

      ws.onerror = () => {
        hadErrorRef.current = true;
        setError('Connection failed');
      };

      ws.onclose = (evt) => {
        setIsConnected(false);
        wsRef.current = null;

        const isCleanClose = evt?.code === 1000 && evt?.wasClean;
        if (finishedRef.current || (isCleanClose && !reconnectOnCleanClose)) {
          return;
        }
        if (hadErrorRef.current && !reconnectOnError) {
          return;
        }

        requestReconnect();
      };

      wsRef.current = ws;
    }

    return () => {
      if (wsRef.current) {
        closeWebSocket(wsRef.current);
        wsRef.current = null;
      }
      if (retryTimerRef.current) {
        window.clearTimeout(retryTimerRef.current);
        retryTimerRef.current = null;
      }
      finishedRef.current = false;
      dataRef.current = undefined;
      setData(undefined);
    };
  }, [
    endpoint,
    enabled,
    initialData,
    injectInitialEntry,
    deduplicatePatches,
    reconnectOnCleanClose,
    reconnectOnError,
    requestReconnect,
    retryNonce,
  ]);

  return { data, isConnected, error };
};
