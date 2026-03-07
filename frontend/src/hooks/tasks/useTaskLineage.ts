import { useQuery } from '@tanstack/react-query';
import { tasksApi } from '@/lib/api';
import type { TaskLineageSummary } from 'shared/types';

export const taskLineageKeys = {
  all: ['taskLineage'] as const,
  byTask: (taskId: string | undefined) => ['taskLineage', taskId] as const,
};

type Options = {
  enabled?: boolean;
};

export function useTaskLineage(taskId?: string, opts?: Options) {
  const enabled = (opts?.enabled ?? true) && !!taskId;

  return useQuery<TaskLineageSummary>({
    queryKey: taskLineageKeys.byTask(taskId),
    queryFn: () => tasksApi.getLineage(taskId!),
    enabled,
    staleTime: 10_000,
  });
}
