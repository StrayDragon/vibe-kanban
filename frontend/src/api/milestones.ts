import type {
  CreateMilestone,
  Milestone,
  RunNextMilestoneStepResponse,
  UpdateMilestone,
} from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const milestonesApi = {
  create: async (data: CreateMilestone): Promise<Milestone> => {
    const response = await makeRequest(`/api/milestones`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Milestone>(response);
  },

  getById: async (milestoneId: string): Promise<Milestone> => {
    const response = await makeRequest(`/api/milestones/${milestoneId}`);
    return handleApiResponse<Milestone>(response);
  },

  update: async (milestoneId: string, data: UpdateMilestone): Promise<Milestone> => {
    const response = await makeRequest(`/api/milestones/${milestoneId}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Milestone>(response);
  },

  runNextStep: async (
    milestoneId: string
  ): Promise<RunNextMilestoneStepResponse> => {
    const response = await makeRequest(
      `/api/milestones/${milestoneId}/run-next-step`,
      {
        method: 'POST',
      }
    );
    return handleApiResponse<RunNextMilestoneStepResponse>(response);
  },
};
