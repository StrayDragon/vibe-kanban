import { useMutation, useQueryClient } from '@tanstack/react-query';
import { attemptsApi } from '@/lib/api';
import {
  createWorkspaceWithSession,
  type WorkspaceWithSession,
} from '@/types/attempt';
import { taskAttemptKeys } from '@/hooks/task-attempts/useTaskAttempts';
import { useOptimisticTasksStore } from '@/stores/useOptimisticTasksStore';
import { taskKeys } from '@/hooks/tasks/useTask';
import type {
  ExecutorProfileId,
  TaskAttemptPromptPreset,
  TaskWithAttemptStatus,
  WorkspaceRepoInput,
  Workspace,
} from 'shared/types';

type CreateAttemptArgs = {
  profile: ExecutorProfileId;
  repos: WorkspaceRepoInput[];
  promptPreset?: TaskAttemptPromptPreset | null;
};

type UseAttemptCreationArgs = {
  taskId: string;
  onSuccess?: (attempt: Workspace) => void;
};

export function useAttemptCreation({
  taskId,
  onSuccess,
}: UseAttemptCreationArgs) {
  const queryClient = useQueryClient();

  const mutation = useMutation({
    mutationFn: ({ profile, repos, promptPreset }: CreateAttemptArgs) =>
      attemptsApi.create({
        task_id: taskId,
        executor_profile_id: profile,
        repos,
        prompt_preset: promptPreset ?? null,
      }),
    onMutate: async () => {
      const store = useOptimisticTasksStore.getState();
      const snapshot = store.getSnapshot(taskId);

      // Optimistically move the task immediately so Kanban/overview updates even
      // if the task stream misses the server-side status patch.
      store.setOverride(taskId, { status: 'inprogress' });

      // Keep any single-task query caches consistent with the optimistic UI.
      queryClient.setQueryData<TaskWithAttemptStatus | undefined>(
        taskKeys.byId(taskId),
        (old) => (old ? { ...old, status: 'inprogress' } : old)
      );

      return { snapshot };
    },
    onSuccess: (newAttempt: Workspace) => {
      queryClient.setQueryData(
        taskAttemptKeys.byTask(taskId),
        (old: Workspace[] = []) => [newAttempt, ...old]
      );
      queryClient.setQueryData(
        taskAttemptKeys.byTaskWithSessions(taskId),
        (old: WorkspaceWithSession[] = []) => [
          createWorkspaceWithSession(newAttempt, undefined),
          ...old,
        ]
      );
      // Ensure the "with sessions" query eventually picks up the created session (and any server-side
      // normalization) even when SSE disables polling.
      queryClient.invalidateQueries({
        queryKey: taskAttemptKeys.byTaskWithSessions(taskId),
      });
      onSuccess?.(newAttempt);
    },
    onError: (_err, _vars, context) => {
      if (context?.snapshot) {
        useOptimisticTasksStore.getState().restoreSnapshot(taskId, context.snapshot);
      }
      queryClient.invalidateQueries({ queryKey: taskKeys.byId(taskId) });
    },
  });

  return {
    createAttempt: mutation.mutateAsync,
    isCreating: mutation.isPending,
    error: mutation.error,
  };
}
