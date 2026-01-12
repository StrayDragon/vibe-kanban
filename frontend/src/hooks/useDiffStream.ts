import { useCallback, useMemo } from 'react';
import type { Diff, DiffSummary, PatchType } from 'shared/types';
import { useJsonPatchWsStream } from './useJsonPatchWsStream';

interface DiffEntries {
  [filePath: string]: PatchType;
}

type DiffStreamEvent = {
  entries: DiffEntries;
  summary?: DiffSummary;
  blocked?: boolean;
  blockedReason?: 'summary_failed' | 'threshold_exceeded' | null;
};

export interface UseDiffStreamOptions {
  statsOnly?: boolean;
  force?: boolean;
  refreshNonce?: number;
}

interface UseDiffStreamResult {
  diffs: Diff[];
  summary?: DiffSummary;
  blocked: boolean;
  blockedReason?: 'summary_failed' | 'threshold_exceeded' | null;
  error: string | null;
}

export const useDiffStream = (
  attemptId: string | null,
  enabled: boolean,
  options?: UseDiffStreamOptions
): UseDiffStreamResult => {
  const endpoint = (() => {
    if (!attemptId) return undefined;
    const query = `/api/task-attempts/${attemptId}/diff/ws`;
    const params = new URLSearchParams();
    if (typeof options?.statsOnly === 'boolean') {
      params.set('stats_only', String(options.statsOnly));
    }
    if (options?.force) {
      params.set('force', 'true');
    }
    if (typeof options?.refreshNonce === 'number') {
      params.set('refresh', String(options.refreshNonce));
    }
    const paramString = params.toString();
    return paramString ? `${query}?${paramString}` : query;
  })();

  const initialData = useCallback(
    (): DiffStreamEvent => ({
      entries: {},
    }),
    []
  );

  const { data, error } = useJsonPatchWsStream<DiffStreamEvent>(
    endpoint,
    enabled && !!attemptId,
    initialData,
    {
      reconnectOnCleanClose: options?.statsOnly ? false : undefined,
      reconnectOnError: options?.statsOnly ? false : undefined,
    }
    // No need for injectInitialEntry or deduplicatePatches for diffs
  );

  const diffs = useMemo(() => {
    return Object.values(data?.entries ?? {})
      .filter((entry) => entry?.type === 'DIFF')
      .map((entry) => entry.content);
  }, [data?.entries]);

  return {
    diffs,
    summary: data?.summary,
    blocked: data?.blocked ?? false,
    blockedReason: data?.blockedReason ?? null,
    error,
  };
};
