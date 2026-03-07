import { useCallback, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  AlertTriangle,
  Bot,
  CheckCircle2,
  Eye,
  ExternalLink,
  RotateCcw,
  UserRound,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { TaskWithAttemptStatus } from 'shared/types';
import type { WorkspaceWithSession } from '@/types/attempt';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import { attemptsApi } from '@/lib/api';
import { useNavigateWithSearch } from '@/hooks';
import { paths } from '@/lib/paths';
import { useDiffSummary } from '@/hooks/task-attempts/useDiffSummary';
import { useTaskMutations } from '@/hooks/tasks/useTaskMutations';
import {
  getResumeAutomationMode,
  getTaskAutomationOwnerKey,
  getTaskAutomationOwnershipPresentation,
  getTaskAutomationRuntimePresentation,
} from '@/utils/automation';

interface TaskHandoffCardProps {
  task: TaskWithAttemptStatus;
  latestAttempt?: WorkspaceWithSession;
  projectId?: string;
  buildAttemptPath?: (
    projectId: string,
    taskId: string,
    attemptId: string
  ) => string;
}

function formatTimeAgo(iso: string): string {
  const date = new Date(iso);
  const diffMs = Date.now() - date.getTime();
  const absSec = Math.round(Math.abs(diffMs) / 1000);

  const rtf =
    typeof Intl !== 'undefined' && typeof Intl.RelativeTimeFormat === 'function'
      ? new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' })
      : null;

  const to = (value: number, unit: Intl.RelativeTimeFormatUnit) =>
    rtf
      ? rtf.format(-value, unit)
      : `${value} ${unit}${value !== 1 ? 's' : ''} ago`;

  if (absSec < 60) return to(Math.round(absSec), 'second');
  const minutes = Math.round(absSec / 60);
  if (minutes < 60) return to(minutes, 'minute');
  const hours = Math.round(minutes / 60);
  if (hours < 24) return to(hours, 'hour');
  const days = Math.round(hours / 24);
  if (days < 30) return to(days, 'day');
  const months = Math.round(days / 30);
  if (months < 12) return to(months, 'month');
  const years = Math.round(months / 12);
  return to(years, 'year');
}

export function TaskHandoffCard({
  task,
  latestAttempt,
  projectId,
  buildAttemptPath,
}: TaskHandoffCardProps) {
  const { t } = useTranslation('tasks');
  const navigate = useNavigateWithSearch();
  const ownerKey = getTaskAutomationOwnerKey(task);
  const ownership = getTaskAutomationOwnershipPresentation(task);
  const runtime = getTaskAutomationRuntimePresentation(task);
  const { updateTask } = useTaskMutations(projectId);
  const { fileCount, added, deleted } = useDiffSummary(
    latestAttempt?.id ?? null
  );

  const attemptPath = buildAttemptPath ?? paths.attempt;
  const latestAttemptId = latestAttempt?.id;
  const reviewable =
    ownerKey === 'needs_review' ||
    task.status === 'inreview' ||
    ownerKey === 'blocked' ||
    task.last_attempt_failed;

  const { data: attemptStatus } = useQuery({
    queryKey: ['taskAttemptStatus', latestAttemptId],
    queryFn: () => attemptsApi.getStatus(latestAttemptId!),
    enabled: !!latestAttemptId,
  });

  const updateCurrentTask = useCallback(
    (
      patch: Partial<Pick<TaskWithAttemptStatus, 'status' | 'automation_mode'>>
    ) => {
      updateTask.mutate({
        taskId: task.id,
        data: {
          title: task.title,
          description: task.description,
          status: patch.status ?? task.status,
          automation_mode: patch.automation_mode ?? task.automation_mode,
          parent_workspace_id: task.parent_workspace_id,
          image_ids: null,
        },
      });
    },
    [task, updateTask]
  );

  const statusLabel = useMemo(() => {
    const state = attemptStatus?.state;
    if (!state) return null;
    return t(`taskPanel.handoff.attemptStates.${state}`);
  }, [attemptStatus?.state, t]);

  if (!latestAttempt || (!reviewable && ownerKey === 'manual')) {
    return null;
  }

  return (
    <div className="rounded-xl border border-border/70 bg-background/80 p-4 space-y-4">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div className="space-y-1">
          <div className="flex items-center gap-2 text-sm font-semibold">
            <Eye className="h-4 w-4 text-amber-500" />
            {t('taskPanel.handoff.title')}
          </div>
          <p className="text-sm text-muted-foreground max-w-2xl">
            {t('taskPanel.handoff.subtitle')}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Badge
            variant={ownership.variant}
            className={cn('gap-1.5', ownership.className)}
          >
            {ownerKey === 'manual' ? (
              <UserRound className="h-3.5 w-3.5" />
            ) : ownerKey === 'managed' ? (
              <Bot className="h-3.5 w-3.5" />
            ) : ownerKey === 'blocked' ? (
              <AlertTriangle className="h-3.5 w-3.5" />
            ) : (
              <Eye className="h-3.5 w-3.5" />
            )}
            {ownership.label}
          </Badge>
          {runtime && (
            <Badge variant={runtime.variant} className={runtime.className}>
              {runtime.label}
            </Badge>
          )}
        </div>
      </div>

      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <div className="rounded-lg border border-border/60 bg-muted/20 p-3">
          <div className="text-[11px] font-medium uppercase tracking-[0.12em] text-muted-foreground">
            {t('taskPanel.handoff.latestAttempt')}
          </div>
          <div className="mt-1 text-sm font-medium truncate">
            {latestAttempt.branch || '—'}
          </div>
          <div className="mt-1 text-xs text-muted-foreground">
            {latestAttempt.session?.executor || t('attempt.agent')}
          </div>
        </div>
        <div className="rounded-lg border border-border/60 bg-muted/20 p-3">
          <div className="text-[11px] font-medium uppercase tracking-[0.12em] text-muted-foreground">
            {t('taskPanel.handoff.updated')}
          </div>
          <div className="mt-1 text-sm font-medium">
            {formatTimeAgo(latestAttempt.updated_at)}
          </div>
          <div className="mt-1 text-xs text-muted-foreground">
            {new Date(latestAttempt.updated_at).toLocaleString()}
          </div>
        </div>
        <div className="rounded-lg border border-border/60 bg-muted/20 p-3">
          <div className="text-[11px] font-medium uppercase tracking-[0.12em] text-muted-foreground">
            {t('taskPanel.handoff.diffSummary')}
          </div>
          <div className="mt-1 text-sm font-medium">
            {t('taskPanel.handoff.filesChanged', { count: fileCount })}
          </div>
          <div className="mt-1 text-xs text-muted-foreground">
            +{added} / -{deleted}
          </div>
        </div>
        <div className="rounded-lg border border-border/60 bg-muted/20 p-3">
          <div className="text-[11px] font-medium uppercase tracking-[0.12em] text-muted-foreground">
            {t('taskPanel.handoff.attemptStatus')}
          </div>
          <div className="mt-1 text-sm font-medium">
            {statusLabel ?? t('taskPanel.handoff.unknownStatus')}
          </div>
          {attemptStatus?.last_activity_at && (
            <div className="mt-1 text-xs text-muted-foreground">
              {t('taskPanel.handoff.lastActivity')}:{' '}
              {formatTimeAgo(attemptStatus.last_activity_at)}
            </div>
          )}
        </div>
      </div>

      <div className="space-y-2">
        <div className="text-xs font-medium uppercase tracking-[0.12em] text-muted-foreground">
          {t('taskPanel.handoff.contextTitle')}
        </div>
        <div className="text-sm text-muted-foreground space-y-1">
          <p>{ownership.detail}</p>
          {runtime?.detail && <p>{runtime.detail}</p>}
          {attemptStatus?.failure_summary && (
            <p>
              <span className="font-medium text-foreground">
                {t('taskPanel.handoff.failureSummary')}:&nbsp;
              </span>
              {attemptStatus.failure_summary}
            </p>
          )}
          {task.automation_diagnostic?.reason_detail && (
            <p>
              <span className="font-medium text-foreground">
                {t('taskPanel.handoff.schedulerNote')}:&nbsp;
              </span>
              {task.automation_diagnostic.reason_detail}
            </p>
          )}
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {projectId && latestAttemptId && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() =>
              navigate(attemptPath(projectId, task.id, latestAttemptId))
            }
          >
            <ExternalLink className="mr-2 h-4 w-4" />
            {t('taskPanel.handoff.openLatestAttempt')}
          </Button>
        )}
        {reviewable && (
          <Button
            type="button"
            size="sm"
            onClick={() => updateCurrentTask({ status: 'done' })}
            disabled={updateTask.isPending}
          >
            <CheckCircle2 className="mr-2 h-4 w-4" />
            {t('taskPanel.handoff.actions.approve')}
          </Button>
        )}
        {reviewable && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => updateCurrentTask({ status: 'todo' })}
            disabled={updateTask.isPending}
          >
            <RotateCcw className="mr-2 h-4 w-4" />
            {t('taskPanel.handoff.actions.rework')}
          </Button>
        )}
        {task.automation_mode !== 'manual' && ownerKey !== 'manual' && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => updateCurrentTask({ automation_mode: 'manual' })}
            disabled={updateTask.isPending}
          >
            <UserRound className="mr-2 h-4 w-4" />
            {t('taskPanel.handoff.actions.takeOver')}
          </Button>
        )}
        {task.automation_mode === 'manual' && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() =>
              updateCurrentTask({
                automation_mode: getResumeAutomationMode(task),
              })
            }
            disabled={updateTask.isPending}
          >
            <Bot className="mr-2 h-4 w-4" />
            {t('taskPanel.handoff.actions.resumeAuto')}
          </Button>
        )}
      </div>
    </div>
  );
}
