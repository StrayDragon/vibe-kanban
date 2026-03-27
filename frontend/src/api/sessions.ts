import type {
  CreateFollowUpAttempt,
  ExecutionProcessPublic as ExecutionProcess,
  MilestonePlanDetectionResult,
  Session,
} from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export type SessionMessageTurn = {
  entry_index: number;
  turn_id: string;
  prompt: string | null;
  summary: string | null;
  created_at: string;
  updated_at: string;
};

export type SessionMessagesPage = {
  entries: SessionMessageTurn[];
  next_cursor: number | null;
  has_more: boolean;
};

export const sessionsApi = {
  getByWorkspace: async (workspaceId: string): Promise<Session[]> => {
    const response = await makeRequest(
      `/api/sessions?workspace_id=${workspaceId}`
    );
    return handleApiResponse<Session[]>(response);
  },

  getById: async (sessionId: string): Promise<Session> => {
    const response = await makeRequest(`/api/sessions/${sessionId}`);
    return handleApiResponse<Session>(response);
  },

  getMessages: async (
    sessionId: string,
    params?: { limit?: number; cursor?: number }
  ): Promise<SessionMessagesPage> => {
    const search = new URLSearchParams();
    if (params?.limit != null) search.set('limit', String(params.limit));
    if (params?.cursor != null) search.set('cursor', String(params.cursor));
    const suffix = search.toString() ? `?${search.toString()}` : '';
    const response = await makeRequest(
      `/api/sessions/${sessionId}/messages${suffix}`
    );
    return handleApiResponse<SessionMessagesPage>(response);
  },

  detectLatestMilestonePlan: async (
    sessionId: string
  ): Promise<MilestonePlanDetectionResult> => {
    const response = await makeRequest(
      `/api/sessions/${sessionId}/milestone-plan/latest`
    );
    return handleApiResponse<MilestonePlanDetectionResult>(response);
  },

  create: async (data: {
    workspace_id: string;
    executor?: string;
  }): Promise<Session> => {
    const response = await makeRequest('/api/sessions', {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Session>(response);
  },

  followUp: async (
    sessionId: string,
    data: CreateFollowUpAttempt
  ): Promise<ExecutionProcess> => {
    const response = await makeRequest(`/api/sessions/${sessionId}/follow-up`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<ExecutionProcess>(response);
  },
};
