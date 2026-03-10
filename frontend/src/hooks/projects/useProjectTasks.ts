import { useCallback, useEffect, useMemo, useState } from 'react';
import type { Operation } from 'rfc6902';
import { useJsonPatchWsStream } from '../useJsonPatchWsStream';
import { normalizeIdMapPatches } from '../jsonPatchUtils';
import type { TaskStatus, TaskWithAttemptStatus } from 'shared/types';
import { useOptimisticTasksStore } from '@/stores/useOptimisticTasksStore';

type TasksState = {
  tasks: Record<string, TaskWithAttemptStatus>;
};

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

  useEffect(() => {
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

  const { data, isConnected, isResyncing, error, resync } =
    useJsonPatchWsStream(endpoint, connectEnabled, initialData, {
      deduplicatePatches,
    });

  const {
    inserts,
    overrides,
    tombstones,
    clearInsert,
    clearOverride,
    clearTombstone,
    markResyncAttempt,
  } = useOptimisticTasksStore((state) => ({
    inserts: state.inserts,
    overrides: state.overrides,
    tombstones: state.tombstones,
    clearInsert: state.clearInsert,
    clearOverride: state.clearOverride,
    clearTombstone: state.clearTombstone,
    markResyncAttempt: state.markResyncAttempt,
  }));

  const streamTasksById = useMemo(() => data?.tasks ?? {}, [data?.tasks]);

  const mergedTasksById = useMemo(() => {
    const merged: Record<string, TaskWithAttemptStatus> = {
      ...streamTasksById,
    };

    Object.values(inserts).forEach(({ task }) => {
      if (task.project_id !== projectId) return;
      if (task.archived_kanban_id) return;
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
  }, [inserts, overrides, projectId, streamTasksById, tombstones]);

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
      }
    });
  }, [
    clearInsert,
    clearOverride,
    clearTombstone,
    inserts,
    overrides,
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
