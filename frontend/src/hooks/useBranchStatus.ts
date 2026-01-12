import { useQuery } from '@tanstack/react-query';
import { attemptsApi } from '@/lib/api';
import { useSsePollingInterval } from '@/hooks/useSsePollingInterval';

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
    refetchInterval,
  });
}
