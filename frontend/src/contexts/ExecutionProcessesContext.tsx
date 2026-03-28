import React, { createContext, useContext, useMemo } from 'react';
import { useExecutionProcesses } from '@/hooks/execution-processes/useExecutionProcesses';
import type { ExecutionProcessPublic as ExecutionProcess } from 'shared/types';

type ExecutionProcessesContextType = {
  executionProcessesAll: ExecutionProcess[];
  executionProcessesByIdAll: Record<string, ExecutionProcess>;
  isAttemptRunningAll: boolean;

  executionProcessesVisible: ExecutionProcess[];
  executionProcessesVisibleSorted: ExecutionProcess[];
  executionProcessesByIdVisible: Record<string, ExecutionProcess>;
  isAttemptRunningVisible: boolean;

  isLoading: boolean;
  isConnected: boolean;
  isResyncing: boolean;
  error: string | null;
  resync: (reason?: string) => void;
};

const ExecutionProcessesContext =
  createContext<ExecutionProcessesContextType | null>(null);

export const ExecutionProcessesProvider: React.FC<{
  attemptId: string | undefined;
  children: React.ReactNode;
}> = ({ attemptId, children }) => {
  const {
    executionProcesses,
    executionProcessesById,
    isAttemptRunning,
    isLoading,
    isConnected,
    isResyncing,
    error,
    resync,
  } = useExecutionProcesses(attemptId, { showSoftDeleted: true });

  const visible = useMemo(
    () => executionProcesses.filter((p) => !p.dropped),
    [executionProcesses]
  );

  const visibleSorted = useMemo(() => {
    const createdAtMsById = new Map<string, number>();
    const createdAtMs = (p: ExecutionProcess) => {
      const cached = createdAtMsById.get(p.id);
      if (typeof cached === 'number') return cached;
      const parsed = Date.parse(p.created_at as unknown as string);
      const ms = Number.isFinite(parsed) ? parsed : 0;
      createdAtMsById.set(p.id, ms);
      return ms;
    };

    return visible
      .slice()
      .sort(
        (a, b) => createdAtMs(a) - createdAtMs(b) || a.id.localeCompare(b.id)
      );
  }, [visible]);

  const executionProcessesByIdVisible = useMemo(() => {
    const m: Record<string, ExecutionProcess> = {};
    for (const p of visible) m[p.id] = p;
    return m;
  }, [visible]);

  const isAttemptRunningVisible = useMemo(
    () =>
      visible.some(
        (process) =>
          (process.run_reason === 'codingagent' ||
            process.run_reason === 'setupscript' ||
            process.run_reason === 'cleanupscript') &&
          process.status === 'running'
      ),
    [visible]
  );

  const value = useMemo<ExecutionProcessesContextType>(
    () => ({
      executionProcessesAll: executionProcesses,
      executionProcessesByIdAll: executionProcessesById,
      isAttemptRunningAll: isAttemptRunning,
      executionProcessesVisible: visible,
      executionProcessesVisibleSorted: visibleSorted,
      executionProcessesByIdVisible,
      isAttemptRunningVisible,
      isLoading,
      isConnected,
      isResyncing,
      error,
      resync,
    }),
    [
      executionProcesses,
      executionProcessesById,
      isAttemptRunning,
      visible,
      visibleSorted,
      executionProcessesByIdVisible,
      isAttemptRunningVisible,
      isLoading,
      isConnected,
      isResyncing,
      error,
      resync,
    ]
  );

  return (
    <ExecutionProcessesContext.Provider value={value}>
      {children}
    </ExecutionProcessesContext.Provider>
  );
};

export const useExecutionProcessesContext = () => {
  const ctx = useContext(ExecutionProcessesContext);
  if (!ctx) {
    throw new Error(
      'useExecutionProcessesContext must be used within ExecutionProcessesProvider'
    );
  }
  return ctx;
};
