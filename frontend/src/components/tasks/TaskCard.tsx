import { useCallback, useEffect, useRef, useState } from 'react';
import { KanbanCard } from '@/components/ui/shadcn-io/kanban';
import { Link, Loader2, XCircle } from 'lucide-react';
import type { TaskWithAttemptStatus } from 'shared/types';
import { ActionsDropdown } from '@/components/ui/actions-dropdown';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { useNavigateWithSearch } from '@/hooks';
import { paths } from '@/lib/paths';
import { attemptsApi } from '@/lib/api';
import { TaskCardHeader } from './TaskCardHeader';
import { useTranslation } from 'react-i18next';
import { getTaskGroupId, isTaskGroupEntry } from '@/utils/taskGroup';

type Task = TaskWithAttemptStatus;

interface TaskCardProps {
  task: Task;
  index: number;
  status: string;
  onViewDetails: (task: Task) => void;
  isOpen?: boolean;
  projectId: string;
  groupSummary?: {
    subtaskCount: number;
  };
  groupTitle?: string;
}

export function TaskCard({
  task,
  index,
  status,
  onViewDetails,
  isOpen,
  projectId,
  groupSummary,
  groupTitle,
}: TaskCardProps) {
  const { t } = useTranslation('tasks');
  const navigate = useNavigateWithSearch();
  const [isNavigatingToParent, setIsNavigatingToParent] = useState(false);
  const isTaskGroup = isTaskGroupEntry(task);
  const taskGroupId = getTaskGroupId(task);
  const isGroupedTask = Boolean(taskGroupId) && !isTaskGroup;
  const displayTitle = isGroupedTask && groupTitle ? groupTitle : task.title;
  const showSubtaskTitle =
    isGroupedTask && groupTitle && groupTitle !== task.title;
  const typeLabel = isTaskGroup
    ? 'Task Group'
    : isGroupedTask
      ? 'Subtask'
      : 'Task';

  const handleClick = useCallback(() => {
    onViewDetails(task);
  }, [task, onViewDetails]);

  const handleParentClick = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!task.parent_workspace_id || isNavigatingToParent) return;

      setIsNavigatingToParent(true);
      try {
        const parentAttempt = await attemptsApi.get(task.parent_workspace_id);
        navigate(
          paths.attempt(
            projectId,
            parentAttempt.task_id,
            task.parent_workspace_id
          )
        );
      } catch (error) {
        console.error('Failed to navigate to parent task attempt:', error);
        setIsNavigatingToParent(false);
      }
    },
    [task.parent_workspace_id, projectId, navigate, isNavigatingToParent]
  );

  const handleOpenTaskGroup = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!taskGroupId) return;
      navigate(paths.taskGroupWorkflow(projectId, taskGroupId));
    },
    [navigate, projectId, taskGroupId]
  );

  const localRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isOpen || !localRef.current) return;
    const el = localRef.current;
    requestAnimationFrame(() => {
      el.scrollIntoView({
        block: 'center',
        inline: 'nearest',
        behavior: 'smooth',
      });
    });
  }, [isOpen]);

  return (
    <KanbanCard
      key={task.id}
      id={task.id}
      name={displayTitle}
      index={index}
      parent={status}
      onClick={handleClick}
      isOpen={isOpen}
      forwardedRef={localRef}
    >
      <div className="flex flex-col gap-2">
        <TaskCardHeader
          title={displayTitle}
          right={
            <>
              {task.has_in_progress_attempt && (
                <Loader2 className="h-4 w-4 animate-spin text-blue-500" />
              )}
              {task.last_attempt_failed && (
                <XCircle className="h-4 w-4 text-destructive" />
              )}
              {task.parent_workspace_id && (
                <Button
                  variant="icon"
                  onClick={handleParentClick}
                  onPointerDown={(e) => e.stopPropagation()}
                  onMouseDown={(e) => e.stopPropagation()}
                  disabled={isNavigatingToParent}
                  title={t('navigateToParent')}
                >
                  <Link className="h-4 w-4" />
                </Button>
              )}
              <ActionsDropdown task={task} />
            </>
          }
        />
        {showSubtaskTitle && (
          <div className="text-xs text-muted-foreground line-clamp-2">
            {task.title}
          </div>
        )}
        <Badge
          variant="outline"
          className="w-fit text-[10px] uppercase tracking-[0.12em] text-muted-foreground border-muted-foreground/40"
        >
          {typeLabel}
        </Badge>
        {groupSummary && groupSummary.subtaskCount > 0 && (
          <div className="text-xs text-muted-foreground">
            {t('taskGroupSubtaskCount', { count: groupSummary.subtaskCount })}
          </div>
        )}
        {isGroupedTask && (
          <Button
            variant="link"
            size="xs"
            className="h-auto p-0 text-xs text-muted-foreground hover:text-foreground"
            onClick={handleOpenTaskGroup}
            onPointerDown={(e) => e.stopPropagation()}
            onMouseDown={(e) => e.stopPropagation()}
            title={t('openTaskGroup')}
          >
            {t('openTaskGroup')}
          </Button>
        )}
        {task.description && (
          <p className="text-sm text-secondary-foreground break-words">
            {task.description.length > 130
              ? `${task.description.substring(0, 130)}...`
              : task.description}
          </p>
        )}
      </div>
    </KanbanCard>
  );
}
