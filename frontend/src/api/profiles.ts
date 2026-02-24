import type {
  ImportLlmanProfilesResponse,
  ResolveLlmanPathResponse,
} from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const profilesApi = {
  load: async (): Promise<{ content: string; path: string }> => {
    const response = await makeRequest('/api/profiles');
    return handleApiResponse<{ content: string; path: string }>(response);
  },

  save: async (content: string): Promise<string> => {
    const response = await makeRequest('/api/profiles', {
      method: 'PUT',
      body: content,
      headers: {
        'Content-Type': 'application/json',
      },
    });
    return handleApiResponse<string>(response);
  },

  importLlman: async (): Promise<ImportLlmanProfilesResponse> => {
    const response = await makeRequest('/api/profiles/import-llman', {
      method: 'POST',
    });
    return handleApiResponse<ImportLlmanProfilesResponse>(response);
  },

  resolveLlmanPath: async (): Promise<ResolveLlmanPathResponse> => {
    const response = await makeRequest('/api/profiles/llman-path');
    return handleApiResponse<ResolveLlmanPathResponse>(response);
  },
};
