import type {
  ExecutionProcessPublic as ExecutionProcess,
  ExecutionProcessRepoState,
  LogHistoryPage,
} from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const executionProcessesApi = {
  getDetails: async (processId: string): Promise<ExecutionProcess> => {
    const response = await makeRequest(`/api/execution-processes/${processId}`);
    return handleApiResponse<ExecutionProcess>(response);
  },

  getRawLogsPage: async (
    processId: string,
    params: { limit: number; cursor?: bigint | null }
  ): Promise<LogHistoryPage> => {
    const search = new URLSearchParams();
    search.set('limit', String(params.limit));
    if (params.cursor != null) {
      search.set('cursor', String(params.cursor));
    }
    const suffix = search.toString() ? `?${search.toString()}` : '';
    const response = await makeRequest(
      `/api/execution-processes/${processId}/raw-logs/v2${suffix}`
    );
    return handleApiResponse<LogHistoryPage>(response);
  },

  getNormalizedLogsPage: async (
    processId: string,
    params: { limit: number; cursor?: bigint | null }
  ): Promise<LogHistoryPage> => {
    const search = new URLSearchParams();
    search.set('limit', String(params.limit));
    if (params.cursor != null) {
      search.set('cursor', String(params.cursor));
    }
    const suffix = search.toString() ? `?${search.toString()}` : '';
    const response = await makeRequest(
      `/api/execution-processes/${processId}/normalized-logs/v2${suffix}`
    );
    return handleApiResponse<LogHistoryPage>(response);
  },

  getRepoStates: async (
    processId: string
  ): Promise<ExecutionProcessRepoState[]> => {
    const response = await makeRequest(
      `/api/execution-processes/${processId}/repo-states`
    );
    return handleApiResponse<ExecutionProcessRepoState[]>(response);
  },

  stopExecutionProcess: async (processId: string): Promise<void> => {
    const response = await makeRequest(
      `/api/execution-processes/${processId}/stop`,
      {
        method: 'POST',
      }
    );
    return handleApiResponse<void>(response);
  },
};
