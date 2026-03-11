import type {
  CreateMilestone,
  Milestone,
  MilestonePlanApplyResponse,
  MilestonePlanPreviewResponse,
  MilestonePlanV1,
  PushMilestoneBaselineRequest,
  PushMilestoneBaselineResponse,
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

  update: async (
    milestoneId: string,
    data: UpdateMilestone
  ): Promise<Milestone> => {
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

  pushBaselineBranch: async (
    milestoneId: string,
    data: PushMilestoneBaselineRequest
  ): Promise<PushMilestoneBaselineResponse> => {
    const response = await makeRequest(
      `/api/milestones/${milestoneId}/push-baseline-branch`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    );
    return handleApiResponse<PushMilestoneBaselineResponse>(response);
  },

  previewPlan: async (
    milestoneId: string,
    plan: MilestonePlanV1
  ): Promise<MilestonePlanPreviewResponse> => {
    const response = await makeRequest(`/api/milestones/${milestoneId}/plan/preview`, {
      method: 'POST',
      body: JSON.stringify(plan),
    });
    return handleApiResponse<MilestonePlanPreviewResponse>(response);
  },

  applyPlan: async (
    milestoneId: string,
    plan: MilestonePlanV1,
    options?: { idempotencyKey?: string }
  ): Promise<MilestonePlanApplyResponse> => {
    const headers: Record<string, string> = {};
    if (options?.idempotencyKey) {
      headers['Idempotency-Key'] = options.idempotencyKey;
    }
    const response = await makeRequest(`/api/milestones/${milestoneId}/plan/apply`, {
      method: 'POST',
      headers,
      body: JSON.stringify(plan),
    });
    return handleApiResponse<MilestonePlanApplyResponse>(response);
  },
};
