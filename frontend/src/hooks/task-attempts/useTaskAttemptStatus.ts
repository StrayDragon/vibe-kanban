import { useQuery } from '@tanstack/react-query';
import { attemptsApi } from '@/lib/api';
import type { TaskAttemptStatusResponse } from 'shared/types';

export function useTaskAttemptStatus(attemptId?: string) {
  return useQuery<TaskAttemptStatusResponse>({
    queryKey: ['taskAttemptStatus', attemptId],
    queryFn: () => attemptsApi.getStatus(attemptId!),
    enabled: !!attemptId,
    refetchInterval: 5000,
  });
}
