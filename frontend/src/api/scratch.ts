import type {
  CreateScratch,
  Scratch,
  ScratchType,
  UpdateScratch,
} from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const scratchApi = {
  create: async (
    scratchType: ScratchType,
    id: string,
    data: CreateScratch
  ): Promise<Scratch> => {
    const response = await makeRequest(`/api/scratch/${scratchType}/${id}`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Scratch>(response);
  },

  get: async (scratchType: ScratchType, id: string): Promise<Scratch> => {
    const response = await makeRequest(`/api/scratch/${scratchType}/${id}`);
    return handleApiResponse<Scratch>(response);
  },

  update: async (
    scratchType: ScratchType,
    id: string,
    data: UpdateScratch
  ): Promise<void> => {
    const response = await makeRequest(`/api/scratch/${scratchType}/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
    return handleApiResponse<void>(response);
  },

  delete: async (scratchType: ScratchType, id: string): Promise<void> => {
    const response = await makeRequest(`/api/scratch/${scratchType}/${id}`, {
      method: 'DELETE',
    });
    return handleApiResponse<void>(response);
  },

  getStreamUrl: (scratchType: ScratchType, id: string): string =>
    `/api/scratch/${scratchType}/${id}/stream/ws`,
};
