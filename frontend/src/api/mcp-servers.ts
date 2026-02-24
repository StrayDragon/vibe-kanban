import type {
  GetMcpServerResponse,
  McpServerQuery,
  UpdateMcpServersBody,
} from 'shared/types';

import { ApiError, handleApiResponse, makeRequest } from './client';

export const mcpServersApi = {
  load: async (query: McpServerQuery): Promise<GetMcpServerResponse> => {
    const params = new URLSearchParams(query);
    const response = await makeRequest(`/api/mcp-config?${params.toString()}`);
    return handleApiResponse<GetMcpServerResponse>(response);
  },

  save: async (
    query: McpServerQuery,
    data: UpdateMcpServersBody
  ): Promise<void> => {
    const params = new URLSearchParams(query);
    const response = await makeRequest(`/api/mcp-config?${params.toString()}`, {
      method: 'POST',
      body: JSON.stringify(data),
    });

    if (!response.ok) {
      const errorData = await response.json();
      console.error('[API Error] Failed to save MCP servers', {
        message: errorData.message,
        status: response.status,
        response,
        timestamp: new Date().toISOString(),
      });
      throw new ApiError(
        errorData.message || 'Failed to save MCP servers',
        response.status,
        response
      );
    }
  },
};
