import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Copy, ExternalLink, Loader2, RefreshCw } from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { toast } from '@/components/ui/toast';
import { useUserSystem } from '@/components/ConfigProvider';
import {
  configApi,
  type ConfigStatusResponse,
  type OpenConfigTarget,
} from '@/lib/api';

const CONFIG_STATUS_QUERY_KEY = ['configStatus'] as const;

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

export function GeneralSettings() {
  const { t } = useTranslation(['settings', 'common']);
  const queryClient = useQueryClient();
  const { reloadSystem } = useUserSystem();
  const [opening, setOpening] = useState<OpenConfigTarget | null>(null);

  const {
    data: status,
    isLoading,
    error,
    refetch,
  } = useQuery({
    queryKey: CONFIG_STATUS_QUERY_KEY,
    queryFn: configApi.getConfigStatus,
    staleTime: 5_000,
  });

  const reloadMutation = useMutation({
    mutationFn: configApi.reloadConfig,
    onSuccess: async (next: ConfigStatusResponse) => {
      queryClient.setQueryData(CONFIG_STATUS_QUERY_KEY, next);
      await reloadSystem();
      toast({
        title: 'Config reloaded',
        description: 'Reload succeeded.',
      });
    },
    onError: (err) => {
      console.error('Config reload failed:', err);
      toast({
        variant: 'destructive',
        title: 'Reload failed',
        description:
          err instanceof Error ? err.message : 'Config reload failed.',
      });
    },
  });

  const openTarget = async (target: OpenConfigTarget) => {
    setOpening(target);
    try {
      const result = await configApi.openConfigTarget(target);
      if (result.url) {
        window.open(result.url, '_blank', 'noopener,noreferrer');
      }
    } catch (err) {
      console.error('Failed to open config target:', err);
      toast({
        variant: 'destructive',
        title: 'Open failed',
        description: err instanceof Error ? err.message : 'Open failed.',
      });
    } finally {
      setOpening(null);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-8">
        <Loader2 className="h-8 w-8 animate-spin" />
        <span className="ml-2">{t('settings.general.loading', 'Loading')}</span>
      </div>
    );
  }

  if (error || !status) {
    return (
      <div className="py-8">
        <Alert variant="destructive">
          <AlertTitle>{t('common:error', 'Error')}</AlertTitle>
          <AlertDescription>
            {error instanceof Error ? error.message : t('common:error', 'Error')}
          </AlertDescription>
        </Alert>
      </div>
    );
  }

  const loadedAt = new Date(status.loaded_at_unix_ms);
  const schemaHeader = '# yaml-language-server: $schema=./config.schema.json';
  const projectsSchemaHeader = '# yaml-language-server: $schema=./projects.schema.json';

  return (
    <div className="space-y-6">
      {status.last_error && (
        <Alert variant="destructive">
          <AlertTitle>{t('settings.config.lastError', 'Last error')}</AlertTitle>
          <AlertDescription>{status.last_error}</AlertDescription>
        </Alert>
      )}

      <Card>
        <CardHeader>
          <CardTitle>{t('settings.config.title', 'Config')}</CardTitle>
          <CardDescription>
            {t(
              'settings.config.description',
              'Edit config.yaml / projects.yaml on disk, then reload to apply changes.'
            )}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <Button
              onClick={() => reloadMutation.mutate()}
              disabled={reloadMutation.isPending}
            >
              {reloadMutation.isPending ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  {t('settings.config.reloading', 'Reloading')}
                </>
              ) : (
                <>
                  <RefreshCw className="mr-2 h-4 w-4" />
                  {t('settings.config.reload', 'Reload')}
                </>
              )}
            </Button>
            <Button
              variant="outline"
              onClick={() => void refetch()}
              disabled={isLoading}
            >
              {t('settings.config.refresh', 'Refresh status')}
            </Button>
          </div>

          <div className="space-y-3">
            <div className="space-y-1">
              <div className="text-sm font-medium">
                {t('settings.config.loadedAt', 'Loaded at')}
              </div>
              <div className="text-sm text-muted-foreground">
                {loadedAt.toLocaleString()}
              </div>
            </div>

            <div className="space-y-1">
              <div className="text-sm font-medium">
                {t('settings.config.configDir', 'Config directory')}
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <code className="text-xs break-all">{status.config_dir}</code>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => copyToClipboard('config dir', status.config_dir)}
                >
                  <Copy className="mr-2 h-4 w-4" />
                  {t('common:buttons.copy', 'Copy')}
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => void openTarget('config_dir')}
                  disabled={opening === 'config_dir'}
                >
                  <ExternalLink className="mr-2 h-4 w-4" />
                  {t('settings.config.open', 'Open')}
                </Button>
              </div>
            </div>

            <div className="space-y-1">
              <div className="text-sm font-medium">config.yaml</div>
              <div className="flex flex-wrap items-center gap-2">
                <code className="text-xs break-all">{status.config_path}</code>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => copyToClipboard('config.yaml path', status.config_path)}
                >
                  <Copy className="mr-2 h-4 w-4" />
                  {t('common:buttons.copy', 'Copy')}
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => void openTarget('config_yaml')}
                  disabled={opening === 'config_yaml'}
                >
                  <ExternalLink className="mr-2 h-4 w-4" />
                  {t('settings.config.open', 'Open')}
                </Button>
              </div>
            </div>

            <div className="space-y-1">
              <div className="text-sm font-medium">projects.yaml</div>
              <div className="flex flex-wrap items-center gap-2">
                <code className="text-xs break-all">{status.projects_path}</code>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    copyToClipboard('projects.yaml path', status.projects_path)
                  }
                >
                  <Copy className="mr-2 h-4 w-4" />
                  {t('common:buttons.copy', 'Copy')}
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => void openTarget('projects_yaml')}
                  disabled={opening === 'projects_yaml'}
                >
                  <ExternalLink className="mr-2 h-4 w-4" />
                  {t('settings.config.open', 'Open')}
                </Button>
              </div>
              <div className="text-xs text-muted-foreground">
                {t(
                  'settings.config.projectsHint',
                  'Projects and repos are configured here (or split across projects.d/*.yaml).'
                )}
              </div>
            </div>

            <div className="space-y-1">
              <div className="text-sm font-medium">projects.d/</div>
              <div className="flex flex-wrap items-center gap-2">
                <code className="text-xs break-all">{status.projects_dir}</code>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    copyToClipboard('projects.d path', status.projects_dir)
                  }
                >
                  <Copy className="mr-2 h-4 w-4" />
                  {t('common:buttons.copy', 'Copy')}
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => void openTarget('projects_dir')}
                  disabled={opening === 'projects_dir'}
                >
                  <ExternalLink className="mr-2 h-4 w-4" />
                  {t('settings.config.open', 'Open')}
                </Button>
              </div>
              <div className="text-xs text-muted-foreground">
                {t(
                  'settings.config.projectsDirHint',
                  'Optional: split projects into multiple YAML files (merged deterministically).'
                )}
              </div>
            </div>

            <div className="space-y-1">
              <div className="text-sm font-medium">secret.env</div>
              <div className="flex flex-wrap items-center gap-2">
                <code className="text-xs break-all">{status.secret_env_path}</code>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    copyToClipboard('secret.env path', status.secret_env_path)
                  }
                >
                  <Copy className="mr-2 h-4 w-4" />
                  {t('common:buttons.copy', 'Copy')}
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => void openTarget('secret_env')}
                  disabled={opening === 'secret_env'}
                >
                  <ExternalLink className="mr-2 h-4 w-4" />
                  {t('settings.config.open', 'Open')}
                </Button>
              </div>
              <div className="text-xs text-muted-foreground">
                {t(
                  'settings.config.secretHint',
                  'Use {{secret.NAME}} to reference values from secret.env (higher priority than system env).'
                )}
              </div>
            </div>

            <div className="space-y-1">
              <div className="text-sm font-medium">config.schema.json</div>
              <div className="flex flex-wrap items-center gap-2">
                <code className="text-xs break-all">{status.schema_path}</code>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    copyToClipboard('config.schema.json path', status.schema_path)
                  }
                >
                  <Copy className="mr-2 h-4 w-4" />
                  {t('common:buttons.copy', 'Copy')}
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => void openTarget('schema')}
                  disabled={opening === 'schema'}
                >
                  <ExternalLink className="mr-2 h-4 w-4" />
                  {t('settings.config.open', 'Open')}
                </Button>
              </div>
              <div className="text-xs text-muted-foreground">
                {t(
                  'settings.config.schemaHint',
                  'Add this line to the top of config.yaml to enable YAML LSP validation:'
                )}
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <code className="text-xs break-all">{schemaHeader}</code>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => copyToClipboard('schema header', schemaHeader)}
                >
                  <Copy className="mr-2 h-4 w-4" />
                  {t('common:buttons.copy', 'Copy')}
                </Button>
              </div>
            </div>

            <div className="space-y-1">
              <div className="text-sm font-medium">projects.schema.json</div>
              <div className="flex flex-wrap items-center gap-2">
                <code className="text-xs break-all">{status.projects_schema_path}</code>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    copyToClipboard(
                      'projects.schema.json path',
                      status.projects_schema_path
                    )
                  }
                >
                  <Copy className="mr-2 h-4 w-4" />
                  {t('common:buttons.copy', 'Copy')}
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => void openTarget('projects_schema')}
                  disabled={opening === 'projects_schema'}
                >
                  <ExternalLink className="mr-2 h-4 w-4" />
                  {t('settings.config.open', 'Open')}
                </Button>
              </div>
              <div className="text-xs text-muted-foreground">
                {t(
                  'settings.config.projectsSchemaHint',
                  'Add this line to the top of projects.yaml to enable YAML LSP validation:'
                )}
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <code className="text-xs break-all">{projectsSchemaHeader}</code>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    copyToClipboard('projects schema header', projectsSchemaHeader)
                  }
                >
                  <Copy className="mr-2 h-4 w-4" />
                  {t('common:buttons.copy', 'Copy')}
                </Button>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
