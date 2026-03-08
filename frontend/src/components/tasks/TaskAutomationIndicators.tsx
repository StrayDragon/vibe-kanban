import {
  Activity,
  AlertTriangle,
  Bot,
  Clock3,
  Eye,
  RotateCcw,
  UserRound,
  XCircle,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { TaskWithAttemptStatus } from 'shared/types';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import {
  getTaskAutomationDetail,
  getTaskAutomationOwnershipPresentation,
  getTaskAutomationOwnerKey,
  getTaskAutomationRuntimePresentation,
} from '@/utils/automation';

const OWNER_ICONS: Record<string, LucideIcon> = {
  manual: UserRound,
  managed: Bot,
  needs_review: Eye,
  blocked: AlertTriangle,
};

function runtimeIcon(label: string): LucideIcon {
  switch (label) {
    case 'Running':
    case 'In Progress':
      return Activity;
    case 'Queued':
      return Clock3;
    case 'Retry Scheduled':
      return RotateCcw;
    case 'Blocked':
      return AlertTriangle;
    case 'Last Run Failed':
      return XCircle;
    case 'Needs Review':
      return Eye;
    default:
      return Activity;
  }
}

interface TaskAutomationIndicatorsProps {
  task: TaskWithAttemptStatus;
  showDetail?: boolean;
  className?: string;
  detailClassName?: string;
  hideReviewOwnership?: boolean;
}

export function TaskAutomationIndicators({
  task,
  showDetail = true,
  className,
  detailClassName,
  hideReviewOwnership = false,
}: TaskAutomationIndicatorsProps) {
  const shouldShowOrchestrationIndicators =
    task.project_execution_mode === 'auto' ||
    task.effective_automation_mode === 'auto';

  if (!shouldShowOrchestrationIndicators) {
    return null;
  }

  const ownership = getTaskAutomationOwnershipPresentation(task);
  const runtime = getTaskAutomationRuntimePresentation(task);
  const shouldHideRuntime =
    runtime?.label === 'In Progress' && task.status === 'inprogress';
  const visibleRuntime = shouldHideRuntime ? null : runtime;
  const OwnerIcon = OWNER_ICONS[getTaskAutomationOwnerKey(task)] ?? UserRound;
  const RuntimeIcon = visibleRuntime ? runtimeIcon(visibleRuntime.label) : null;
  const detail = getTaskAutomationDetail(task);
  const shouldHideOwnership =
    hideReviewOwnership && ownership.kind === 'needs_review';
  const shouldShowBadgeRow = !shouldHideOwnership || Boolean(visibleRuntime);
  const shouldShowDetail = showDetail && detail && !shouldHideOwnership;

  return (
    <div className={cn('space-y-2', className)}>
      {shouldShowBadgeRow && (
        <div className="flex flex-wrap items-center gap-2">
          {!shouldHideOwnership && (
            <Badge
              variant={ownership.variant}
              className={cn('gap-1.5', ownership.className)}
            >
              <OwnerIcon className="h-3.5 w-3.5" />
              {ownership.label}
            </Badge>
          )}
          {visibleRuntime && (
            <Badge
              variant={visibleRuntime.variant}
              className={cn('gap-1.5', visibleRuntime.className)}
            >
              {RuntimeIcon && <RuntimeIcon className="h-3.5 w-3.5" />}
              {visibleRuntime.label}
            </Badge>
          )}
        </div>
      )}
      {shouldShowDetail && (
        <p
          className={cn(
            'text-xs text-muted-foreground line-clamp-2',
            detailClassName
          )}
        >
          {detail}
        </p>
      )}
    </div>
  );
}
