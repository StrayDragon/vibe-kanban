import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { Operation } from 'rfc6902';
import { useJsonPatchWsStream } from '@/hooks/useJsonPatchWsStream';
import { normalizeIdMapPatches } from '@/hooks/jsonPatchUtils';
import type { TaskStatus, TaskWithAttemptStatus } from 'shared/types';
import {
  applyTaskDerivationChanges,
  buildTaskDerivationCache,
  EMPTY_TASKS_BY_STATUS,
  type TaskDerivationCache,
} from '../tasks/taskDerivation';

type TasksState = {
  tasks: Record<string, TaskWithAttemptStatus>;
};

export interface UseArchivedKanbanTasksResult {
  tasks: TaskWithAttemptStatus[];
  tasksById: Record<string, TaskWithAttemptStatus>;
  tasksByStatus: Record<TaskStatus, TaskWithAttemptStatus[]>;
  isLoading: boolean;
  isConnected: boolean;
  error: string | null;
}

export const useArchivedKanbanTasks = (
  archiveId: string
): UseArchivedKanbanTasksResult => {
  const endpoint = `/api/tasks/stream/ws?archived_kanban_id=${encodeURIComponent(
    archiveId
  )}&include_archived=true`;
  const [connectEnabled, setConnectEnabled] = useState(false);
  const invalidatedTaskIdsRef = useRef<Set<string>>(new Set());
  const derivedCacheRef = useRef<TaskDerivationCache | null>(null);

  useEffect(() => {
    setConnectEnabled(false);
    if (!archiveId) return;
    const timer = window.setTimeout(() => setConnectEnabled(true), 200);
    return () => window.clearTimeout(timer);
  }, [archiveId]);

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

  const { data, isConnected, error } = useJsonPatchWsStream<TasksState>(
    endpoint,
    connectEnabled,
    initialData,
    { deduplicatePatches, onInvalidate }
  );

  const tasksById = useMemo(() => data?.tasks ?? {}, [data?.tasks]);

  const { tasks, tasksByStatus } = useMemo(() => {
    if (!data) {
      derivedCacheRef.current = null;
      return { tasks: [], tasksByStatus: EMPTY_TASKS_BY_STATUS };
    }

    const changedIds = new Set<string>();
    invalidatedTaskIdsRef.current.forEach((id) => changedIds.add(id));
    invalidatedTaskIdsRef.current.clear();

    const prev = derivedCacheRef.current;
    const needsFullRebuild = !prev || changedIds.size === 0;

    const rebuild = () => {
      const baseCache = buildTaskDerivationCache(Object.values(tasksById));
      derivedCacheRef.current = baseCache;
      return baseCache;
    };

    if (needsFullRebuild) {
      const nextCache = rebuild();
      return { tasks: nextCache.tasks, tasksByStatus: nextCache.tasksByStatus };
    }

    const applied = applyTaskDerivationChanges(
      prev,
      changedIds,
      (id) => tasksById[id] ?? null
    );

    if (!applied) {
      const nextCache = rebuild();
      return { tasks: nextCache.tasks, tasksByStatus: nextCache.tasksByStatus };
    }

    return { tasks: prev.tasks, tasksByStatus: prev.tasksByStatus };
  }, [data, tasksById]);

  const isLoading = !!archiveId && !data && !error;

  return {
    tasks,
    tasksById,
    tasksByStatus,
    isLoading,
    isConnected,
    error,
  };
};
