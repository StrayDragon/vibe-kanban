import { useEffect, useState, useRef, useCallback } from 'react';
import { applyPatch } from 'rfc6902';
import type { Operation } from 'rfc6902';
import { createWebSocket } from '@/lib/api';

type WsBaseMsg = { seq?: number };
type WsJsonPatchMsg = WsBaseMsg & {
  JsonPatch: Operation[];
  invalidate?: unknown;
};
type WsFinishedMsg = WsBaseMsg & { finished: boolean };
type WsMsg = WsJsonPatchMsg | WsFinishedMsg;

type PendingPatchEntry = {
  seq: number | null;
  invalidate?: unknown;
  patches: Operation[];
};

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
  /**
   * Optional hint hook for consumers to receive server-side invalidation metadata.
   * This is meant for performance optimizations (cache invalidation / selective recompute),
   * and MUST NOT be used as the sole correctness mechanism.
   */
  onInvalidate?: (invalidate: unknown, meta: { seq: number | null }) => void;
}

interface UseJsonPatchStreamResult<T> {
  data: T | undefined;
  isConnected: boolean;
  isResyncing: boolean;
  error: string | null;
  resync: (reason?: string) => void;
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const decodePointerSegment = (segment: string) =>
  segment.replace(/~1/g, '/').replace(/~0/g, '~');

const splitPointerPath = (path: string) =>
  path
    .split('/')
    .filter((segment) => segment.length > 0)
    .map(decodePointerSegment);

const tryApplyIdMapPatches = <TCurrent extends object>(
  current: TCurrent,
  patches: Operation[]
): TCurrent | null => {
  if (!isRecord(current)) return null;

  let nextRoot: Record<string, unknown> | null = null;
  const clonedMaps = new Map<string, Record<string, unknown>>();

  const ensureRootClone = () => {
    if (nextRoot) return nextRoot;
    nextRoot = { ...current };
    return nextRoot;
  };

  const ensureMapClone = (mapKey: string): Record<string, unknown> | null => {
    const existing = clonedMaps.get(mapKey);
    if (existing) return existing;

    const baseValue =
      nextRoot && Object.prototype.hasOwnProperty.call(nextRoot, mapKey)
        ? nextRoot[mapKey]
        : current[mapKey];
    if (!isRecord(baseValue)) return null;

    const clone = { ...baseValue };
    clonedMaps.set(mapKey, clone);
    ensureRootClone()[mapKey] = clone;
    return clone;
  };

  for (const patch of patches) {
    const segments = splitPointerPath(patch.path);
    if (segments.length === 0) return null;

    // Snapshot shape: replace /<mapKey> with a full object
    if (segments.length === 1) {
      if (patch.op !== 'replace') return null;
      if (!isRecord(patch.value)) return null;
      ensureRootClone()[segments[0]] = patch.value;
      continue;
    }

    // Delta shape: add/replace/remove /<mapKey>/<id>
    if (segments.length !== 2) return null;
    const [mapKey, id] = segments;
    const map = ensureMapClone(mapKey);
    if (!map) return null;

    if (patch.op === 'remove') {
      delete map[id];
      continue;
    }
    if (patch.op === 'add' || patch.op === 'replace') {
      map[id] = patch.value;
      continue;
    }

    return null;
  }

  return nextRoot ? (nextRoot as TCurrent) : current;
};

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
  const [isResyncing, setIsResyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const dataRef = useRef<T | undefined>(undefined);
  const retryTimerRef = useRef<number | null>(null);
  const retryAttemptsRef = useRef<number>(0);
  const finishedRef = useRef<boolean>(false);
  const hadErrorRef = useRef<boolean>(false);
  const connectRef = useRef<
    ((kind: 'initial' | 'retry' | 'resync') => void) | null
  >(null);
  const closeForResyncRef = useRef<boolean>(false);
  const closeOnOpenForResyncRef = useRef<boolean>(false);
  const lastSeqRef = useRef<number | null>(null);
  const maxSeqQueuedRef = useRef<number | null>(null);
  const pendingPatchesRef = useRef<PendingPatchEntry[]>([]);
  const flushScheduledRef = useRef<boolean>(false);
  const flushHandleRef = useRef<number | null>(null);
  const flushHandleKindRef = useRef<'raf' | 'timeout' | null>(null);

  const injectInitialEntry = options?.injectInitialEntry;
  const deduplicatePatches = options?.deduplicatePatches;
  const reconnectOnCleanClose = options?.reconnectOnCleanClose ?? true;
  const reconnectOnError = options?.reconnectOnError ?? true;
  const onInvalidate = options?.onInvalidate;

  const clearRetryTimer = useCallback(() => {
    if (!retryTimerRef.current) return;
    window.clearTimeout(retryTimerRef.current);
    retryTimerRef.current = null;
  }, []);

  const cancelFlush = useCallback(() => {
    if (!flushHandleRef.current) return;
    const handle = flushHandleRef.current;
    flushHandleRef.current = null;
    flushScheduledRef.current = false;
    const kind = flushHandleKindRef.current;
    flushHandleKindRef.current = null;
    if (kind === 'raf') {
      window.cancelAnimationFrame(handle);
      return;
    }
    window.clearTimeout(handle);
  }, []);

  const flushPendingPatches = useCallback(() => {
    cancelFlush();

    const entries = pendingPatchesRef.current;
    if (entries.length === 0) return;
    pendingPatchesRef.current = [];

    const initial = dataRef.current;
    if (initial === undefined) return;
    let current = initial as T;

    try {
      for (const entry of entries) {
        const patches = entry.patches;
        if (!patches.length) continue;

        const nextFast: T | null = tryApplyIdMapPatches(current, patches);
        if (nextFast) {
          current = nextFast;
        } else {
          const next = structuredClone(current);
          applyPatch(next, patches);
          current = next;
        }

        if (typeof entry.seq === 'number') {
          lastSeqRef.current = entry.seq;
          maxSeqQueuedRef.current = entry.seq;
        }
      }
    } catch (err) {
      console.error('Failed to apply queued patches:', err);
      setError('Failed to process stream update');
      pendingPatchesRef.current = [];
      closeForResyncRef.current = true;
      wsRef.current?.close(4000, 'resync:stream_error');
      return;
    }

    dataRef.current = current;
    setData(current);
  }, [cancelFlush]);

  const scheduleFlush = useCallback(() => {
    if (flushScheduledRef.current) return;
    flushScheduledRef.current = true;

    if (typeof window.requestAnimationFrame === 'function') {
      flushHandleKindRef.current = 'raf';
      flushHandleRef.current = window.requestAnimationFrame(() => {
        flushHandleRef.current = null;
        flushHandleKindRef.current = null;
        flushScheduledRef.current = false;
        flushPendingPatches();
      });
      return;
    }

    flushHandleKindRef.current = 'timeout';
    flushHandleRef.current = window.setTimeout(() => {
      flushHandleRef.current = null;
      flushHandleKindRef.current = null;
      flushScheduledRef.current = false;
      flushPendingPatches();
    }, 0);
  }, [flushPendingPatches]);

  const scheduleReconnect = useCallback(() => {
    // Exponential backoff with cap: 1s, 2s, 4s, 8s (max), then stay at 8s
    const attempt = retryAttemptsRef.current;
    const delay = Math.min(8000, 1000 * Math.pow(2, attempt));
    retryTimerRef.current = window.setTimeout(() => {
      retryTimerRef.current = null;
      connectRef.current?.('retry');
    }, delay);
  }, []);

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
      // Close connection and hard-reset state
      if (wsRef.current) {
        closeWebSocket(wsRef.current);
        wsRef.current = null;
      }
      cancelFlush();
      pendingPatchesRef.current = [];
      clearRetryTimer();
      retryAttemptsRef.current = 0;
      finishedRef.current = false;
      closeForResyncRef.current = false;
      closeOnOpenForResyncRef.current = false;
      lastSeqRef.current = null;
      maxSeqQueuedRef.current = null;
      setData(undefined);
      setIsConnected(false);
      setIsResyncing(false);
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

    connectRef.current = (kind) => {
      if (!enabled || !endpoint) return;
      if (wsRef.current) return;

      const buildEndpoint = (): string => {
        if (kind !== 'retry') return endpoint;
        if (typeof lastSeqRef.current !== 'number') return endpoint;
        try {
          const url = new URL(endpoint, window.location.origin);
          url.searchParams.set('after_seq', String(lastSeqRef.current));
          return url.pathname + url.search + url.hash;
        } catch {
          return endpoint;
        }
      };

      // Reset finished flag for new connection
      finishedRef.current = false;
      hadErrorRef.current = false;

      if (kind === 'resync') {
        setIsResyncing(true);
      }

      const ws = createWebSocket(buildEndpoint());

      ws.onopen = () => {
        if (closeOnOpenForResyncRef.current) {
          closeOnOpenForResyncRef.current = false;
          closeForResyncRef.current = true;
          ws.close(4000, 'resync');
          return;
        }

        setError(null);
        setIsConnected(true);
        setIsResyncing(false);
        hadErrorRef.current = false;
        // Reset backoff on successful connection
        retryAttemptsRef.current = 0;
        clearRetryTimer();
      };

      ws.onmessage = (event) => {
        try {
          const msg: WsMsg = JSON.parse(event.data);
          const seq = typeof msg.seq === 'number' ? msg.seq : null;

          if (
            typeof seq === 'number' &&
            typeof (maxSeqQueuedRef.current ?? lastSeqRef.current) ===
              'number' &&
            seq < (maxSeqQueuedRef.current ?? lastSeqRef.current)!
          ) {
            // Seq going backwards is not expected; force a full resync.
            closeForResyncRef.current = true;
            ws.close(4000, 'resync:seq_backwards');
            return;
          }

          // Handle JsonPatch messages (same as SSE json_patch event)
          if ('JsonPatch' in msg) {
            const patches: Operation[] = msg.JsonPatch;
            const filtered = deduplicatePatches
              ? deduplicatePatches(patches, dataRef.current)
              : patches;

            const current = dataRef.current;
            if (!filtered.length || !current) return;

            if (typeof seq === 'number') {
              maxSeqQueuedRef.current = seq;
            }

            if (
              typeof onInvalidate === 'function' &&
              msg.invalidate !== undefined
            ) {
              onInvalidate(msg.invalidate, { seq });
            }

            pendingPatchesRef.current.push({
              seq,
              invalidate: msg.invalidate,
              patches: filtered,
            });
            scheduleFlush();
          }

          // Handle finished messages ({finished: true})
          // Treat finished as terminal - do NOT reconnect
          if ('finished' in msg) {
            if (typeof seq === 'number') {
              lastSeqRef.current = seq;
              maxSeqQueuedRef.current = seq;
            }
            finishedRef.current = true;
            ws.close(1000, 'finished');
          }
        } catch (err) {
          console.error('Failed to process WebSocket message:', err);
          setError('Failed to process stream update');
          // Force a resync on parse/patch errors.
          closeForResyncRef.current = true;
          ws.close(4000, 'resync:stream_error');
        }
      };

      ws.onerror = () => {
        hadErrorRef.current = true;
        setError('Connection failed');
      };

      ws.onclose = (evt) => {
        // Best-effort: apply any queued patches before deciding how to reconnect.
        flushPendingPatches();

        setIsConnected(false);
        wsRef.current = null;

        if (finishedRef.current) {
          setIsResyncing(false);
          return;
        }

        if (closeForResyncRef.current) {
          closeForResyncRef.current = false;
          // Immediate reconnect, keep current UI state.
          connectRef.current?.('resync');
          return;
        }

        // Terminal close codes: don't reconnect (avoid "infinite reconnect" background loops).
        if (evt?.code === 4404) {
          const reason = typeof evt.reason === 'string' ? evt.reason : '';
          const message =
            reason === 'workspace_not_found'
              ? 'Workspace not found'
              : reason.trim().length > 0
                ? reason
                : 'Resource not found';
          setError(message);
          setIsResyncing(false);
          return;
        }

        const isCleanClose = evt?.code === 1000 && evt?.wasClean;
        if (isCleanClose && !reconnectOnCleanClose) {
          setIsResyncing(false);
          return;
        }
        if (hadErrorRef.current && !reconnectOnError) {
          setIsResyncing(false);
          return;
        }

        setIsResyncing(false);
        requestReconnect();
      };

      wsRef.current = ws;
    };

    // Create WebSocket if it doesn't exist.
    // This preserves existing UI state while (re)connecting.
    if (!wsRef.current) {
      connectRef.current?.('initial');
    }

    return () => {
      if (wsRef.current) {
        closeWebSocket(wsRef.current);
        wsRef.current = null;
      }
      cancelFlush();
      pendingPatchesRef.current = [];
      clearRetryTimer();
      finishedRef.current = false;
      closeForResyncRef.current = false;
      closeOnOpenForResyncRef.current = false;
      lastSeqRef.current = null;
      maxSeqQueuedRef.current = null;
      dataRef.current = undefined;
      setData(undefined);
      setIsResyncing(false);
    };
  }, [
    endpoint,
    enabled,
    initialData,
    injectInitialEntry,
    deduplicatePatches,
    reconnectOnCleanClose,
    reconnectOnError,
    onInvalidate,
    requestReconnect,
    clearRetryTimer,
    cancelFlush,
    flushPendingPatches,
    scheduleFlush,
  ]);

  const resync = useCallback(
    (reason?: string) => {
      if (!enabled || !endpoint) return;

      flushPendingPatches();
      clearRetryTimer();
      retryAttemptsRef.current = 0;

      if (!wsRef.current) {
        connectRef.current?.('resync');
        return;
      }

      setIsResyncing(true);
      closeForResyncRef.current = true;

      if (wsRef.current.readyState === WebSocket.CONNECTING) {
        closeOnOpenForResyncRef.current = true;
        return;
      }

      const closeReason = reason ? `resync:${reason}` : 'resync';
      wsRef.current.close(4000, closeReason.slice(0, 120));
    },
    [clearRetryTimer, enabled, endpoint, flushPendingPatches]
  );

  return { data, isConnected, isResyncing, error, resync };
};
