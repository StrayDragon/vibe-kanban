import { useEffect, useState, useRef, useCallback } from 'react';
import type { ApiResponse, LogHistoryPage, PatchType } from 'shared/types';
import { streamLogEntries } from '@/utils/streamLogEntries';

type LogEntry = Extract<PatchType, { type: 'STDOUT' } | { type: 'STDERR' }>;

type IndexedRawEntry = {
  entry_index: bigint | number;
  entry: LogEntry;
};

interface UseLogStreamResult {
  logs: LogEntry[];
  error: string | null;
  hasMoreHistory: boolean;
  loadingOlder: boolean;
  truncated: boolean;
  loadOlder: () => Promise<void>;
}

const RAW_HISTORY_PAGE_SIZE = 200;
const RAW_BUFFER_LIMIT = 2000;
const RAW_BUFFER_LIMIT_MAX = 10000;
const MAX_RECONNECT_DELAY_MS = 8000;

const isRawEntry = (entry: PatchType): entry is LogEntry =>
  entry.type === 'STDOUT' || entry.type === 'STDERR';

const normalizeIndex = (index: bigint | number) =>
  typeof index === 'bigint' ? index : BigInt(index);

const compareIndices = (a: bigint, b: bigint) =>
  a < b ? -1 : a > b ? 1 : 0;

export const useLogStream = (processId: string): UseLogStreamResult => {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [cursor, setCursor] = useState<bigint | null>(null);
  const [hasMoreHistory, setHasMoreHistory] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [truncated, setTruncated] = useState(false);
  const [retryNonce, setRetryNonce] = useState(0);

  const hasMoreHistoryRef = useRef(hasMoreHistory);
  const bufferLimitRef = useRef<number>(RAW_BUFFER_LIMIT);
  const droppedLinesRef = useRef<boolean>(false);
  const entriesRef = useRef<Map<string, LogEntry>>(new Map());
  const entryOrderRef = useRef<bigint[]>([]);
  const controllerRef = useRef<ReturnType<typeof streamLogEntries> | null>(null);
  const retryTimerRef = useRef<number | null>(null);
  const retryAttemptsRef = useRef<number>(0);
  const refreshInFlightRef = useRef(false);
  const refreshedCycleRef = useRef(0);
  const finishedRef = useRef(false);
  const streamCycleRef = useRef(0);

  const updateTruncated = useCallback(() => {
    const truncatedNow = hasMoreHistoryRef.current || droppedLinesRef.current;
    setTruncated(truncatedNow);
  }, []);

  useEffect(() => {
    hasMoreHistoryRef.current = hasMoreHistory;
    updateTruncated();
  }, [hasMoreHistory, updateTruncated]);

  const fetchHistoryPage = useCallback(
    async (cursorValue: bigint | null) => {
      const params = new URLSearchParams();
      params.set('limit', String(RAW_HISTORY_PAGE_SIZE));
      if (cursorValue !== null) {
        params.set('cursor', String(cursorValue));
      }

      const res = await fetch(
        `/api/execution-processes/${processId}/raw-logs/v2?${params.toString()}`
      );
      if (!res.ok) {
        throw new Error('Failed to load log history');
      }
      const body = (await res.json()) as ApiResponse<LogHistoryPage>;
      if (!body.data) {
        throw new Error('No log history returned');
      }
      return body.data;
    },
    [processId]
  );

  const mergeCursor = useCallback((current: bigint | null, next: bigint | null) => {
    if (current === null) return next;
    if (next === null) return current;
    return current < next ? current : next;
  }, []);

  const rebuildLogs = useCallback(() => {
    const ordered = entryOrderRef.current;
    if (ordered.length > bufferLimitRef.current) {
      droppedLinesRef.current = true;
      const trimmed = ordered.slice(ordered.length - bufferLimitRef.current);
      const keep = new Set(trimmed.map((index) => index.toString()));
      for (const key of entriesRef.current.keys()) {
        if (!keep.has(key)) {
          entriesRef.current.delete(key);
        }
      }
      entryOrderRef.current = trimmed;
    }

    const nextLogs = entryOrderRef.current
      .map((index) => entriesRef.current.get(index.toString()))
      .filter(Boolean) as LogEntry[];

    setLogs(nextLogs);
    updateTruncated();
  }, [updateTruncated]);

  const upsertEntries = useCallback(
    (entries: IndexedRawEntry[]) => {
      let added = false;
      for (const entry of entries) {
        const normalizedIndex = normalizeIndex(entry.entry_index);
        const key = normalizedIndex.toString();
        if (!entriesRef.current.has(key)) {
          entryOrderRef.current.push(normalizedIndex);
          added = true;
        }
        entriesRef.current.set(key, entry.entry);
      }

      if (added) {
        entryOrderRef.current.sort(compareIndices);
      }

      rebuildLogs();
    },
    [rebuildLogs]
  );

  const applyHistoryPage = useCallback(
    (page: LogHistoryPage, mode: 'tail' | 'older') => {
      const incoming = page.entries
        .filter((entry) => isRawEntry(entry.entry))
        .map((entry) => ({
          entry_index: entry.entry_index,
          entry: entry.entry as LogEntry,
        }));

      if (incoming.length) {
        upsertEntries(incoming);
      } else {
        rebuildLogs();
      }

      const nextCursor =
        page.next_cursor === null
          ? null
          : normalizeIndex(page.next_cursor as bigint | number);

      if (mode === 'older') {
        setCursor(nextCursor);
        setHasMoreHistory(page.has_more);
      } else {
        setCursor((prev) => mergeCursor(prev, nextCursor));
        setHasMoreHistory((prev) => prev || page.has_more);
      }
    },
    [mergeCursor, rebuildLogs, upsertEntries]
  );

  const scheduleReconnect = useCallback(() => {
    if (retryTimerRef.current) return;
    retryAttemptsRef.current += 1;
    const delay = Math.min(
      MAX_RECONNECT_DELAY_MS,
      1000 * Math.pow(2, retryAttemptsRef.current - 1)
    );
    retryTimerRef.current = window.setTimeout(() => {
      retryTimerRef.current = null;
      setRetryNonce((prev) => prev + 1);
    }, delay);
  }, []);

  useEffect(() => {
    if (!processId) {
      return;
    }

    setLogs([]);
    setError(null);
    setCursor(null);
    setHasMoreHistory(false);
    setLoadingOlder(false);
    setTruncated(false);
    droppedLinesRef.current = false;
    bufferLimitRef.current = RAW_BUFFER_LIMIT;
    entriesRef.current = new Map();
    entryOrderRef.current = [];
    retryAttemptsRef.current = 0;
    finishedRef.current = false;

    if (retryTimerRef.current) {
      window.clearTimeout(retryTimerRef.current);
      retryTimerRef.current = null;
    }

    setRetryNonce(0);
  }, [processId]);

  useEffect(() => {
    if (!processId) {
      return;
    }

    streamCycleRef.current += 1;
    const cycle = streamCycleRef.current;
    finishedRef.current = false;

    const refreshLatestHistory = async () => {
      if (refreshInFlightRef.current) return;
      if (refreshedCycleRef.current === cycle) return;
      refreshInFlightRef.current = true;
      try {
        const page = await fetchHistoryPage(null);
        if (streamCycleRef.current !== cycle) return;
        applyHistoryPage(page, 'tail');
        refreshedCycleRef.current = cycle;
      } catch (err) {
        if (streamCycleRef.current === cycle) {
          setError('Failed to load log history');
        }
      } finally {
        refreshInFlightRef.current = false;
        if (streamCycleRef.current === cycle) {
          updateTruncated();
        }
      }
    };

    const openStream = () => {
      controllerRef.current?.close();

      const controller = streamLogEntries(
        `/api/execution-processes/${processId}/raw-logs/v2/ws`,
        {
          onOpen: () => {
            if (streamCycleRef.current !== cycle) return;
            setError(null);
            retryAttemptsRef.current = 0;
            if (retryTimerRef.current) {
              window.clearTimeout(retryTimerRef.current);
              retryTimerRef.current = null;
            }
            refreshLatestHistory();
          },
          onAppend: (entryIndex, entry) => {
            if (!isRawEntry(entry)) return;
            upsertEntries([{ entry_index: entryIndex, entry }]);
          },
          onReplace: (entryIndex, entry) => {
            if (!isRawEntry(entry)) return;
            upsertEntries([{ entry_index: entryIndex, entry }]);
          },
          onFinished: () => {
            finishedRef.current = true;
            controller.close();
          },
          onError: () => {
            if (streamCycleRef.current !== cycle || finishedRef.current) return;
            setError('Connection failed');
            scheduleReconnect();
          },
        }
      );

      controllerRef.current = controller;
    };

    openStream();
    refreshLatestHistory();

    return () => {
      if (controllerRef.current) {
        controllerRef.current.close();
        controllerRef.current = null;
      }
      if (retryTimerRef.current) {
        window.clearTimeout(retryTimerRef.current);
        retryTimerRef.current = null;
      }
      refreshInFlightRef.current = false;
    };
  }, [
    applyHistoryPage,
    fetchHistoryPage,
    processId,
    retryNonce,
    scheduleReconnect,
    updateTruncated,
    upsertEntries,
  ]);

  const loadOlder = useCallback(async () => {
    if (loadingOlder || !hasMoreHistory) {
      return;
    }

    setLoadingOlder(true);
    try {
      if (bufferLimitRef.current < RAW_BUFFER_LIMIT_MAX) {
        bufferLimitRef.current = Math.min(
          bufferLimitRef.current + RAW_HISTORY_PAGE_SIZE,
          RAW_BUFFER_LIMIT_MAX
        );
      }

      const page = await fetchHistoryPage(cursor);
      applyHistoryPage(page, 'older');
    } catch (err) {
      setError('Failed to load more history');
    } finally {
      setLoadingOlder(false);
      updateTruncated();
    }
  }, [
    applyHistoryPage,
    cursor,
    fetchHistoryPage,
    hasMoreHistory,
    loadingOlder,
    updateTruncated,
  ]);

  return { logs, error, hasMoreHistory, loadingOlder, truncated, loadOlder };
};
