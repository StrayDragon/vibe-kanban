import type { QueryClient } from '@tanstack/react-query';
import type { Operation } from 'rfc6902';
import { branchStatusKeys } from '@/hooks/task-attempts/useBranchStatus';
import { collectInvalidations } from '@/utils/eventInvalidation';
import { taskAttemptKeys } from '@/query-keys/taskAttemptKeys';

type InvalidatableQueryClient = Pick<QueryClient, 'invalidateQueries'>;

export function invalidateQueriesFromJsonPatch(
  queryClient: InvalidatableQueryClient,
  patch: Operation[]
) {
  const { taskIds, workspaceIds, hasExecutionProcess } =
    collectInvalidations(patch);

  for (const taskId of taskIds) {
    queryClient.invalidateQueries({
      queryKey: taskAttemptKeys.byTask(taskId),
    });
    queryClient.invalidateQueries({
      queryKey: taskAttemptKeys.byTaskWithSessions(taskId),
    });
  }

  for (const workspaceId of workspaceIds) {
    queryClient.invalidateQueries({
      queryKey: branchStatusKeys.byAttempt(workspaceId),
    });
    queryClient.invalidateQueries({
      queryKey: taskAttemptKeys.attempt(workspaceId),
    });
    queryClient.invalidateQueries({
      queryKey: taskAttemptKeys.attemptWithSession(workspaceId),
    });
  }

  if (hasExecutionProcess) {
    queryClient.invalidateQueries({
      queryKey: branchStatusKeys.all,
    });
  }
}
