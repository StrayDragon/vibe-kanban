import { useQuery } from '@tanstack/react-query';
import { attemptsApi } from '@/lib/api';
import type { Workspace } from 'shared/types';
import type { WorkspaceWithSession } from '@/types/attempt';
import { useSsePollingInterval } from '@/hooks/useSsePollingInterval';

export const taskAttemptKeys = {
  all: ['taskAttempts'] as const,
  byTask: (taskId: string | undefined) => ['taskAttempts', taskId] as const,
  byTaskWithSessions: (taskId: string | undefined) =>
    ['taskAttemptsWithSessions', taskId] as const,
};

type Options = {
  enabled?: boolean;
  refetchInterval?: number | false;
};

export function useTaskAttempts(taskId?: string, opts?: Options) {
  const enabled = (opts?.enabled ?? true) && !!taskId;
  const fallbackInterval = opts?.refetchInterval ?? 5000;
  const sseFallbackInterval =
    fallbackInterval === false ? 5000 : fallbackInterval;
  const sseInterval = useSsePollingInterval(sseFallbackInterval);
  const refetchInterval = fallbackInterval === false ? false : sseInterval;

  return useQuery<Workspace[]>({
    queryKey: taskAttemptKeys.byTask(taskId),
    queryFn: () => attemptsApi.getAll(taskId!),
    enabled,
    refetchInterval,
  });
}

/**
 * Hook for components that need session data for all attempts.
 * Fetches all attempts and their sessions in parallel.
 */
export function useTaskAttemptsWithSessions(taskId?: string, opts?: Options) {
  const enabled = (opts?.enabled ?? true) && !!taskId;
  const fallbackInterval = opts?.refetchInterval ?? 5000;
  const sseFallbackInterval =
    fallbackInterval === false ? 5000 : fallbackInterval;
  const sseInterval = useSsePollingInterval(sseFallbackInterval);
  const refetchInterval = fallbackInterval === false ? false : sseInterval;

  return useQuery<WorkspaceWithSession[]>({
    queryKey: taskAttemptKeys.byTaskWithSessions(taskId),
    queryFn: () => attemptsApi.getAllWithSessions(taskId!),
    enabled,
    refetchInterval,
  });
}
