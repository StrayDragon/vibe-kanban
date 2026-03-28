import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { Operation } from 'rfc6902';
import { useJsonPatchWsStream } from '../useJsonPatchWsStream';
import { normalizeIdMapPatches } from '../jsonPatchUtils';
import type { TaskStatus, TaskWithAttemptStatus } from 'shared/types';
import { useOptimisticTasksStore } from '@/stores/useOptimisticTasksStore';
import {
  applyTaskDerivationChanges,
  buildTaskDerivationCache,
  EMPTY_TASKS_BY_STATUS,
  type TaskDerivationCache,
} from '../tasks/taskDerivation';

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

const EMPTY_TASKS_BY_ID: Record<string, TaskWithAttemptStatus> = {};

type TasksDerivationCache = TaskDerivationCache & { projectId: string };

export interface UseProjectTasksResult {
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

/**
 * Stream tasks for a project via WebSocket (JSON Patch) and expose as array + map.
 * Server sends initial snapshot: replace /tasks with an object keyed by id.
 * Live updates arrive at /tasks/<id> via add/replace/remove operations.
 */
export const useProjectTasks = (projectId: string): UseProjectTasksResult => {
  const endpoint = `/api/tasks/stream/ws?project_id=${encodeURIComponent(projectId)}`;
  const [connectEnabled, setConnectEnabled] = useState(false);
  const [optimisticStaleTick, setOptimisticStaleTick] = useState(0);
  const invalidatedTaskIdsRef = useRef<Set<string>>(new Set());
  const derivedCacheRef = useRef<TasksDerivationCache | null>(null);

  useEffect(() => {
    invalidatedTaskIdsRef.current.clear();
    derivedCacheRef.current = null;
    setConnectEnabled(false);
    if (!projectId) return;
    const timer = window.setTimeout(() => setConnectEnabled(true), 200);
    return () => window.clearTimeout(timer);
  }, [projectId]);

  const initialData = useCallback((): TasksState => ({ tasks: {} }), []);
  const deduplicatePatches = useCallback(
    (patches: Operation[], current: TasksState | undefined) =>
      normalizeIdMapPatches(patches, current?.tasks, '/tasks/'),
    []
  );

  const onInvalidate = useCallback((invalidate: unknown) => {
    if (!invalidate || typeof invalidate !== 'object') return;
    const record = invalidate as Record<string, unknown>;
    const taskIds = record.taskIds;
    if (!Array.isArray(taskIds)) return;
    taskIds.forEach((id) => {
      if (typeof id !== 'string') return;
      invalidatedTaskIdsRef.current.add(id);
    });
  }, []);

  const { data, isConnected, isResyncing, error, resync } =
    useJsonPatchWsStream(endpoint, connectEnabled, initialData, {
      deduplicatePatches,
      onInvalidate,
    });

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

  const streamTasksById = useMemo(
    () => data?.tasks ?? EMPTY_TASKS_BY_ID,
    [data?.tasks]
  );

  const hasOptimisticState = useMemo(() => {
    return (
      Object.keys(inserts).length > 0 ||
      Object.keys(overrides).length > 0 ||
      Object.keys(tombstones).length > 0
    );
  }, [inserts, overrides, tombstones]);

  useEffect(() => {
    if (!connectEnabled) return;
    if (!hasOptimisticState) return;
    const timer = window.setTimeout(() => {
      setOptimisticStaleTick((value) => value + 1);
    }, 250);
    return () => window.clearTimeout(timer);
  }, [connectEnabled, hasOptimisticState, optimisticStaleTick]);

  const { tasks, tasksById, tasksByStatus } = useMemo(() => {
    if (!data) {
      derivedCacheRef.current = null;
      return {
        tasks: [],
        tasksById: EMPTY_TASKS_BY_ID,
        tasksByStatus: EMPTY_TASKS_BY_STATUS,
      };
    }

    const tombstonedIds = new Set(Object.keys(tombstones));

    const insertedById: Record<string, TaskWithAttemptStatus> = {};
    Object.values(inserts).forEach(({ task }) => {
      if (tombstonedIds.has(task.id)) return;
      if (task.project_id !== projectId) return;
      if (task.archived_kanban_id) return;
      insertedById[task.id] = task;
    });

    const overridePatchesById: Record<
      string,
      Partial<TaskWithAttemptStatus>
    > = {};
    Object.entries(overrides).forEach(([taskId, entry]) => {
      if (tombstonedIds.has(taskId)) return;
      overridePatchesById[taskId] = entry.patch;
    });

    const getEffectiveTask = (taskId: string) => {
      if (tombstonedIds.has(taskId)) return null;
      const base = insertedById[taskId] ?? streamTasksById[taskId];
      if (!base) return null;
      const patch = overridePatchesById[taskId];
      if (!patch) return base;
      return { ...base, ...patch } satisfies TaskWithAttemptStatus;
    };

    const tasksById = Object.create(streamTasksById) as Record<
      string,
      TaskWithAttemptStatus
    >;
    Object.keys(tombstones).forEach((taskId) => {
      (tasksById as Record<string, unknown>)[taskId] = undefined;
    });
    Object.entries(insertedById).forEach(([taskId]) => {
      const effective = getEffectiveTask(taskId);
      if (effective) {
        tasksById[taskId] = effective;
      }
    });
    Object.entries(overridePatchesById).forEach(([taskId]) => {
      if (tombstonedIds.has(taskId)) return;
      if (Object.prototype.hasOwnProperty.call(insertedById, taskId)) return;
      const effective = getEffectiveTask(taskId);
      if (effective) {
        tasksById[taskId] = effective;
      }
    });

    const changedIds = new Set<string>();
    invalidatedTaskIdsRef.current.forEach((id) => changedIds.add(id));
    invalidatedTaskIdsRef.current.clear();
    Object.keys(insertedById).forEach((id) => changedIds.add(id));
    Object.keys(overridePatchesById).forEach((id) => changedIds.add(id));
    Object.keys(tombstones).forEach((id) => changedIds.add(id));

    const prev = derivedCacheRef.current;
    const needsFullRebuild =
      !prev || prev.projectId !== projectId || changedIds.size === 0;

    const rebuild = () => {
      const tasks: TaskWithAttemptStatus[] = [];
      const insertIds = new Set(Object.keys(insertedById));
      Object.keys(insertedById).forEach((id) => {
        const effective = getEffectiveTask(id);
        if (effective) tasks.push(effective);
      });
      Object.entries(streamTasksById).forEach(([id]) => {
        if (insertIds.has(id)) return;
        const effective = getEffectiveTask(id);
        if (effective) tasks.push(effective);
      });

      const baseCache = buildTaskDerivationCache(tasks);
      const nextCache: TasksDerivationCache = { projectId, ...baseCache };
      derivedCacheRef.current = nextCache;
      return nextCache;
    };

    if (needsFullRebuild) {
      const nextCache = rebuild();
      return {
        tasks: nextCache.tasks,
        tasksById,
        tasksByStatus: nextCache.tasksByStatus,
      };
    }

    const cache = prev;
    const applied = applyTaskDerivationChanges(
      cache,
      changedIds,
      getEffectiveTask
    );
    if (!applied) {
      const nextCache = rebuild();
      return {
        tasks: nextCache.tasks,
        tasksById,
        tasksByStatus: nextCache.tasksByStatus,
      };
    }

    return {
      tasks: cache.tasks,
      tasksById,
      tasksByStatus: cache.tasksByStatus,
    };
  }, [data, inserts, overrides, projectId, streamTasksById, tombstones]);

  const isLoading = !!projectId && !data && !error; // until first snapshot

  useEffect(() => {
    // Drop optimistic inserts once the stream contains the canonical task.
    Object.keys(inserts).forEach((taskId) => {
      if (streamTasksById[taskId]) {
        clearInsert(taskId);
      }
    });

    // Drop tombstones once the stream has removed the task.
    Object.keys(tombstones).forEach((taskId) => {
      if (!streamTasksById[taskId]) {
        clearTombstone(taskId);
      }
    });

    // Drop overrides once the stream matches the patched fields.
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
    if (!connectEnabled) return;

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
      if (meta.lastResyncAt && now - meta.lastResyncAt < minResyncGapMs)
        return false;
      return true;
    };

    const candidates: string[] = [];

    Object.values(inserts).forEach(({ task, meta }) => {
      if (task.project_id !== projectId) return;
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
    connectEnabled,
    inserts,
    markResyncAttempt,
    overrides,
    optimisticStaleTick,
    projectId,
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
