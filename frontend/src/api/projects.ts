import type {
  CreateProject,
  CreateProjectRepo,
  Project,
  ProjectFileSearchResponse,
  ProjectRepo,
  ProjectRepoPublic,
  Repo,
  UpdateProject,
  UpdateProjectRepo,
} from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const projectsApi = {
  create: async (data: CreateProject): Promise<Project> => {
    const response = await makeRequest('/api/projects', {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Project>(response);
  },

  update: async (id: string, data: UpdateProject): Promise<Project> => {
    const response = await makeRequest(`/api/projects/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Project>(response);
  },

  delete: async (id: string): Promise<void> => {
    const response = await makeRequest(`/api/projects/${id}`, {
      method: 'DELETE',
    });
    return handleApiResponse<void>(response);
  },

  searchFiles: async (
    id: string,
    query: string,
    mode?: string,
    options?: RequestInit
  ): Promise<ProjectFileSearchResponse> => {
    const modeParam = mode ? `&mode=${encodeURIComponent(mode)}` : '';
    const response = await makeRequest(
      `/api/projects/${id}/search?q=${encodeURIComponent(query)}${modeParam}`,
      options
    );
    return handleApiResponse<ProjectFileSearchResponse>(response);
  },

  getRepositories: async (projectId: string): Promise<Repo[]> => {
    const response = await makeRequest(
      `/api/projects/${projectId}/repositories`
    );
    return handleApiResponse<Repo[]>(response);
  },

  addRepository: async (
    projectId: string,
    data: CreateProjectRepo
  ): Promise<Repo> => {
    const response = await makeRequest(
      `/api/projects/${projectId}/repositories`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    );
    return handleApiResponse<Repo>(response);
  },

  deleteRepository: async (
    projectId: string,
    repoId: string
  ): Promise<void> => {
    const response = await makeRequest(
      `/api/projects/${projectId}/repositories/${repoId}`,
      {
        method: 'DELETE',
      }
    );
    return handleApiResponse<void>(response);
  },

  getRepository: async (
    projectId: string,
    repoId: string
  ): Promise<ProjectRepoPublic> => {
    const response = await makeRequest(
      `/api/projects/${projectId}/repositories/${repoId}`
    );
    return handleApiResponse<ProjectRepoPublic>(response);
  },

  updateRepository: async (
    projectId: string,
    repoId: string,
    data: UpdateProjectRepo
  ): Promise<ProjectRepo> => {
    const response = await makeRequest(
      `/api/projects/${projectId}/repositories/${repoId}`,
      {
        method: 'PUT',
        body: JSON.stringify(data),
      }
    );
    return handleApiResponse<ProjectRepo>(response);
  },
};
