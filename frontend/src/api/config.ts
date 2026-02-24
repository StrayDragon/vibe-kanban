import type {
  AvailabilityInfo,
  BaseCodingAgent,
  CheckEditorAvailabilityResponse,
  CliDependencyPreflightResponse,
  Config,
  EditorType,
  UserSystemInfo,
} from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const configApi = {
  getConfig: async (): Promise<UserSystemInfo> => {
    const response = await makeRequest('/api/info', { cache: 'no-store' });
    return handleApiResponse<UserSystemInfo>(response);
  },

  saveConfig: async (config: Config): Promise<Config> => {
    const response = await makeRequest('/api/config', {
      method: 'PUT',
      body: JSON.stringify(config),
    });
    return handleApiResponse<Config>(response);
  },

  checkEditorAvailability: async (
    editorType: EditorType
  ): Promise<CheckEditorAvailabilityResponse> => {
    const response = await makeRequest(
      `/api/editors/check-availability?editor_type=${encodeURIComponent(editorType)}`
    );
    return handleApiResponse<CheckEditorAvailabilityResponse>(response);
  },

  checkAgentAvailability: async (
    agent: BaseCodingAgent
  ): Promise<AvailabilityInfo> => {
    const response = await makeRequest(
      `/api/agents/check-availability?executor=${encodeURIComponent(agent)}`
    );
    return handleApiResponse<AvailabilityInfo>(response);
  },

  cliPreflight: async (
    agent: BaseCodingAgent
  ): Promise<CliDependencyPreflightResponse> => {
    const response = await makeRequest(
      `/api/preflight/cli?executor=${encodeURIComponent(agent)}`
    );
    return handleApiResponse<CliDependencyPreflightResponse>(response);
  },
};
