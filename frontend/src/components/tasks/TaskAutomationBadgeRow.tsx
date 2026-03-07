import {
  AlertTriangle,
  Bot,
  Clock3,
  Eye,
  PlayCircle,
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
  getTaskOrchestrationLane,
  getTaskOwnershipPresentation,
  getTaskRuntimePresentation,
} from '@/utils/automation';

function getOwnershipIcon(task: TaskWithAttemptStatus): LucideIcon {
  switch (getTaskOrchestrationLane(task)) {
    case 'managed':
      return Bot;
    case 'needs_review':
      return Eye;
    case 'blocked':
      return AlertTriangle;
    case 'manual':
    default:
      return UserRound;
  }
}

function getRuntimeIcon(task: TaskWithAttemptStatus): LucideIcon {
  const state = task.dispatch_state?.status;

  if (state === 'retry_scheduled') {
    return RotateCcw;
  }
  if (state === 'claimed') {
    return Clock3;
  }
  if (state === 'blocked') {
    return AlertTriangle;
  }
  if (task.last_attempt_failed) {
    return XCircle;
  }

  return PlayCircle;
}

interface TaskAutomationBadgeRowProps {
  task: TaskWithAttemptStatus;
  showDetail?: boolean;
  className?: string;
  detailClassName?: string;
}

export function TaskAutomationBadgeRow({
  task,
  showDetail = false,
  className,
  detailClassName,
}: TaskAutomationBadgeRowProps) {
  const ownership = getTaskOwnershipPresentation(task);
  const runtime = getTaskRuntimePresentation(task);
  const detail = getTaskAutomationDetail(task);
  const OwnershipIcon = getOwnershipIcon(task);
  const RuntimeIcon = runtime ? getRuntimeIcon(task) : null;

  return (
    <div className={cn('space-y-2', className)}>
      <div className="flex flex-wrap items-center gap-2">
        <Badge
          variant={ownership.variant}
          className={cn('gap-1.5', ownership.className)}
        >
          <OwnershipIcon className="h-3.5 w-3.5" />
          {ownership.label}
        </Badge>
        {runtime && RuntimeIcon && (
          <Badge
            variant={runtime.variant}
            className={cn('gap-1.5', runtime.className)}
          >
            <RuntimeIcon className="h-3.5 w-3.5" />
            {runtime.label}
          </Badge>
        )}
      </div>
      {showDetail && detail && (
        <p className={cn('text-xs text-muted-foreground', detailClassName)}>
          {detail}
        </p>
      )}
    </div>
  );
}
