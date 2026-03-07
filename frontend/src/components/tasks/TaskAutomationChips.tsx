import {
  AlertTriangle,
  Bot,
  Clock3,
  Eye,
  Loader2,
  UserRound,
} from 'lucide-react';
import type { TaskWithAttemptStatus } from 'shared/types';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import {
  getTaskOwnershipPresentation,
  getTaskRuntimePresentation,
} from '@/utils/automation';

interface TaskAutomationChipsProps {
  task: TaskWithAttemptStatus;
  className?: string;
  compact?: boolean;
}

const ownershipStyles = {
  manual:
    'border-slate-500/30 bg-slate-500/10 text-slate-700 dark:text-slate-200',
  managed:
    'border-indigo-500/30 bg-indigo-500/10 text-indigo-700 dark:text-indigo-200',
  needs_review:
    'border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-200',
  blocked: 'border-red-500/30 bg-red-500/10 text-red-700 dark:text-red-200',
} as const;

function OwnershipIcon({ task }: { task: TaskWithAttemptStatus }) {
  const ownership = getTaskOwnershipPresentation(task);

  switch (ownership.kind) {
    case 'managed':
      return <Bot className="h-3.5 w-3.5" />;
    case 'needs_review':
      return <Eye className="h-3.5 w-3.5" />;
    case 'blocked':
      return <AlertTriangle className="h-3.5 w-3.5" />;
    case 'manual':
    default:
      return <UserRound className="h-3.5 w-3.5" />;
  }
}

function RuntimeIcon({ task }: { task: TaskWithAttemptStatus }) {
  const runtime = getTaskRuntimePresentation(task);

  if (!runtime) {
    return null;
  }

  if (runtime.label === 'Running' || runtime.label === 'In Progress') {
    return <Loader2 className="h-3.5 w-3.5 animate-spin" />;
  }

  if (runtime.label === 'Queued' || runtime.label === 'Retry Scheduled') {
    return <Clock3 className="h-3.5 w-3.5" />;
  }

  if (runtime.label === 'Awaiting Review') {
    return <Eye className="h-3.5 w-3.5" />;
  }

  if (runtime.variant === 'destructive') {
    return <AlertTriangle className="h-3.5 w-3.5" />;
  }

  return null;
}

export function TaskAutomationChips({
  task,
  className,
  compact = false,
}: TaskAutomationChipsProps) {
  const ownership = getTaskOwnershipPresentation(task);
  const runtime = getTaskRuntimePresentation(task);

  return (
    <div className={cn('flex flex-wrap items-center gap-2', className)}>
      <Badge
        variant="outline"
        className={cn(
          'w-fit gap-1.5 border font-medium',
          compact && 'px-2 py-0.5 text-[11px]',
          ownership.kind ? ownershipStyles[ownership.kind] : undefined
        )}
      >
        <OwnershipIcon task={task} />
        {ownership.label}
      </Badge>
      {runtime && (
        <Badge
          variant={runtime.variant}
          className={cn(
            'w-fit gap-1.5',
            compact && 'px-2 py-0.5 text-[11px]',
            runtime.className
          )}
        >
          <RuntimeIcon task={task} />
          {runtime.label}
        </Badge>
      )}
    </div>
  );
}
