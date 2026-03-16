import { useQuery } from '@tanstack/react-query';
import { ApiError, attemptsApi } from '@/lib/api';
import { useSsePollingInterval } from '@/hooks/utils/useSsePollingInterval';

export const branchStatusKeys = {
  all: ['branchStatus'] as const,
  byAttempt: (attemptId: string | undefined) =>
    ['branchStatus', attemptId] as const,
};

export function useBranchStatus(attemptId?: string) {
  const refetchInterval = useSsePollingInterval(5000);

  return useQuery({
    queryKey: branchStatusKeys.byAttempt(attemptId),
    queryFn: () => attemptsApi.getBranchStatus(attemptId!),
    enabled: !!attemptId,
    retry: (failureCount, error) => {
      if (error instanceof ApiError && error.statusCode === 404) return false;
      return failureCount < 2;
    },
    refetchInterval: (query) => {
      const err = query.state.error;
      if (err instanceof ApiError && err.statusCode === 404) {
        return false;
      }
      return refetchInterval;
    },
  });
}
