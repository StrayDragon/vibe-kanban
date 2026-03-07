import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  ArrowRight,
  Bot,
  CheckCircle2,
  GitBranch,
  RotateCcw,
  UserRound,
} from 'lucide-react';
import type { TaskWithAttemptStatus } from 'shared/types';
import type { WorkspaceWithSession } from '@/types/attempt';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { sessionsApi } from '@/lib/api';
import { attemptsApi } from '@/lib/api';
import { useDiffSummary } from '@/hooks/task-attempts/useDiffSummary';
import {
  getTaskAutomationOwnerKey,
  getTaskAutomationRuntimePresentation,
  isTaskAwaitingHumanReview,
} from '@/utils/automation';

interface TaskAutomationHandoffCardProps {
  task: TaskWithAttemptStatus;
  latestAttempt?: WorkspaceWithSession;
  onApproveResult: () => void;
  onRequestRework: () => void;
  onTakeOverManually: () => void;
  onOpenLatestAttempt: () => void;
  isMutating?: boolean;
}

function formatTimeAgo(iso?: string | null) {
  if (!iso) return '—';
  const value = new Date(iso).getTime();
  if (Number.isNaN(value)) return '—';
  const diffMs = Date.now() - value;
  const mins = Math.max(1, Math.round(diffMs / 60000));
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.round(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  return `${days}d ago`;
}

export function TaskAutomationHandoffCard({
  task,
  latestAttempt,
  onApproveResult,
  onRequestRework,
  onTakeOverManually,
  onOpenLatestAttempt,
  isMutating = false,
}: TaskAutomationHandoffCardProps) {
  const ownerKey = getTaskAutomationOwnerKey(task);
  const runtime = getTaskAutomationRuntimePresentation(task);
  const shouldShow =
    !!latestAttempt &&
    (task.effective_automation_mode === 'auto' ||
      task.automation_mode === 'auto' ||
      ownerKey === 'needs_review' ||
      ownerKey === 'blocked');

  const {
    fileCount,
    added,
    deleted,
    error: diffError,
  } = useDiffSummary(latestAttempt?.id ?? null);

  const { data: attemptStatus } = useQuery({
    queryKey: ['taskAttemptStatus', latestAttempt?.id],
    queryFn: () => attemptsApi.getStatus(latestAttempt!.id),
    enabled: !!latestAttempt?.id,
  });

  const { data: sessionMessages } = useQuery({
    queryKey: ['sessionMessages', latestAttempt?.session?.id],
    queryFn: () =>
      sessionsApi.getMessages(latestAttempt!.session!.id, { limit: 10 }),
    enabled: !!latestAttempt?.session?.id,
  });

  const latestSummary = useMemo(() => {
    const entries = sessionMessages?.entries ?? [];
    return (
      entries.find((entry) => entry.summary?.trim())?.summary?.trim() ?? null
    );
  }, [sessionMessages?.entries]);

  if (!shouldShow || !latestAttempt) {
    return null;
  }

  const inReview = isTaskAwaitingHumanReview(task);

  return (
    <div className="rounded-lg border border-border/60 bg-card p-4 space-y-4">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div className="space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant={runtime?.variant ?? 'outline'}>
              {inReview ? 'Needs review' : 'Managed handoff'}
            </Badge>
            <Badge variant="outline">
              <GitBranch className="mr-1 h-3.5 w-3.5" />
              {latestAttempt.branch}
            </Badge>
            {latestAttempt.session?.executor && (
              <Badge variant="outline">
                <Bot className="mr-1 h-3.5 w-3.5" />
                {latestAttempt.session.executor}
              </Badge>
            )}
          </div>
          <div>
            <h3 className="text-sm font-semibold">
              {inReview ? 'Review handoff' : 'Latest managed outcome'}
            </h3>
            <p className="text-sm text-muted-foreground">
              {task.automation_diagnostic?.reason_detail ??
                'Review the latest result before deciding whether to approve, rework, or take over manually.'}
            </p>
          </div>
        </div>
        <Button variant="outline" size="sm" onClick={onOpenLatestAttempt}>
          Open latest attempt
          <ArrowRight className="ml-2 h-4 w-4" />
        </Button>
      </div>

      <div className="grid gap-3 md:grid-cols-3">
        <div className="rounded-md border bg-background px-3 py-2">
          <div className="text-xs text-muted-foreground">Latest state</div>
          <div className="mt-1 text-sm font-medium capitalize">
            {attemptStatus?.state ?? 'idle'}
          </div>
        </div>
        <div className="rounded-md border bg-background px-3 py-2">
          <div className="text-xs text-muted-foreground">Last updated</div>
          <div className="mt-1 text-sm font-medium">
            {formatTimeAgo(
              attemptStatus?.updated_at ?? latestAttempt.updated_at
            )}
          </div>
        </div>
        <div className="rounded-md border bg-background px-3 py-2">
          <div className="text-xs text-muted-foreground">Diff</div>
          <div className="mt-1 text-sm font-medium">
            {diffError
              ? 'Summary unavailable'
              : `${fileCount} files · +${added} / -${deleted}`}
          </div>
        </div>
      </div>

      <div className="space-y-2">
        <div>
          <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Latest agent summary
          </div>
          <p className="mt-1 text-sm text-foreground whitespace-pre-wrap">
            {latestSummary ??
              attemptStatus?.failure_summary ??
              'No agent summary is available yet. Open the latest attempt for full logs and artifacts.'}
          </p>
        </div>
        {attemptStatus?.failure_summary &&
          latestSummary !== attemptStatus.failure_summary && (
            <div>
              <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                Failure summary
              </div>
              <p className="mt-1 text-sm text-foreground whitespace-pre-wrap">
                {attemptStatus.failure_summary}
              </p>
            </div>
          )}
      </div>

      {inReview && (
        <div className="flex flex-wrap items-center gap-2">
          <Button size="sm" onClick={onApproveResult} disabled={isMutating}>
            <CheckCircle2 className="mr-2 h-4 w-4" />
            Approve result
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={onRequestRework}
            disabled={isMutating}
          >
            <RotateCcw className="mr-2 h-4 w-4" />
            Request rework
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={onTakeOverManually}
            disabled={isMutating}
          >
            <UserRound className="mr-2 h-4 w-4" />
            Take over manually
          </Button>
        </div>
      )}
    </div>
  );
}
