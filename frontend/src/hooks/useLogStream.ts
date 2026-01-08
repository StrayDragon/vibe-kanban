import { useEffect, useState, useRef, useMemo, useCallback } from 'react';
import type { ApiResponse, LogHistoryPage, PatchType } from 'shared/types';
import { streamLogEntries } from '@/utils/streamLogEntries';

type LogEntry = Extract<PatchType, { type: 'STDOUT' } | { type: 'STDERR' }>;

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

const isRawEntry = (entry: PatchType): entry is LogEntry =>
  entry.type === 'STDOUT' || entry.type === 'STDERR';

export const useLogStream = (processId: string): UseLogStreamResult => {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [cursor, setCursor] = useState<bigint | null>(null);
  const [hasMoreHistory, setHasMoreHistory] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [truncated, setTruncated] = useState(false);
  const hasMoreHistoryRef = useRef(hasMoreHistory);
  const bufferLimitRef = useRef<number>(RAW_BUFFER_LIMIT);
  const droppedLinesRef = useRef<boolean>(false);

  const updateLogsWithLimit = useMemo(
    () => (nextLogs: LogEntry[], limit: number) => {
      if (nextLogs.length <= limit) {
        return nextLogs;
      }
      droppedLinesRef.current = true;
      return nextLogs.slice(nextLogs.length - limit);
    },
    []
  );

  const fetchHistoryPage = async (cursorValue: bigint | null) => {
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
  };

  const updateTruncated = useCallback(() => {
    const truncatedNow = hasMoreHistoryRef.current || droppedLinesRef.current;
    setTruncated(truncatedNow);
  }, []);

  useEffect(() => {
    hasMoreHistoryRef.current = hasMoreHistory;
    updateTruncated();
  }, [hasMoreHistory, updateTruncated]);

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

    let cancelled = false;

    const loadInitialHistory = async () => {
      try {
        const page = await fetchHistoryPage(null);
        if (cancelled) return;

        const entries = page.entries
          .map((entry) => entry.entry)
          .filter(isRawEntry);

        const limited = updateLogsWithLimit(entries, bufferLimitRef.current);
        setLogs(limited);
        setCursor(page.next_cursor ?? null);
        setHasMoreHistory(page.has_more);
      } catch (err) {
        if (!cancelled) {
          setError('Failed to load log history');
        }
      }
    };

    const openStream = () => {
      const controller = streamLogEntries(
        `/api/execution-processes/${processId}/raw-logs/v2/ws`,
        {
          onAppend: (_, entry) => {
            if (!isRawEntry(entry)) return;
            setLogs((prev) =>
              updateLogsWithLimit([...prev, entry], bufferLimitRef.current)
            );
            setTruncated(hasMoreHistoryRef.current || droppedLinesRef.current);
          },
          onReplace: (_, entry) => {
            if (!isRawEntry(entry)) return;
            setLogs((prev) =>
              updateLogsWithLimit([...prev, entry], bufferLimitRef.current)
            );
            setTruncated(hasMoreHistoryRef.current || droppedLinesRef.current);
          },
          onFinished: () => {
            controller.close();
          },
          onError: () => {
            setError('Connection failed');
          },
        }
      );
      return controller;
    };

    let controller = openStream();

    loadInitialHistory().finally(() => updateTruncated());

    return () => {
      cancelled = true;
      controller?.close();
    };
  }, [processId, updateLogsWithLimit, updateTruncated]);

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
      const entries = page.entries
        .map((entry) => entry.entry)
        .filter(isRawEntry);

      setLogs((prev) => {
        const merged = [...entries, ...prev];
        return updateLogsWithLimit(merged, bufferLimitRef.current);
      });
      setCursor(page.next_cursor ?? null);
      setHasMoreHistory(page.has_more);
    } catch (err) {
      setError('Failed to load more history');
    } finally {
      setLoadingOlder(false);
      updateTruncated();
    }
  }, [cursor, hasMoreHistory, loadingOlder, updateLogsWithLimit, updateTruncated]);

  return { logs, error, hasMoreHistory, loadingOlder, truncated, loadOlder };
};
