import { useCallback, useEffect, useState } from 'react';
import type { Operation } from 'rfc6902';
import { useJsonPatchWsStream } from './useJsonPatchWsStream';
import { normalizeIdMapPatches } from './jsonPatchUtils';
import type { ExecutionProcess } from 'shared/types';

type ExecutionProcessState = {
  execution_processes: Record<string, ExecutionProcess>;
};

interface UseExecutionProcessesResult {
  executionProcesses: ExecutionProcess[];
  executionProcessesById: Record<string, ExecutionProcess>;
  isAttemptRunning: boolean;
  isLoading: boolean;
  isConnected: boolean;
  error: string | null;
}

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

  const initialData = useCallback(
    (): ExecutionProcessState => ({ execution_processes: {} }),
    []
  );
  const deduplicatePatches = useCallback(
    (patches: Operation[], current: ExecutionProcessState | undefined) =>
      normalizeIdMapPatches(
        patches,
        current?.execution_processes,
        '/execution_processes/'
      ),
    []
  );

  const { data, isConnected, error } =
    useJsonPatchWsStream<ExecutionProcessState>(
      endpoint,
      connectEnabled,
      initialData,
      { deduplicatePatches }
    );

  const executionProcessesById = data?.execution_processes ?? {};
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
    error,
  };
};
