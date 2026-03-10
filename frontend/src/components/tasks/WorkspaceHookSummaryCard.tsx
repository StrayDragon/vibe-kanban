import { useTranslation } from 'react-i18next';
import { CheckCircle2, Clock3, Wrench, XCircle } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import type {
  Workspace,
  WorkspaceLifecycleHookPhase,
  WorkspaceLifecycleHookStatus,
} from 'shared/types';

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

interface WorkspaceHookSummaryCardProps {
  workspace?: WorkspaceHookSurface | null;
  className?: string;
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

function getStatusBadgeVariant(status?: WorkspaceLifecycleHookStatus | null) {
  if (status === 'succeeded') {
    return 'secondary' as const;
  }
  if (status === 'failed') {
    return 'destructive' as const;
  }
  return 'outline' as const;
}

export function WorkspaceHookSummaryCard({
  workspace,
  className,
}: WorkspaceHookSummaryCardProps) {
  const { t } = useTranslation('tasks');

  if (
    !workspace ||
    (!workspace.latest_hook_run &&
      !workspace.after_prepare_hook_status &&
      !workspace.before_cleanup_hook_status)
  ) {
    return null;
  }

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
    <div
      className={cn(
        'rounded-lg border border-border/60 bg-card/80 p-4 space-y-3',
        className
      )}
    >
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="space-y-1">
          <div className="text-sm font-semibold">
            {t('taskPanel.hooks.title')}
          </div>
          <p className="text-sm text-muted-foreground">
            {t('taskPanel.hooks.subtitle')}
          </p>
        </div>
        {workspace.latest_hook_run ? (
          <Badge
            variant={getStatusBadgeVariant(workspace.latest_hook_run.status)}
          >
            {formatPhaseLabel(workspace.latest_hook_run.phase, t)} ·{' '}
            {formatStatusLabel(workspace.latest_hook_run.status, t)}
          </Badge>
        ) : null}
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        {entries.map((entry) => {
          const Icon = entry.icon;
          return (
            <div
              key={entry.key}
              className="rounded-md border bg-background px-3 py-3"
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <Icon className="h-4 w-4 text-muted-foreground" />
                  <span>{entry.label}</span>
                </div>
                <Badge variant={getStatusBadgeVariant(entry.status)}>
                  {entry.status
                    ? formatStatusLabel(entry.status, t)
                    : t('taskPanel.hooks.notRun')}
                </Badge>
              </div>

              {entry.ranAt ? (
                <div className="mt-2 text-xs text-muted-foreground">
                  {t('taskPanel.hooks.ranAt', {
                    time: new Date(entry.ranAt).toLocaleString(),
                  })}
                </div>
              ) : (
                <div className="mt-2 text-xs text-muted-foreground">
                  {t('taskPanel.hooks.notRunDescription')}
                </div>
              )}

              {entry.errorSummary ? (
                <div className="mt-3 flex items-start gap-2 rounded-md border border-destructive/20 bg-destructive/5 px-3 py-2 text-sm text-destructive">
                  <XCircle className="mt-0.5 h-4 w-4 shrink-0" />
                  <span className="whitespace-pre-wrap break-words">
                    {entry.errorSummary}
                  </span>
                </div>
              ) : entry.status === 'succeeded' ? (
                <div className="mt-3 flex items-start gap-2 rounded-md border border-emerald-500/20 bg-emerald-500/5 px-3 py-2 text-sm text-emerald-700 dark:text-emerald-300">
                  <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0" />
                  <span>{t('taskPanel.hooks.successDescription')}</span>
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
    </div>
  );
}
