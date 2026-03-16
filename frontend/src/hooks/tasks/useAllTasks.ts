import { useCallback, useEffect, useMemo, useState } from 'react';
import type { Operation } from 'rfc6902';
import { useJsonPatchWsStream } from '../useJsonPatchWsStream';
import { normalizeIdMapPatches } from '../jsonPatchUtils';
import type { TaskStatus, TaskWithAttemptStatus } from 'shared/types';
import { useOptimisticTasksStore } from '@/stores/useOptimisticTasksStore';

type TasksState = {
  tasks: Record<string, TaskWithAttemptStatus>;
};

const taskStatusRank: Record<TaskStatus, number> = {
  todo: 0,
  inprogress: 1,
  inreview: 2,
  done: 3,
  cancelled: 4,
};

export interface UseAllTasksResult {
  tasks: TaskWithAttemptStatus[];
  tasksById: Record<string, TaskWithAttemptStatus>;
  streamTasksById: Record<string, TaskWithAttemptStatus>;
  tasksByStatus: Record<TaskStatus, TaskWithAttemptStatus[]>;
  isLoading: boolean;
  isConnected: boolean;
  isResyncing: boolean;
  error: string | null;
  resync: () => void;
}

export interface UseAllTasksOptions {
  includeArchived?: boolean;
}

/**
 * Stream tasks across all projects via WebSocket (JSON Patch).
 */
export const useAllTasks = (
  options?: UseAllTasksOptions
): UseAllTasksResult => {
  const includeArchived = options?.includeArchived ?? false;
  const [optimisticStaleTick, setOptimisticStaleTick] = useState(0);
  const endpoint = includeArchived
    ? '/api/tasks/stream/ws?include_archived=true'
    : '/api/tasks/stream/ws';

  const initialData = useCallback((): TasksState => ({ tasks: {} }), []);
  const deduplicatePatches = useCallback(
    (patches: Operation[], current: TasksState | undefined) =>
      normalizeIdMapPatches(patches, current?.tasks, '/tasks/'),
    []
  );

  const { data, isConnected, isResyncing, error, resync } =
    useJsonPatchWsStream(endpoint, true, initialData, { deduplicatePatches });

  const inserts = useOptimisticTasksStore((state) => state.inserts);
  const overrides = useOptimisticTasksStore((state) => state.overrides);
  const tombstones = useOptimisticTasksStore((state) => state.tombstones);
  const clearInsert = useOptimisticTasksStore((state) => state.clearInsert);
  const setOverride = useOptimisticTasksStore((state) => state.setOverride);
  const clearOverride = useOptimisticTasksStore((state) => state.clearOverride);
  const clearTombstone = useOptimisticTasksStore(
    (state) => state.clearTombstone
  );
  const markResyncAttempt = useOptimisticTasksStore(
    (state) => state.markResyncAttempt
  );

  const streamTasksById = useMemo(() => data?.tasks ?? {}, [data?.tasks]);

  const hasOptimisticState = useMemo(() => {
    return (
      Object.keys(inserts).length > 0 ||
      Object.keys(overrides).length > 0 ||
      Object.keys(tombstones).length > 0
    );
  }, [inserts, overrides, tombstones]);

  useEffect(() => {
    if (!hasOptimisticState) return;
    const timer = window.setTimeout(() => {
      setOptimisticStaleTick((value) => value + 1);
    }, 250);
    return () => window.clearTimeout(timer);
  }, [hasOptimisticState, optimisticStaleTick]);

  const mergedTasksById = useMemo(() => {
    const merged: Record<string, TaskWithAttemptStatus> = {
      ...streamTasksById,
    };

    Object.values(inserts).forEach(({ task }) => {
      if (!includeArchived && task.archived_kanban_id) return;
      merged[task.id] = task;
    });

    Object.entries(overrides).forEach(([taskId, { patch }]) => {
      const base = merged[taskId];
      if (!base) return;
      merged[taskId] = { ...base, ...patch };
    });

    Object.keys(tombstones).forEach((taskId) => {
      delete merged[taskId];
    });

    return merged;
  }, [includeArchived, inserts, overrides, streamTasksById, tombstones]);

  const { tasks, tasksById, tasksByStatus } = useMemo(() => {
    const merged: Record<string, TaskWithAttemptStatus> = {
      ...mergedTasksById,
    };
    const byStatus: Record<TaskStatus, TaskWithAttemptStatus[]> = {
      todo: [],
      inprogress: [],
      inreview: [],
      done: [],
      cancelled: [],
    };

    Object.values(merged).forEach((task) => {
      byStatus[task.status]?.push(task);
    });

    const sorted = Object.values(merged).sort(
      (a, b) =>
        new Date(b.created_at as string).getTime() -
        new Date(a.created_at as string).getTime()
    );

    (Object.values(byStatus) as TaskWithAttemptStatus[][]).forEach((list) => {
      list.sort(
        (a, b) =>
          new Date(b.created_at as string).getTime() -
          new Date(a.created_at as string).getTime()
      );
    });

    return { tasks: sorted, tasksById: merged, tasksByStatus: byStatus };
  }, [mergedTasksById]);

  const isLoading = !data && !error; // until first snapshot

  useEffect(() => {
    Object.keys(inserts).forEach((taskId) => {
      if (streamTasksById[taskId]) {
        clearInsert(taskId);
      }
    });

    Object.keys(tombstones).forEach((taskId) => {
      if (!streamTasksById[taskId]) {
        clearTombstone(taskId);
      }
    });

    Object.entries(overrides).forEach(([taskId, entry]) => {
      const streamTask = streamTasksById[taskId];
      if (!streamTask) return;

      const patch = entry.patch;
      const satisfied = Object.entries(patch).every(([key, value]) => {
        return (streamTask as Record<string, unknown>)[key] === value;
      });
      if (satisfied) {
        clearOverride(taskId);
        return;
      }

      // If the stream skips over an optimistic status (e.g. todo -> inreview),
      // a stale optimistic override would otherwise mask the authoritative task
      // status indefinitely until a full refresh.
      const patchStatus = patch.status;
      if (
        typeof patchStatus === 'string' &&
        streamTask.status !== patchStatus
      ) {
        const streamRank = taskStatusRank[streamTask.status];
        const patchRank = taskStatusRank[patchStatus as TaskStatus];
        if (streamRank > patchRank) {
          const updatedAtMs = Date.parse(streamTask.updated_at);
          const skewToleranceMs = 5_000;
          const baseUpdatedAtMs = entry.meta.baseUpdatedAtMs;
          const hasBaseUpdatedAtMs =
            typeof baseUpdatedAtMs === 'number' &&
            Number.isFinite(baseUpdatedAtMs);
          const serverLooksNewer =
            Number.isFinite(updatedAtMs) &&
            (hasBaseUpdatedAtMs
              ? updatedAtMs > baseUpdatedAtMs
              : updatedAtMs >= entry.meta.setAt - skewToleranceMs);
          if (serverLooksNewer) {
            const nextPatch = { ...patch };
            delete nextPatch.status;
            if (Object.keys(nextPatch).length === 0) {
              clearOverride(taskId);
            } else {
              setOverride(taskId, nextPatch, { replace: true });
            }
          }
        }
      }
    });
  }, [
    clearInsert,
    clearOverride,
    clearTombstone,
    inserts,
    overrides,
    setOverride,
    streamTasksById,
    tombstones,
  ]);

  useEffect(() => {
    const now = Date.now();
    const resyncAfterMs = 1200;
    const minResyncGapMs = 800;
    const maxAttempts = 2;

    const shouldResync = (meta: {
      setAt: number;
      resyncAttempts: number;
      lastResyncAt: number | null;
    }) => {
      if (now - meta.setAt < resyncAfterMs) return false;
      if (meta.resyncAttempts >= maxAttempts) return false;
      if (meta.lastResyncAt && now - meta.lastResyncAt < minResyncGapMs) {
        return false;
      }
      return true;
    };

    const candidates: string[] = [];

    Object.values(inserts).forEach(({ task, meta }) => {
      if (streamTasksById[task.id]) return;
      if (!shouldResync(meta)) return;
      candidates.push(task.id);
    });

    Object.entries(overrides).forEach(([taskId, { patch, meta }]) => {
      const streamTask = streamTasksById[taskId];
      if (!streamTask) return;
      const satisfied = Object.entries(patch).every(([key, value]) => {
        return (streamTask as Record<string, unknown>)[key] === value;
      });
      if (satisfied) return;
      if (!shouldResync(meta)) return;
      candidates.push(taskId);
    });

    Object.entries(tombstones).forEach(([taskId, { meta }]) => {
      if (!streamTasksById[taskId]) return;
      if (!shouldResync(meta)) return;
      candidates.push(taskId);
    });

    if (candidates.length === 0) return;

    resync('optimistic-stale');
    candidates.forEach(markResyncAttempt);
  }, [
    inserts,
    markResyncAttempt,
    overrides,
    optimisticStaleTick,
    resync,
    streamTasksById,
    tombstones,
  ]);

  return {
    tasks,
    tasksById,
    streamTasksById,
    tasksByStatus,
    isLoading,
    isConnected,
    isResyncing,
    error,
    resync: () => resync('manual'),
  };
};
