import type {
  AvailabilityInfo,
  BaseCodingAgent,
  CheckEditorAvailabilityResponse,
  CliDependencyPreflightResponse,
  CodexProtocolCompatibility,
  EditorType,
  UserSystemInfo,
} from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export type ConfigStatusResponse = {
  config_dir: string;
  config_path: string;
  projects_path: string;
  projects_dir: string;
  secret_env_path: string;
  schema_path: string;
  projects_schema_path: string;
  loaded_at_unix_ms: number;
  last_error: string | null;
};

export type OpenConfigTarget =
  | 'config_dir'
  | 'config_yaml'
  | 'projects_yaml'
  | 'projects_dir'
  | 'secret_env'
  | 'schema'
  | 'projects_schema';

export type OpenConfigTargetResponse = {
  url: string | null;
};

export const configApi = {
  getConfig: async (): Promise<UserSystemInfo> => {
    const response = await makeRequest('/api/info', { cache: 'no-store' });
    return handleApiResponse<UserSystemInfo>(response);
  },

  getConfigStatus: async (): Promise<ConfigStatusResponse> => {
    const response = await makeRequest('/api/config/status', { cache: 'no-store' });
    return handleApiResponse<ConfigStatusResponse>(response);
  },

  reloadConfig: async (): Promise<ConfigStatusResponse> => {
    const response = await makeRequest('/api/config/reload', { method: 'POST' });
    return handleApiResponse<ConfigStatusResponse>(response);
  },

  openConfigTarget: async (
    target: OpenConfigTarget,
    editorType?: string | null
  ): Promise<OpenConfigTargetResponse> => {
    const response = await makeRequest('/api/config/open', {
      method: 'POST',
      body: JSON.stringify({
        target,
        editor_type: editorType ?? undefined,
      }),
    });
    return handleApiResponse<OpenConfigTargetResponse>(response);
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
