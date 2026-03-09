import type { CreateTaskGroup, RunNextMilestoneStepResponse } from 'shared/types';
import type { TaskGroup, UpdateTaskGroup } from '@/types/task-group';

import { handleApiResponse, makeRequest } from './client';

export const taskGroupsApi = {
  create: async (data: CreateTaskGroup): Promise<TaskGroup> => {
    const response = await makeRequest(`/api/task-groups`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<TaskGroup>(response);
  },

  getById: async (taskGroupId: string): Promise<TaskGroup> => {
    const response = await makeRequest(`/api/task-groups/${taskGroupId}`);
    return handleApiResponse<TaskGroup>(response);
  },

  update: async (
    taskGroupId: string,
    data: UpdateTaskGroup
  ): Promise<TaskGroup> => {
    const response = await makeRequest(`/api/task-groups/${taskGroupId}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
    return handleApiResponse<TaskGroup>(response);
  },

  runNextStep: async (
    taskGroupId: string
  ): Promise<RunNextMilestoneStepResponse> => {
    const response = await makeRequest(
      `/api/task-groups/${taskGroupId}/run-next-step`,
      {
        method: 'POST',
      }
    );
    return handleApiResponse<RunNextMilestoneStepResponse>(response);
  },
};
