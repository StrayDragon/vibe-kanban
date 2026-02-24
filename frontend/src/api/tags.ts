import type { CreateTag, Tag, TagSearchParams, UpdateTag } from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const tagsApi = {
  list: async (params?: TagSearchParams): Promise<Tag[]> => {
    const queryParam = params?.search
      ? `?search=${encodeURIComponent(params.search)}`
      : '';
    const response = await makeRequest(`/api/tags${queryParam}`);
    return handleApiResponse<Tag[]>(response);
  },

  create: async (data: CreateTag): Promise<Tag> => {
    const response = await makeRequest('/api/tags', {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Tag>(response);
  },

  update: async (tagId: string, data: UpdateTag): Promise<Tag> => {
    const response = await makeRequest(`/api/tags/${tagId}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Tag>(response);
  },

  delete: async (tagId: string): Promise<void> => {
    const response = await makeRequest(`/api/tags/${tagId}`, {
      method: 'DELETE',
    });
    return handleApiResponse<void>(response);
  },
};
