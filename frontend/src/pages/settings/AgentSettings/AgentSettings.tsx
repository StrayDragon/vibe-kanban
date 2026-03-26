import { useMutation } from '@tanstack/react-query';
import { Loader2, RefreshCw } from 'lucide-react';
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
import { configApi } from '@/lib/api';

export function AgentSettings() {
  const { t } = useTranslation(['settings', 'common']);
  const { reloadSystem } = useUserSystem();

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

  return (
    <div className="space-y-6">
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
          <CardTitle>{t('settings.agents.title', 'Agents')}</CardTitle>
          <CardDescription>
            {t(
              'settings.agents.description',
              'Edit config.yaml, then reload to apply changes.'
            )}
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-center gap-2">
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
    </div>
  );
}
