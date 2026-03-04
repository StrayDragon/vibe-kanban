import { useCallback, useEffect, useMemo, useState } from 'react';
import type { Operation } from 'rfc6902';
import { useJsonPatchWsStream } from '@/hooks/useJsonPatchWsStream';
import { normalizeIdMapPatches } from '@/hooks/jsonPatchUtils';
import type { TaskStatus, TaskWithAttemptStatus } from 'shared/types';

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

  const { data, isConnected, error } = useJsonPatchWsStream<TasksState>(
    endpoint,
    connectEnabled,
    initialData,
    { deduplicatePatches }
  );

  const localTasksById = useMemo(() => data?.tasks ?? {}, [data?.tasks]);

  const { tasks, tasksById, tasksByStatus } = useMemo(() => {
    const merged: Record<string, TaskWithAttemptStatus> = { ...localTasksById };
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
  }, [localTasksById]);

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

