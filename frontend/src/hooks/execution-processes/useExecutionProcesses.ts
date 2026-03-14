import { useCallback, useEffect, useMemo, useState } from 'react';
import type { ExecutionProcess } from 'shared/types';
import { useIdMapWsStream } from '@/realtime';
import { useOptimisticExecutionProcessesStore } from '@/stores/useOptimisticExecutionProcessesStore';

interface UseExecutionProcessesResult {
  executionProcesses: ExecutionProcess[];
  executionProcessesById: Record<string, ExecutionProcess>;
  isAttemptRunning: boolean;
  isLoading: boolean;
  isConnected: boolean;
  isResyncing: boolean;
  error: string | null;
  resync: (reason?: string) => void;
}

const EMPTY_EXECUTION_PROCESSES_BY_ID: Record<string, ExecutionProcess> = {};

/**
 * Stream execution processes for a task attempt via WebSocket (JSON Patch) and expose as array + map.
 * Server sends initial snapshot: replace /execution_processes with an object keyed by id.
 * Live updates arrive at /execution_processes/<id> via add/replace/remove operations.
 */
export const useExecutionProcesses = (
  taskAttemptId: string | undefined,
  opts?: { showSoftDeleted?: boolean }
): UseExecutionProcessesResult => {
  const showSoftDeleted = opts?.showSoftDeleted;
  const [connectEnabled, setConnectEnabled] = useState(false);
  let endpoint: string | undefined;

  if (taskAttemptId) {
    const params = new URLSearchParams({ workspace_id: taskAttemptId });
    if (typeof showSoftDeleted === 'boolean') {
      params.set('show_soft_deleted', String(showSoftDeleted));
    }
    endpoint = `/api/execution-processes/stream/ws?${params.toString()}`;
  }

  useEffect(() => {
    setConnectEnabled(false);
    if (!taskAttemptId) return;
    const timer = window.setTimeout(() => setConnectEnabled(true), 200);
    return () => window.clearTimeout(timer);
  }, [taskAttemptId, showSoftDeleted]);

  const { data, isConnected, isResyncing, error, resync } =
    useIdMapWsStream<'execution_processes', ExecutionProcess>(
      endpoint,
      connectEnabled,
      'execution_processes',
      '/execution_processes/'
    );

  const streamById = useMemo(
    () => data?.execution_processes ?? {},
    [data?.execution_processes]
  );
  const optimisticById = useOptimisticExecutionProcessesStore(
    useCallback(
      (state) =>
        taskAttemptId
          ? (state.byAttemptId[taskAttemptId] ??
              EMPTY_EXECUTION_PROCESSES_BY_ID)
          : EMPTY_EXECUTION_PROCESSES_BY_ID,
      [taskAttemptId]
    )
  );
  const removeManyOptimistic = useOptimisticExecutionProcessesStore(
    (state) => state.removeMany
  );

  useEffect(() => {
    if (!taskAttemptId) return;
    const optimisticIds = Object.keys(optimisticById);
    if (optimisticIds.length === 0) return;
    const present = optimisticIds.filter((id) => !!streamById[id]);
    if (present.length > 0) {
      removeManyOptimistic(taskAttemptId, present);
    }
  }, [optimisticById, removeManyOptimistic, streamById, taskAttemptId]);

  const executionProcessesById = taskAttemptId
    ? { ...optimisticById, ...streamById }
    : streamById;
  const executionProcesses = Object.values(executionProcessesById).sort(
    (a, b) =>
      new Date(a.created_at as unknown as string).getTime() -
      new Date(b.created_at as unknown as string).getTime()
  );
  const isAttemptRunning = executionProcesses.some(
    (process) =>
      (process.run_reason === 'codingagent' ||
        process.run_reason === 'setupscript' ||
        process.run_reason === 'cleanupscript') &&
      process.status === 'running'
  );
  const isLoading = !!taskAttemptId && !data && !error; // until first snapshot

  return {
    executionProcesses,
    executionProcessesById,
    isAttemptRunning,
    isLoading,
    isConnected,
    isResyncing,
    error,
    resync,
  };
};
