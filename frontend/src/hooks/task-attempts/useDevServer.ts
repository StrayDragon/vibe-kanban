import { useMemo } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { attemptsApi, executionProcessesApi } from '@/lib/api';
import { useAttemptExecution } from '@/hooks/task-attempts/useAttemptExecution';
import type { ExecutionProcessPublic as ExecutionProcess } from 'shared/types';
import { executionProcessKeys } from '@/query-keys/executionProcessKeys';

interface UseDevServerOptions {
  onStartSuccess?: () => void;
  onStartError?: (err: unknown) => void;
  onStopSuccess?: () => void;
  onStopError?: (err: unknown) => void;
}

export function useDevServer(
  attemptId: string | undefined,
  options?: UseDevServerOptions
) {
  const queryClient = useQueryClient();
  const { attemptData } = useAttemptExecution(attemptId);

  // Find running dev server process
  const runningDevServer = useMemo<ExecutionProcess | undefined>(() => {
    return attemptData.processes.find(
      (process) =>
        process.run_reason === 'devserver' && process.status === 'running'
    );
  }, [attemptData.processes]);

  // Find latest dev server process (for logs viewing)
  const latestDevServerProcess = useMemo<ExecutionProcess | undefined>(() => {
    return [...attemptData.processes]
      .filter((process) => process.run_reason === 'devserver')
      .sort(
        (a, b) =>
          new Date(b.started_at).getTime() - new Date(a.started_at).getTime()
      )[0];
  }, [attemptData.processes]);

  // Start mutation
  const startMutation = useMutation({
    mutationKey: ['startDevServer', attemptId],
    mutationFn: async () => {
      if (!attemptId) return;
      await attemptsApi.startDevServer(attemptId);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: executionProcessKeys.byAttempt(attemptId),
      });
      options?.onStartSuccess?.();
    },
    onError: (err) => {
      console.error('Failed to start dev server:', err);
      options?.onStartError?.(err);
    },
  });

  // Stop mutation
  const stopMutation = useMutation({
    mutationKey: ['stopDevServer', runningDevServer?.id],
    mutationFn: async () => {
      if (!runningDevServer) return;
      await executionProcessesApi.stopExecutionProcess(runningDevServer.id);
    },
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: executionProcessKeys.byAttempt(attemptId),
        }),
        runningDevServer
          ? queryClient.invalidateQueries({
              queryKey: executionProcessKeys.details(runningDevServer.id),
            })
          : Promise.resolve(),
      ]);
      options?.onStopSuccess?.();
    },
    onError: (err) => {
      console.error('Failed to stop dev server:', err);
      options?.onStopError?.(err);
    },
  });

  return {
    start: startMutation.mutate,
    stop: stopMutation.mutate,
    isStarting: startMutation.isPending,
    isStopping: stopMutation.isPending,
    runningDevServer,
    latestDevServerProcess,
  };
}
