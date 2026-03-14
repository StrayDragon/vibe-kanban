import type { ImageMetadata, ImageResponse } from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const imagesApi = {
  upload: async (file: File): Promise<ImageResponse> => {
    const formData = new FormData();
    formData.append('image', file);

    const response = await makeRequest('/api/images/upload', {
      method: 'POST',
      body: formData,
    });
    return handleApiResponse<ImageResponse>(response);
  },

  uploadForTask: async (taskId: string, file: File): Promise<ImageResponse> => {
    const formData = new FormData();
    formData.append('image', file);

    const response = await makeRequest(`/api/images/task/${taskId}/upload`, {
      method: 'POST',
      body: formData,
    });
    return handleApiResponse<ImageResponse>(response);
  },

  uploadForAttempt: async (
    attemptId: string,
    file: File
  ): Promise<ImageResponse> => {
    const formData = new FormData();
    formData.append('image', file);

    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/images/upload`,
      {
        method: 'POST',
        body: formData,
      }
    );
    return handleApiResponse<ImageResponse>(response);
  },

  delete: async (imageId: string): Promise<void> => {
    const response = await makeRequest(`/api/images/${imageId}`, {
      method: 'DELETE',
    });
    return handleApiResponse<void>(response);
  },

  getTaskImages: async (taskId: string): Promise<ImageResponse[]> => {
    const response = await makeRequest(`/api/images/task/${taskId}`);
    return handleApiResponse<ImageResponse[]>(response);
  },

  getAttemptImageMetadata: async (
    attemptId: string,
    path: string
  ): Promise<ImageMetadata | null> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/images/metadata?path=${encodeURIComponent(path)}`
    );
    return handleApiResponse<ImageMetadata | null>(response);
  },

  getTaskImageMetadata: async (
    taskId: string,
    path: string
  ): Promise<ImageMetadata | null> => {
    const response = await makeRequest(
      `/api/images/task/${taskId}/metadata?path=${encodeURIComponent(path)}`
    );
    return handleApiResponse<ImageMetadata | null>(response);
  },

  getImageUrl: (imageId: string): string => {
    return `/api/images/${imageId}/file`;
  },
};
