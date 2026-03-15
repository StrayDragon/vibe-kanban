import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigateWithSearch } from '@/hooks';
import { tasksApi } from '@/lib/api';
import { paths } from '@/lib/paths';
import { taskRelationshipsKeys } from '@/hooks/tasks/useTaskRelationships';
import { useTranslation } from 'react-i18next';
import { toast } from '@/components/ui/toast';
import { useOptimisticTasksStore } from '@/stores/useOptimisticTasksStore';
import type {
  CreateTask,
  CreateAndStartTaskRequest,
  Task,
  TaskWithAttemptStatus,
  UpdateTask,
} from 'shared/types';
import { taskKeys } from './useTask';
import { taskLineageKeys } from './useTaskLineage';

export function useTaskMutations(projectId?: string) {
  const queryClient = useQueryClient();
  const navigate = useNavigateWithSearch();
  const { t } = useTranslation(['tasks', 'common']);

  const invalidateQueries = (taskId?: string) => {
    queryClient.invalidateQueries({ queryKey: taskKeys.all });
    if (taskId) {
      queryClient.invalidateQueries({ queryKey: taskKeys.byId(taskId) });
    }
  };

  const createTask = useMutation({
    mutationFn: (data: CreateTask) => tasksApi.create(data),
    onSuccess: (createdTask: Task) => {
      invalidateQueries();
      // Invalidate parent's relationships cache if this is a subtask
      if (createdTask.parent_workspace_id) {
        queryClient.invalidateQueries({
          queryKey: taskRelationshipsKeys.byAttempt(
            createdTask.parent_workspace_id
          ),
        });
      }
      queryClient.invalidateQueries({
        queryKey: taskLineageKeys.byTask(createdTask.id),
      });
      if (createdTask.origin_task_id) {
        queryClient.invalidateQueries({
          queryKey: taskLineageKeys.byTask(createdTask.origin_task_id),
        });
      }
      if (projectId) {
        navigate(`${paths.task(projectId, createdTask.id)}/attempts/latest`);
      }

      // Populate optimistic stream state to guarantee a visible UI update even
      // if the task stream misses the create event.
      useOptimisticTasksStore.getState().insertTask({
        has_in_progress_attempt: false,
        last_attempt_failed: false,
        executor: '',
        dispatch_state: null,
        orchestration: null,
        ...createdTask,
      });

      // Best-effort: refresh with canonical attempt-status fields.
      void tasksApi
        .getById(createdTask.id)
        .then((full) => useOptimisticTasksStore.getState().insertTask(full))
        .catch(() => {});
    },
    onError: (err) => {
      toast({
        variant: 'destructive',
        title: t('tasks:errors.createFailed'),
        description: err instanceof Error ? err.message : undefined,
      });
      console.error('Failed to create task:', err);
    },
  });

  const createAndStart = useMutation({
    mutationFn: (data: CreateAndStartTaskRequest) =>
      tasksApi.createAndStart(data),
    onSuccess: (createdTask: TaskWithAttemptStatus) => {
      useOptimisticTasksStore.getState().insertTask(createdTask);
      invalidateQueries();
      // Invalidate parent's relationships cache if this is a subtask
      if (createdTask.parent_workspace_id) {
        queryClient.invalidateQueries({
          queryKey: taskRelationshipsKeys.byAttempt(
            createdTask.parent_workspace_id
          ),
        });
      }
      queryClient.invalidateQueries({
        queryKey: taskLineageKeys.byTask(createdTask.id),
      });
      if (createdTask.origin_task_id) {
        queryClient.invalidateQueries({
          queryKey: taskLineageKeys.byTask(createdTask.origin_task_id),
        });
      }
      if (projectId) {
        navigate(`${paths.task(projectId, createdTask.id)}/attempts/latest`);
      }
    },
    onError: (err) => {
      toast({
        variant: 'destructive',
        title: t('tasks:errors.createAndStartFailed'),
        description: err instanceof Error ? err.message : undefined,
      });
      console.error('Failed to create and start task:', err);
    },
  });

  const updateTask = useMutation({
    mutationFn: ({ taskId, data }: { taskId: string; data: UpdateTask }) =>
      tasksApi.update(taskId, data),
    onMutate: async ({ taskId, data }) => {
      const store = useOptimisticTasksStore.getState();
      const snapshot = store.getSnapshot(taskId);
      const patch: Partial<TaskWithAttemptStatus> = {};

      if (typeof data.title === 'string') patch.title = data.title;
      if (typeof data.description === 'string' || data.description === null) {
        patch.description = data.description;
      }
      if (typeof data.status === 'string') patch.status = data.status;
      if (
        typeof data.parent_workspace_id === 'string' ||
        data.parent_workspace_id === null
      ) {
        patch.parent_workspace_id = data.parent_workspace_id;
      }

      if (Object.keys(patch).length > 0) {
        store.setOverride(taskId, patch);
      }

      return { snapshot };
    },
    onSuccess: (updatedTask: Task) => {
      invalidateQueries(updatedTask.id);
      queryClient.invalidateQueries({
        queryKey: taskLineageKeys.byTask(updatedTask.id),
      });
      if (updatedTask.origin_task_id) {
        queryClient.invalidateQueries({
          queryKey: taskLineageKeys.byTask(updatedTask.origin_task_id),
        });
      }
    },
    onError: (err, variables, context) => {
      if (context?.snapshot) {
        useOptimisticTasksStore
          .getState()
          .restoreSnapshot(variables.taskId, context.snapshot);
      }
      toast({
        variant: 'destructive',
        title: t('tasks:errors.updateFailed'),
        description: err instanceof Error ? err.message : undefined,
      });
      console.error('Failed to update task:', err);
    },
  });

  const deleteTask = useMutation({
    mutationFn: (taskId: string) => tasksApi.delete(taskId),
    onMutate: async (taskId) => {
      const store = useOptimisticTasksStore.getState();
      const snapshot = store.getSnapshot(taskId);
      store.tombstoneTask(taskId);
      return { snapshot };
    },
    onSuccess: (_: unknown, taskId: string) => {
      invalidateQueries(taskId);
      // Remove single-task cache entry to avoid stale data flashes
      queryClient.removeQueries({
        queryKey: taskKeys.byId(taskId),
        exact: true,
      });
      // Invalidate all task relationships caches (safe approach since we don't know parent)
      queryClient.invalidateQueries({ queryKey: taskRelationshipsKeys.all });
    },
    onError: (err, taskId, context) => {
      if (context?.snapshot) {
        useOptimisticTasksStore
          .getState()
          .restoreSnapshot(taskId, context.snapshot);
      }
      toast({
        variant: 'destructive',
        title: t('tasks:errors.deleteFailed'),
        description: err instanceof Error ? err.message : undefined,
      });
      console.error('Failed to delete task:', err);
    },
  });

  return {
    createTask,
    createAndStart,
    updateTask,
    deleteTask,
  };
}
