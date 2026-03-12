import type {
  CreateAndStartTaskRequest,
  CreateTask,
  Task,
  TaskLineageSummary,
  TaskWithAttemptStatus,
  UpdateTask,
} from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const tasksApi = {
  getAll: async (options?: {
    projectId?: string;
    includeArchived?: boolean;
    archivedKanbanId?: string;
  }): Promise<TaskWithAttemptStatus[]> => {
    const params = new URLSearchParams();
    if (options?.projectId) params.set('project_id', options.projectId);
    if (options?.includeArchived) params.set('include_archived', 'true');
    if (options?.archivedKanbanId)
      params.set('archived_kanban_id', options.archivedKanbanId);

    const query = params.toString();
    const response = await makeRequest(`/api/tasks${query ? `?${query}` : ''}`);
    return handleApiResponse<TaskWithAttemptStatus[]>(response);
  },

  getById: async (taskId: string): Promise<TaskWithAttemptStatus> => {
    const response = await makeRequest(`/api/tasks/${taskId}`);
    return handleApiResponse<TaskWithAttemptStatus>(response);
  },

  getLineage: async (taskId: string): Promise<TaskLineageSummary> => {
    const response = await makeRequest(`/api/tasks/${taskId}/lineage`);
    return handleApiResponse<TaskLineageSummary>(response);
  },

  create: async (data: CreateTask): Promise<Task> => {
    const response = await makeRequest(`/api/tasks`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Task>(response);
  },

  createAndStart: async (
    data: CreateAndStartTaskRequest
  ): Promise<TaskWithAttemptStatus> => {
    const response = await makeRequest(`/api/tasks/create-and-start`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<TaskWithAttemptStatus>(response);
  },

  update: async (taskId: string, data: UpdateTask): Promise<Task> => {
    const response = await makeRequest(`/api/tasks/${taskId}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Task>(response);
  },

  delete: async (taskId: string): Promise<void> => {
    const response = await makeRequest(`/api/tasks/${taskId}`, {
      method: 'DELETE',
    });
    return handleApiResponse<void>(response);
  },
};
