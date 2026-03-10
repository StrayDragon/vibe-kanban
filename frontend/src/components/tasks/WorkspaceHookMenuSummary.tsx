import { Clock3, Wrench, XCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/ui/badge';
import type {
  Workspace,
  WorkspaceLifecycleHookPhase,
  WorkspaceLifecycleHookStatus,
} from 'shared/types';
import { getWorkspaceHookOutcome } from '@/utils/workspaceHooks';

type WorkspaceHookSurface = Pick<
  Workspace,
  | 'latest_hook_run'
  | 'after_prepare_hook_status'
  | 'after_prepare_hook_ran_at'
  | 'after_prepare_hook_error_summary'
  | 'before_cleanup_hook_status'
  | 'before_cleanup_hook_ran_at'
  | 'before_cleanup_hook_error_summary'
>;

interface WorkspaceHookMenuSummaryProps {
  workspace?: WorkspaceHookSurface | null;
}

function getStatusVariant(status?: WorkspaceLifecycleHookStatus | null) {
  if (status === 'succeeded') {
    return 'secondary' as const;
  }
  if (status === 'failed') {
    return 'destructive' as const;
  }
  return 'outline' as const;
}

function formatPhaseLabel(
  phase: WorkspaceLifecycleHookPhase,
  t: (key: string) => string
) {
  return phase === 'after_prepare'
    ? t('taskPanel.hooks.phases.afterPrepare')
    : t('taskPanel.hooks.phases.beforeCleanup');
}

function formatStatusLabel(
  status: WorkspaceLifecycleHookStatus,
  t: (key: string) => string
) {
  return status === 'succeeded'
    ? t('taskPanel.hooks.statuses.succeeded')
    : t('taskPanel.hooks.statuses.failed');
}

export function WorkspaceHookMenuSummary({
  workspace,
}: WorkspaceHookMenuSummaryProps) {
  const { t } = useTranslation('tasks');

  if (
    !workspace ||
    (!workspace.latest_hook_run &&
      !workspace.after_prepare_hook_status &&
      !workspace.before_cleanup_hook_status)
  ) {
    return null;
  }

  const latest = getWorkspaceHookOutcome(workspace);
  const entries = [
    {
      key: 'after_prepare',
      label: t('taskPanel.hooks.phases.afterPrepare'),
      icon: Wrench,
      status: workspace.after_prepare_hook_status,
      ranAt: workspace.after_prepare_hook_ran_at,
      errorSummary: workspace.after_prepare_hook_error_summary,
    },
    {
      key: 'before_cleanup',
      label: t('taskPanel.hooks.phases.beforeCleanup'),
      icon: Clock3,
      status: workspace.before_cleanup_hook_status,
      ranAt: workspace.before_cleanup_hook_ran_at,
      errorSummary: workspace.before_cleanup_hook_error_summary,
    },
  ] as const;

  return (
    <div className="rounded-lg border border-border/60 bg-background/80 p-3 space-y-3">
      {latest ? (
        <div className="flex items-center justify-between gap-3">
          <div className="min-w-0">
            <div className="text-xs font-medium text-muted-foreground">
              {t('taskPanel.hooks.subtitle')}
            </div>
          </div>
          <Badge variant={getStatusVariant(latest.status)}>
            {formatPhaseLabel(latest.phase, t)} ·{' '}
            {formatStatusLabel(latest.status, t)}
          </Badge>
        </div>
      ) : null}

      <div className="space-y-2">
        {entries.map((entry) => {
          const Icon = entry.icon;
          return (
            <div
              key={entry.key}
              className="rounded-md border bg-card px-3 py-2"
            >
              <div className="flex items-center justify-between gap-3">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <Icon className="h-3.5 w-3.5 text-muted-foreground" />
                  <span>{entry.label}</span>
                </div>
                <Badge variant={getStatusVariant(entry.status)}>
                  {entry.status
                    ? formatStatusLabel(entry.status, t)
                    : t('taskPanel.hooks.notRun')}
                </Badge>
              </div>
              {entry.ranAt ? (
                <div className="mt-1 text-xs text-muted-foreground">
                  {t('taskPanel.hooks.ranAt', {
                    time: new Date(entry.ranAt).toLocaleString(),
                  })}
                </div>
              ) : (
                <div className="mt-1 text-xs text-muted-foreground">
                  {t('taskPanel.hooks.notRunDescription')}
                </div>
              )}
              {entry.errorSummary ? (
                <div className="mt-2 flex items-start gap-2 text-xs text-destructive">
                  <XCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                  <span className="whitespace-pre-wrap break-words">
                    {entry.errorSummary}
                  </span>
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
    </div>
  );
}
