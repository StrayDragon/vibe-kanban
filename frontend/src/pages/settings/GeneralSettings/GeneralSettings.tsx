import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Copy, Loader2, RefreshCw } from 'lucide-react';

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
import { useCopyToClipboard } from '@/hooks/utils/useCopyToClipboard';
import { configApi } from '@/lib/api';
import type { ConfigStatusResponse } from 'shared/types';
import {
  TableBody,
  TableCell,
  TableHead,
  TableHeaderCell,
  TableRow,
} from '@/components/ui/table';
import { SettingsTable } from '@/pages/settings/components/SettingsTable';

const CONFIG_STATUS_QUERY_KEY = ['configStatus'] as const;

function basename(path: string): string {
  const trimmed = path.trim();
  if (!trimmed) return trimmed;

  const parts = trimmed.split(/[/\\]/);
  return parts[parts.length - 1] ?? trimmed;
}

export function GeneralSettings() {
  const { t } = useTranslation(['settings', 'common']);
  const queryClient = useQueryClient();
  const { reloadSystem } = useUserSystem();
  const copyToClipboard = useCopyToClipboard();
  const copyText = t('common:buttons.copy', 'Copy');

  const {
    data: status,
    isLoading,
    error,
    refetch,
  } = useQuery({
    queryKey: CONFIG_STATUS_QUERY_KEY,
    queryFn: configApi.getConfigStatus,
    staleTime: 5_000,
    refetchInterval: 5_000,
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
            {error instanceof Error
              ? error.message
              : t('common:error', 'Error')}
          </AlertDescription>
        </Alert>
      </div>
    );
  }

  const loadedAt = new Date(status.loaded_at_unix_ms);
  const schemaHeader = '# yaml-language-server: $schema=./config.schema.json';
  const projectsSchemaHeader =
    '# yaml-language-server: $schema=./projects.schema.json';
  const schemaUpsertCommand = 'vk config schema upsert';
  const secretEnvLabel = basename(status.secret_env_path) || 'secret.env';
  const rows: Array<{
    id: string;
    item: string;
    value: ReactNode;
    hint?: ReactNode;
    copyValue?: string;
  }> = [
    {
      id: 'loaded-at',
      item: t('settings.config.loadedAt', 'Loaded at'),
      value: (
        <span className="text-sm text-muted-foreground">
          {loadedAt.toLocaleString()}
        </span>
      ),
    },
    {
      id: 'config-dir',
      item: t('settings.config.configDir', 'Config directory'),
      value: (
        <code className="text-xs font-mono break-all">{status.config_dir}</code>
      ),
      copyValue: status.config_dir,
    },
    {
      id: 'config-yaml',
      item: 'config.yaml',
      value: (
        <code className="text-xs font-mono break-all">
          {status.config_path}
        </code>
      ),
      copyValue: status.config_path,
    },
    {
      id: 'projects-yaml',
      item: 'projects.yaml',
      value: (
        <code className="text-xs font-mono break-all">
          {status.projects_path}
        </code>
      ),
      hint: t(
        'settings.config.projectsHint',
        'Projects and repos are configured here (or split across projects.d/*.yaml).'
      ),
      copyValue: status.projects_path,
    },
    {
      id: 'projects-dir',
      item: 'projects.d/',
      value: (
        <code className="text-xs font-mono break-all">
          {status.projects_dir}
        </code>
      ),
      hint: t(
        'settings.config.projectsDirHint',
        'Optional: split projects into multiple YAML files (merged deterministically).'
      ),
      copyValue: status.projects_dir,
    },
    {
      id: 'secret-env',
      item: 'secret.env',
      value: (
        <code
          className="text-xs font-mono break-all"
          title={status.secret_env_path}
        >
          {secretEnvLabel}
        </code>
      ),
      hint: t(
        'settings.config.secretHint',
        'Use {{secret.NAME}} to reference values from secret.env (higher priority than system env). Templates are only supported in specific whitelisted fields (see schema).'
      ),
      copyValue: status.secret_env_path,
    },
    {
      id: 'config-schema-path',
      item: 'config.schema.json',
      value: (
        <code className="text-xs font-mono break-all">{status.schema_path}</code>
      ),
      copyValue: status.schema_path,
    },
    {
      id: 'schema-upsert-command',
      item: t('settings.config.schemaUpsertCommandLabel', 'Schema upsert command'),
      value: (
        <code className="text-xs font-mono break-all">{schemaUpsertCommand}</code>
      ),
      hint: t(
        'settings.config.schemaUpsertHint',
        'Generate/update schema files with:'
      ),
      copyValue: schemaUpsertCommand,
    },
    {
      id: 'config-yaml-schema-header',
      item: t('settings.config.configYamlSchemaHeader', 'config.yaml schema header'),
      value: (
        <code className="text-xs font-mono break-all">{schemaHeader}</code>
      ),
      hint: t(
        'settings.config.schemaHint',
        'Add this line to the top of config.yaml to enable YAML LSP validation:'
      ),
      copyValue: schemaHeader,
    },
    {
      id: 'projects-schema-path',
      item: 'projects.schema.json',
      value: (
        <code className="text-xs font-mono break-all">
          {status.projects_schema_path}
        </code>
      ),
      copyValue: status.projects_schema_path,
    },
    {
      id: 'projects-yaml-schema-header',
      item: t(
        'settings.config.projectsYamlSchemaHeader',
        'projects.yaml schema header'
      ),
      value: (
        <code className="text-xs font-mono break-all">
          {projectsSchemaHeader}
        </code>
      ),
      hint: t(
        'settings.config.projectsSchemaHint',
        'Add this line to the top of projects.yaml (or projects.d/*.yaml) to enable YAML LSP validation:'
      ),
      copyValue: projectsSchemaHeader,
    },
  ];

  return (
    <div className="space-y-6">
      {status.last_error && (
        <Alert variant="destructive">
          <AlertTitle>
            {t('settings.config.lastError', 'Last error')}
          </AlertTitle>
          <AlertDescription>{status.last_error}</AlertDescription>
        </Alert>
      )}

      {status.dirty && (
        <Alert>
          <AlertTitle>
            {t('settings.config.dirtyTitle', 'Modified but not applied')}
          </AlertTitle>
          <AlertDescription>
            {t(
              'settings.config.dirtyDescription',
              'Config files have changed on disk but are not applied yet. Click Reload to apply.'
            )}
          </AlertDescription>
        </Alert>
      )}

      <Alert>
        <AlertTitle>
          {t('settings.agents.readOnlyTitle', 'Agents are file-configured')}
        </AlertTitle>
        <AlertDescription>
          {t(
            'settings.agents.readOnlyDescription',
            'Executor profiles and agent-related settings are configured in config.yaml. Editing via UI is disabled.'
          )}
        </AlertDescription>
      </Alert>

      <Card>
        <CardHeader>
          <CardTitle>{t('settings.config.title', 'Config')}</CardTitle>
          <CardDescription>
            {t(
              'settings.config.description',
              'Edit config.yaml / projects.yaml / projects.d/*.yaml on disk, then reload to apply changes.'
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
          <SettingsTable>
            <TableHead className="bg-muted/50 sticky top-0 z-10 border-b normal-case">
              <tr>
                <TableHeaderCell className="p-2 text-xs font-medium w-32 sm:w-44">
                  {t('settings.table.columns.item', 'Item')}
                </TableHeaderCell>
                <TableHeaderCell className="p-2 text-xs font-medium">
                  {t('settings.table.columns.value', 'Value')}
                </TableHeaderCell>
                <TableHeaderCell className="p-2 text-xs font-medium text-right w-14">
                  {t('settings.table.columns.actions', 'Actions')}
                </TableHeaderCell>
              </tr>
            </TableHead>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.id} className="hover:bg-muted/30">
                  <TableCell className="p-2 align-top text-xs text-muted-foreground font-medium">
                    {row.item}
                  </TableCell>
                  <TableCell className="p-2 align-top">
                    <div className="space-y-1">
                      {row.value}
                      {row.hint && (
                        <div className="text-xs text-muted-foreground">
                          {row.hint}
                        </div>
                      )}
                    </div>
                  </TableCell>
                  <TableCell className="p-2 align-top text-right">
                    {row.copyValue ? (
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8"
                        title={`${copyText}: ${row.item}`}
                        aria-label={`${copyText}: ${row.item}`}
                        onClick={() => copyToClipboard(row.item, row.copyValue!)}
                      >
                        <Copy className="h-4 w-4" />
                      </Button>
                    ) : (
                      <span className="sr-only">
                        {t('settings.table.columns.actions', 'Actions')}
                      </span>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </SettingsTable>
        </CardContent>
      </Card>
    </div>
  );
}
