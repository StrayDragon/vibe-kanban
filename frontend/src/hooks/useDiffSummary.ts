import { useEffect, useState } from 'react';
import { useDiffStream } from '@/hooks/useDiffStream';
import type { DiffSummary } from 'shared/types';

export function useDiffSummary(attemptId: string | null) {
  const [refreshNonce, setRefreshNonce] = useState(0);
  const [lastSummary, setLastSummary] = useState<DiffSummary | null>(null);
  const [connectEnabled, setConnectEnabled] = useState(false);

  useEffect(() => {
    setRefreshNonce(0);
    setLastSummary(null);
    setConnectEnabled(false);
  }, [attemptId]);

  useEffect(() => {
    if (!attemptId) return;
    const timer = window.setTimeout(() => setConnectEnabled(true), 200);
    return () => window.clearTimeout(timer);
  }, [attemptId]);

  useEffect(() => {
    if (!attemptId) return;
    const interval = window.setInterval(() => {
      setRefreshNonce((prev) => prev + 1);
    }, 10000);
    return () => window.clearInterval(interval);
  }, [attemptId]);

  const { summary, error } = useDiffStream(attemptId, connectEnabled, {
    statsOnly: true,
    refreshNonce,
  });

  useEffect(() => {
    if (summary) {
      setLastSummary(summary);
    }
  }, [summary]);

  const effectiveSummary = summary ?? lastSummary;

  return {
    fileCount: effectiveSummary?.fileCount ?? 0,
    added: effectiveSummary?.added ?? 0,
    deleted: effectiveSummary?.deleted ?? 0,
    error,
  };
}
