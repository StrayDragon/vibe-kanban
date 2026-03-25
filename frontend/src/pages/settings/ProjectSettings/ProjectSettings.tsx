import { useMutation } from '@tanstack/react-query';
import { ExternalLink, Loader2, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

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
import { useProjects } from '@/hooks/projects/useProjects';
import { configApi } from '@/lib/api';

export function ProjectSettings() {
  const { t } = useTranslation(['settings', 'common']);
  const { reloadSystem } = useUserSystem();
  const { projects, isLoading, error, isConnected } = useProjects();

  const reloadMutation = useMutation({
    mutationFn: configApi.reloadConfig,
    onSuccess: async () => {
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

  const openConfigYaml = async () => {
    try {
      const result = await configApi.openConfigTarget('config_yaml');
      if (result.url) {
        window.open(result.url, '_blank', 'noopener,noreferrer');
      }
    } catch (err) {
      console.error('Failed to open config.yaml:', err);
      toast({
        variant: 'destructive',
        title: 'Open failed',
        description:
          err instanceof Error ? err.message : 'Failed to open config.yaml.',
      });
    }
  };

  return (
    <div className="space-y-6">
      <Alert>
        <AlertTitle>
          {t('settings.projects.readOnlyTitle', 'Projects are file-configured')}
        </AlertTitle>
        <AlertDescription>
          {t(
            'settings.projects.readOnlyDescription',
            'Projects and repos are configured in config.yaml. Editing via UI is disabled.'
          )}
        </AlertDescription>
      </Alert>

      <Card>
        <CardHeader>
          <CardTitle>{t('settings.projects.title', 'Projects')}</CardTitle>
          <CardDescription>
            {t(
              'settings.projects.description',
              'Edit config.yaml on disk, then reload to apply changes.'
            )}
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-center gap-2">
          <Button variant="outline" onClick={() => void openConfigYaml()}>
            <ExternalLink className="mr-2 h-4 w-4" />
            {t('settings.config.open', 'Open config.yaml')}
          </Button>
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
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>
            {t('settings.projects.configured', 'Configured projects')}
          </CardTitle>
          <CardDescription>
            {t(
              'settings.projects.configuredDescription',
              'Read-only view of projects currently loaded from config.yaml.'
            )}{' '}
            <span className="text-xs text-muted-foreground">
              {isConnected
                ? t('settings.projects.connected', 'Live')
                : t('settings.projects.disconnected', 'Disconnected')}
            </span>
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {error && (
            <Alert variant="destructive">
              <AlertTitle>{t('common:error', 'Error')}</AlertTitle>
              <AlertDescription>{error.message}</AlertDescription>
            </Alert>
          )}

          {!error && isLoading && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              {t('settings.projects.loading', 'Loading')}
            </div>
          )}

          {!error && !isLoading && projects.length === 0 && (
            <div className="text-sm text-muted-foreground">
              {t(
                'settings.projects.empty',
                'No projects configured. Add a projects entry to config.yaml and reload.'
              )}
            </div>
          )}

          {!error &&
            !isLoading &&
            projects.map((project) => (
              <div
                key={project.id}
                className="flex flex-col gap-1 rounded-md border border-border/60 p-3"
              >
                <div className="text-sm font-medium">{project.name}</div>
                <div className="text-xs text-muted-foreground font-mono break-all">
                  {project.id}
                </div>
              </div>
            ))}
        </CardContent>
      </Card>
    </div>
  );
}

