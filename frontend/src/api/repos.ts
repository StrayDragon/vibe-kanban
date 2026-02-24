import type { GitBranch, Repo } from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const repoApi = {
  register: async (data: {
    path: string;
    display_name?: string;
  }): Promise<Repo> => {
    const response = await makeRequest('/api/repos', {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Repo>(response);
  },

  getBranches: async (repoId: string): Promise<GitBranch[]> => {
    const response = await makeRequest(`/api/repos/${repoId}/branches`);
    return handleApiResponse<GitBranch[]>(response);
  },

  init: async (data: {
    parent_path: string;
    folder_name: string;
  }): Promise<Repo> => {
    const response = await makeRequest('/api/repos/init', {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Repo>(response);
  },
};
