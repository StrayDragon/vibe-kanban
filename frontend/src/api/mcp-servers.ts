import type { GetMcpServerResponse, McpServerQuery } from 'shared/types';

import { handleApiResponse, makeRequest } from './client';

export const mcpServersApi = {
  load: async (query: McpServerQuery): Promise<GetMcpServerResponse> => {
    const params = new URLSearchParams(query);
    const response = await makeRequest(`/api/mcp-config?${params.toString()}`);
    return handleApiResponse<GetMcpServerResponse>(response);
  },
};
