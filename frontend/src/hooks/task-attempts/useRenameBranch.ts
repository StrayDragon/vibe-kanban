import { useMutation, useQueryClient } from '@tanstack/react-query';
import { attemptsApi } from '@/lib/api';
import { branchStatusKeys } from './useBranchStatus';
import { taskAttemptKeys } from '@/query-keys/taskAttemptKeys';

export function useRenameBranch(
  attemptId?: string,
  onSuccess?: (newBranchName: string) => void,
  onError?: (err: unknown) => void
) {
  const queryClient = useQueryClient();

  return useMutation<{ branch: string }, unknown, string>({
    mutationFn: async (newBranchName) => {
      if (!attemptId) throw new Error('Attempt id is not set');
      return attemptsApi.renameBranch(attemptId, newBranchName);
    },
    onSuccess: (data) => {
      if (attemptId) {
        queryClient.invalidateQueries({
          queryKey: taskAttemptKeys.attempt(attemptId),
        });
        queryClient.invalidateQueries({
          queryKey: taskAttemptKeys.branch(attemptId),
        });
        queryClient.invalidateQueries({
          queryKey: branchStatusKeys.byAttempt(attemptId),
        });
        queryClient.invalidateQueries({
          queryKey: taskAttemptKeys.all,
        });
      }
      onSuccess?.(data.branch);
    },
    onError: (err) => {
      console.error('Failed to rename branch:', err);
      if (attemptId) {
        queryClient.invalidateQueries({
          queryKey: branchStatusKeys.byAttempt(attemptId),
        });
      }
      onError?.(err);
    },
  });
}
