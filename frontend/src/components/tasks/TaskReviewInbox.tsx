import { ArrowRight, Inbox } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useState } from 'react';
import type { TaskWithAttemptStatus } from 'shared/types';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { cn } from '@/lib/utils';
import { statusBoardColors, statusLabels } from '@/utils/statusLabels';

interface TaskReviewInboxProps {
  tasks: TaskWithAttemptStatus[];
  onSelectTask: (task: TaskWithAttemptStatus) => void;
  projectNames?: Record<string, string>;
  className?: string;
}

export function TaskReviewInbox({
  tasks,
  onSelectTask,
  projectNames,
  className,
}: TaskReviewInboxProps) {
  const { t } = useTranslation('tasks');
  const [open, setOpen] = useState(false);

  if (tasks.length === 0) {
    return null;
  }

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className={cn('relative h-9 w-9', className)}
          aria-label={t('orchestration.reviewInbox.openDrawer')}
          title={t('orchestration.reviewInbox.openDrawer')}
        >
          <Inbox className="h-4 w-4" />
          <span className="absolute -right-0.5 -top-0.5 inline-flex min-h-4 min-w-4 items-center justify-center rounded-full border border-amber-200 bg-amber-50 px-1 text-[9px] font-semibold leading-none text-amber-700 dark:border-amber-900 dark:bg-amber-950/70 dark:text-amber-200">
            {tasks.length}
          </span>
        </Button>
      </DropdownMenuTrigger>

      <DropdownMenuContent
        align="end"
        className="w-[360px] max-w-[calc(100vw-1rem)] rounded-xl p-0"
        sideOffset={8}
      >
        <div className="p-3">
          <DropdownMenuLabel className="px-0 py-0 text-sm font-semibold">
            {t('orchestration.reviewInbox.title')}
          </DropdownMenuLabel>
          <p className="mt-1 text-xs text-muted-foreground">
            {t('orchestration.reviewInbox.subtitle', { count: tasks.length })}
          </p>
        </div>

        <DropdownMenuSeparator className="mx-0 my-0" />

        <div className="max-h-[min(70vh,480px)] overflow-y-auto p-2">
          <div className="space-y-2">
            {tasks.map((task) => {
              const projectName = projectNames?.[task.project_id];
              const statusColor = statusBoardColors[task.status];

              return (
                <DropdownMenuItem
                  key={task.id}
                  className="block cursor-pointer rounded-lg border bg-card p-0 focus:bg-transparent focus:text-foreground"
                  onSelect={() => onSelectTask(task)}
                >
                  <div className="rounded-lg px-3 py-3 text-left transition-colors duration-200 hover:bg-accent/40">
                    <div className="min-w-0">
                      <div className="text-sm font-medium leading-5 line-clamp-2">
                        {task.title || t('orchestration.reviewInbox.untitled')}
                      </div>
                      {projectName && (
                        <div className="mt-1 text-[11px] text-muted-foreground line-clamp-1">
                          {projectName}
                        </div>
                      )}
                    </div>

                    {task.automation_diagnostic?.reason_detail && (
                      <p className="mt-2 text-xs leading-5 text-muted-foreground line-clamp-2">
                        {task.automation_diagnostic.reason_detail}
                      </p>
                    )}

                    <div className="mt-3 flex items-center justify-between gap-2">
                      <Badge
                        variant="outline"
                        className="text-[10px] font-semibold uppercase tracking-[0.08em]"
                        style={{
                          color: `hsl(var(${statusColor}))`,
                          borderColor: `hsl(var(${statusColor}) / 0.4)`,
                          backgroundColor: `hsl(var(${statusColor}) / 0.08)`,
                        }}
                      >
                        {statusLabels[task.status]}
                      </Badge>
                      <span className="inline-flex items-center gap-1 text-xs font-medium text-foreground">
                        {t('orchestration.reviewInbox.reviewAction')}
                        <ArrowRight className="h-3.5 w-3.5" />
                      </span>
                    </div>
                  </div>
                </DropdownMenuItem>
              );
            })}
          </div>
        </div>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
