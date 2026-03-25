import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Copy, Loader2 } from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { JSONEditor } from '@/components/ui/json-editor';
import { toast } from '@/components/ui/toast';
import { useUserSystem } from '@/components/ConfigProvider';
import { mcpServersApi } from '@/lib/api';
import type { BaseCodingAgent } from 'shared/types';

function copyToClipboard(label: string, value: string) {
  void navigator.clipboard
    .writeText(value)
    .then(() => {
      toast({
        title: 'Copied',
        description: `${label} copied to clipboard.`,
      });
    })
    .catch((err) => {
      console.error('Failed to copy to clipboard:', err);
      toast({
        variant: 'destructive',
        title: 'Copy failed',
        description: `Could not copy ${label}.`,
      });
    });
}

export function McpSettings() {
  const { t } = useTranslation(['settings', 'common']);
  const { config, profiles } = useUserSystem();

  const executorOptions = useMemo(() => {
    return Object.keys(profiles ?? {}).sort();
  }, [profiles]);

  const defaultExecutor = useMemo(() => {
    const fromConfig = config?.executor_profile?.executor;
    if (fromConfig && executorOptions.includes(fromConfig)) return fromConfig;
    return executorOptions[0] ?? null;
  }, [config?.executor_profile?.executor, executorOptions]);

  const [selectedExecutor, setSelectedExecutor] = useState<string | null>(
    defaultExecutor
  );

  useEffect(() => {
    if (!selectedExecutor && defaultExecutor) {
      setSelectedExecutor(defaultExecutor);
    }
  }, [defaultExecutor, selectedExecutor]);

  const {
    data,
    isLoading,
    error,
    refetch,
  } = useQuery({
    queryKey: ['mcpConfig', selectedExecutor],
    queryFn: async () => {
      if (!selectedExecutor) return null;
      return await mcpServersApi.load({
        executor: selectedExecutor as BaseCodingAgent,
      });
    },
    enabled: Boolean(selectedExecutor),
    staleTime: 5_000,
  });

  const configPath = data?.config_path ?? '';
  const serversJson = useMemo(() => {
    if (!data?.mcp_config) return '{}';
    try {
      return JSON.stringify((data.mcp_config as any).servers ?? {}, null, 2);
    } catch {
      return '{}';
    }
  }, [data?.mcp_config]);

  const serverKeys = useMemo(() => {
    if (!data?.mcp_config) return [];
    const servers = (data.mcp_config as any).servers as Record<string, unknown>;
    return Object.keys(servers ?? {}).sort();
  }, [data?.mcp_config]);

  return (
    <div className="space-y-6">
      <Alert>
        <AlertTitle>
          {t('settings.mcp.readOnlyTitle', 'MCP config is file-backed')}
        </AlertTitle>
        <AlertDescription>
          {t(
            'settings.mcp.readOnlyDescription',
            'Editing MCP servers via UI is disabled. Edit the executor config file on disk.'
          )}
        </AlertDescription>
      </Alert>

      <Card>
        <CardHeader>
          <CardTitle>{t('settings.mcp.title', 'MCP')}</CardTitle>
          <CardDescription>
            {t(
              'settings.mcp.description',
              'Select an executor to view its MCP config file path and current servers.'
            )}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <div className="min-w-[220px]">
              <Select
                value={selectedExecutor ?? undefined}
                onValueChange={(value) => setSelectedExecutor(value)}
                disabled={executorOptions.length === 0}
              >
                <SelectTrigger>
                  <SelectValue
                    placeholder={t('settings.mcp.selectExecutor', 'Select executor')}
                  />
                </SelectTrigger>
                <SelectContent>
                  {executorOptions.map((executor) => (
                    <SelectItem key={executor} value={executor}>
                      {executor}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <Button
              variant="outline"
              onClick={() => void refetch()}
              disabled={isLoading || !selectedExecutor}
            >
              {isLoading ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  {t('common:loading', 'Loading')}
                </>
              ) : (
                t('common:buttons.refresh', 'Refresh')
              )}
            </Button>
          </div>

          {error && (
            <Alert variant="destructive">
              <AlertTitle>{t('common:error', 'Error')}</AlertTitle>
              <AlertDescription>
                {error instanceof Error ? error.message : String(error)}
              </AlertDescription>
            </Alert>
          )}

          {!error && selectedExecutor && configPath && (
            <div className="space-y-2">
              <div className="text-sm font-medium">
                {t('settings.mcp.configPath', 'Config file')}
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <code className="text-xs break-all">{configPath}</code>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => copyToClipboard('config path', configPath)}
                >
                  <Copy className="mr-2 h-4 w-4" />
                  {t('common:buttons.copy', 'Copy')}
                </Button>
              </div>
            </div>
          )}

          {!error && selectedExecutor && (
            <div className="space-y-2">
              <div className="text-sm font-medium">
                {t('settings.mcp.servers', 'Servers')}
              </div>
              <div className="text-xs text-muted-foreground">
                {serverKeys.length > 0
                  ? serverKeys.join(', ')
                  : t('settings.mcp.noServers', 'No servers configured')}
              </div>
              <JSONEditor
                value={serversJson}
                onChange={() => {}}
                disabled
                minHeight={220}
              />
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

