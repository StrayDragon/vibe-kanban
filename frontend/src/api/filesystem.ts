import type { DirectoryEntry, DirectoryListResponse } from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const fileSystemApi = {
  list: async (path?: string): Promise<DirectoryListResponse> => {
    const queryParam = path ? `?path=${encodeURIComponent(path)}` : '';
    const response = await makeRequest(
      `/api/filesystem/directory${queryParam}`
    );
    return handleApiResponse<DirectoryListResponse>(response);
  },

  listGitRepos: async (path?: string): Promise<DirectoryEntry[]> => {
    const queryParam = path ? `?path=${encodeURIComponent(path)}` : '';
    const response = await makeRequest(
      `/api/filesystem/git-repos${queryParam}`
    );
    return handleApiResponse<DirectoryEntry[]>(response);
  },
};
