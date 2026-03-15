import type {
  AvailabilityInfo,
  BaseCodingAgent,
  CheckEditorAvailabilityResponse,
  CliDependencyPreflightResponse,
  Config,
  CodexProtocolCompatibility,
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

  checkAgentCompatibility: async (
    agent: BaseCodingAgent,
    variant?: string | null,
    refresh?: boolean
  ): Promise<CodexProtocolCompatibility> => {
    const params = new URLSearchParams();
    params.set('executor', agent);
    if (variant) {
      params.set('variant', variant);
    }
    if (refresh) {
      params.set('refresh', 'true');
    }

    const response = await makeRequest(
      `/api/agents/check-compatibility?${params.toString()}`,
      { cache: 'no-store' }
    );
    return handleApiResponse<CodexProtocolCompatibility>(response);
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
