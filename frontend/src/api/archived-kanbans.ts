import type {
  ArchiveProjectKanbanRequest,
  ArchiveProjectKanbanResponse,
  ArchivedKanbanWithTaskCount,
  DeleteArchivedKanbanResponse,
  GetArchivedKanbanResponse,
  RestoreArchivedKanbanRequest,
  RestoreArchivedKanbanResponse,
} from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const archivedKanbansApi = {
  listByProject: async (
    projectId: string
  ): Promise<ArchivedKanbanWithTaskCount[]> => {
    const response = await makeRequest(
      `/api/projects/${encodeURIComponent(projectId)}/archived-kanbans`
    );
    return handleApiResponse<ArchivedKanbanWithTaskCount[]>(response);
  },

  archiveProjectKanban: async (
    projectId: string,
    data: ArchiveProjectKanbanRequest
  ): Promise<ArchiveProjectKanbanResponse> => {
    const response = await makeRequest(
      `/api/projects/${encodeURIComponent(projectId)}/archived-kanbans`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    );
    return handleApiResponse<ArchiveProjectKanbanResponse>(response);
  },

  getById: async (archiveId: string): Promise<GetArchivedKanbanResponse> => {
    const response = await makeRequest(
      `/api/archived-kanbans/${encodeURIComponent(archiveId)}`
    );
    return handleApiResponse<GetArchivedKanbanResponse>(response);
  },

  restore: async (
    archiveId: string,
    data: RestoreArchivedKanbanRequest
  ): Promise<RestoreArchivedKanbanResponse> => {
    const response = await makeRequest(
      `/api/archived-kanbans/${encodeURIComponent(archiveId)}/restore`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    );
    return handleApiResponse<RestoreArchivedKanbanResponse>(response);
  },

  delete: async (archiveId: string): Promise<DeleteArchivedKanbanResponse> => {
    const response = await makeRequest(
      `/api/archived-kanbans/${encodeURIComponent(archiveId)}`,
      {
        method: 'DELETE',
      }
    );
    return handleApiResponse<DeleteArchivedKanbanResponse>(response);
  },
};
