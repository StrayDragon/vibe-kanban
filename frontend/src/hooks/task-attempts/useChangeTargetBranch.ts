import { useMutation, useQueryClient } from '@tanstack/react-query';
import { attemptsApi } from '@/lib/api';
import type {
  ChangeTargetBranchRequest,
  ChangeTargetBranchResponse,
} from 'shared/types';
import { repoBranchKeys } from './useRepoBranches';
import { branchStatusKeys } from './useBranchStatus';
import { taskAttemptKeys } from '@/query-keys/taskAttemptKeys';

type ChangeTargetBranchParams = {
  newTargetBranch: string;
  repoId: string;
};

export function useChangeTargetBranch(
  attemptId: string | undefined,
  repoId: string | undefined,
  onSuccess?: (data: ChangeTargetBranchResponse) => void,
  onError?: (err: unknown) => void
) {
  const queryClient = useQueryClient();

  return useMutation<
    ChangeTargetBranchResponse,
    unknown,
    ChangeTargetBranchParams
  >({
    mutationFn: async ({ newTargetBranch, repoId }) => {
      if (!attemptId) {
        throw new Error('Attempt id is not set');
      }

      const payload: ChangeTargetBranchRequest = {
        new_target_branch: newTargetBranch,
        repo_id: repoId,
      };
      return attemptsApi.change_target_branch(attemptId, payload);
    },
    onSuccess: (data) => {
      if (attemptId) {
        queryClient.invalidateQueries({
          queryKey: branchStatusKeys.byAttempt(attemptId),
        });
        // Invalidate taskAttempt query to refresh attempt.target_branch
        queryClient.invalidateQueries({
          queryKey: taskAttemptKeys.attempt(attemptId),
        });
      }

      if (repoId) {
        queryClient.invalidateQueries({
          queryKey: repoBranchKeys.byRepo(repoId),
        });
      }

      onSuccess?.(data);
    },
    onError: (err) => {
      console.error('Failed to change target branch:', err);
      if (attemptId) {
        queryClient.invalidateQueries({
          queryKey: branchStatusKeys.byAttempt(attemptId),
        });
      }
      onError?.(err);
    },
  });
}
