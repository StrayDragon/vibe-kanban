import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
} from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  type Config,
  type Environment,
  type BaseAgentCapability,
  type AgentCommandResolution,
} from 'shared/types';
import type { ExecutorConfig } from 'shared/types';
import { configApi } from '../lib/api';
import { updateLanguageFromConfig } from '../i18n/config';
import { userSystemKeys } from '@/query-keys/userSystemKeys';

interface UserSystemState {
  config: Config | null;
  environment: Environment | null;
  profiles: Record<string, ExecutorConfig> | null;
  capabilities: Record<string, BaseAgentCapability[]> | null;
  agentCommandResolutions: Record<string, AgentCommandResolution> | null;
}

interface UserSystemContextType {
  // Full system state
  system: UserSystemState;

  config: Config | null;

  // System data access
  environment: Environment | null;
  profiles: Record<string, ExecutorConfig> | null;
  capabilities: Record<string, BaseAgentCapability[]> | null;
  agentCommandResolutions: Record<string, AgentCommandResolution> | null;

  // Reload system data
  reloadSystem: () => Promise<void>;

  // State
  loading: boolean;
}

const UserSystemContext = createContext<UserSystemContextType | undefined>(
  undefined
);

interface UserSystemProviderProps {
  children: ReactNode;
}

export function UserSystemProvider({ children }: UserSystemProviderProps) {
  const queryClient = useQueryClient();

  const { data: userSystemInfo, isLoading } = useQuery({
    queryKey: userSystemKeys.all,
    queryFn: configApi.getConfig,
    staleTime: 5 * 60 * 1000, // 5 minutes
  });

  const config = userSystemInfo?.config || null;
  const environment = userSystemInfo?.environment || null;
  const profiles =
    (userSystemInfo?.executors as Record<string, ExecutorConfig> | null) ||
    null;
  const capabilities =
    (userSystemInfo?.capabilities as Record<
      string,
      BaseAgentCapability[]
    > | null) || null;
  const agentCommandResolutions =
    (userSystemInfo?.agent_command_resolutions as Record<
      string,
      AgentCommandResolution
    > | null) || null;

  // Sync language with i18n when config changes
  useEffect(() => {
    if (config?.language) {
      updateLanguageFromConfig(config.language);
    }
  }, [config?.language]);

  const reloadSystem = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: userSystemKeys.all });
  }, [queryClient]);

  // Memoize context value to prevent unnecessary re-renders
  const value = useMemo<UserSystemContextType>(
    () => ({
      system: {
        config,
        environment,
        profiles,
        capabilities,
        agentCommandResolutions,
      },
      config,
      environment,
      profiles,
      capabilities,
      agentCommandResolutions,
      reloadSystem,
      loading: isLoading,
    }),
    [
      config,
      environment,
      profiles,
      capabilities,
      agentCommandResolutions,
      reloadSystem,
      isLoading,
    ]
  );

  return (
    <UserSystemContext.Provider value={value}>
      {children}
    </UserSystemContext.Provider>
  );
}

export function useUserSystem() {
  const context = useContext(UserSystemContext);
  if (context === undefined) {
    throw new Error('useUserSystem must be used within a UserSystemProvider');
  }
  return context;
}
