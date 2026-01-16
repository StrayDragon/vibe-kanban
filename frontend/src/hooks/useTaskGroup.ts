import { useQuery } from '@tanstack/react-query';
import { taskGroupsApi } from '@/lib/api';
import type { TaskGroup } from '@/types/task-group';

export const taskGroupKeys = {
  byId: (taskGroupId: string | undefined) => ['taskGroups', taskGroupId],
};

type Options = {
  enabled?: boolean;
};

export function useTaskGroup(taskGroupId?: string, opts?: Options) {
  const enabled = (opts?.enabled ?? true) && !!taskGroupId;

  return useQuery<TaskGroup>({
    queryKey: taskGroupKeys.byId(taskGroupId),
    queryFn: () => taskGroupsApi.getById(taskGroupId!),
    enabled,
  });
}
