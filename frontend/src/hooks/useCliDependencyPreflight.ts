import { useQuery } from '@tanstack/react-query';
import type { BaseCodingAgent, CliDependencyPreflightResponse } from 'shared/types';
import { configApi } from '@/lib/api';

export function useCliDependencyPreflight(
  agent: BaseCodingAgent | null | undefined,
  enabled = true
) {
  return useQuery<CliDependencyPreflightResponse>({
    queryKey: ['cli-preflight', agent],
    queryFn: () => configApi.cliPreflight(agent as BaseCodingAgent),
    enabled: enabled && !!agent,
    staleTime: 30_000,
  });
}

