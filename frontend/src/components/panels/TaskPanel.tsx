import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useProject } from '@/contexts/ProjectContext';
import { useTaskAttemptsWithSessions } from '@/hooks/task-attempts/useTaskAttempts';
import { useTaskAttemptWithSession } from '@/hooks/task-attempts/useTaskAttempt';
import { useNavigateWithSearch } from '@/hooks';
import { useTaskMutations } from '@/hooks/tasks/useTaskMutations';
import { paths } from '@/lib/paths';
import type { TaskWithAttemptStatus } from 'shared/types';
import type { WorkspaceWithSession } from '@/types/attempt';
import { NewCardContent } from '../ui/new-card';
import { Button } from '../ui/button';
import { PlusIcon } from 'lucide-react';
import { CreateAttemptDialog } from '@/components/dialogs/tasks/CreateAttemptDialog';
import WYSIWYGEditor from '@/components/ui/wysiwyg';
import { DataTable, type ColumnDef } from '@/components/ui/table';
import { TaskLineageCard } from '@/components/tasks/TaskLineageCard';
import { Badge } from '@/components/ui/badge';

interface TaskPanelProps {
  task: TaskWithAttemptStatus | null;
  projectId?: string;
  buildTaskPath?: (projectId: string, taskId: string) => string;
  buildAttemptPath?: (
    projectId: string,
    taskId: string,
    attemptId: string
  ) => string;
}

const TaskPanel = ({
  task,
  projectId: projectIdOverride,
  buildTaskPath,
  buildAttemptPath,
}: TaskPanelProps) => {
  const { t } = useTranslation('tasks');
  const navigate = useNavigateWithSearch();
  const { projectId } = useProject();
  const resolvedProjectId = projectIdOverride ?? projectId;
  const attemptPath = buildAttemptPath ?? paths.attempt;
  const { updateTask } = useTaskMutations(resolvedProjectId);

  const {
    data: attempts = [],
    isLoading: isAttemptsLoading,
    isError: isAttemptsError,
  } = useTaskAttemptsWithSessions(task?.id);

  const { data: parentAttempt, isLoading: isParentLoading } =
    useTaskAttemptWithSession(task?.parent_workspace_id || undefined);

  const formatTimeAgo = (iso: string) => {
    const d = new Date(iso);
    const diffMs = Date.now() - d.getTime();
    const absSec = Math.round(Math.abs(diffMs) / 1000);

    const rtf =
      typeof Intl !== 'undefined' &&
      typeof Intl.RelativeTimeFormat === 'function'
        ? new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' })
        : null;

    const to = (value: number, unit: Intl.RelativeTimeFormatUnit) =>
      rtf
        ? rtf.format(-value, unit)
        : `${value} ${unit}${value !== 1 ? 's' : ''} ago`;

    if (absSec < 60) return to(Math.round(absSec), 'second');
    const mins = Math.round(absSec / 60);
    if (mins < 60) return to(mins, 'minute');
    const hours = Math.round(mins / 60);
    if (hours < 24) return to(hours, 'hour');
    const days = Math.round(hours / 24);
    if (days < 30) return to(days, 'day');
    const months = Math.round(days / 30);
    if (months < 12) return to(months, 'month');
    const years = Math.round(months / 12);
    return to(years, 'year');
  };

  const displayedAttempts = [...attempts].sort(
    (a, b) =>
      new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
  );

  const handleStartAttempt = useCallback(() => {
    if (!task?.id) return;
    CreateAttemptDialog.show({ taskId: task.id });
  }, [task?.id]);

  if (!task) {
    return (
      <div className="text-muted-foreground">
        {t('taskPanel.noTaskSelected')}
      </div>
    );
  }

  const isMilestoneEntry = task.task_kind === 'milestone';
  const titleContent = `# ${task.title || 'Task'}`;
  const descriptionContent = task.description || '';

  const attemptColumns: ColumnDef<WorkspaceWithSession>[] = [
    {
      id: 'executor',
      header: '',
      accessor: (attempt) => attempt.session?.executor || 'Base Agent',
      className: 'pr-4',
    },
    {
      id: 'branch',
      header: '',
      accessor: (attempt) => attempt.branch || '—',
      className: 'pr-4',
    },
    {
      id: 'time',
      header: '',
      accessor: (attempt) => formatTimeAgo(attempt.created_at),
      className: 'pr-0 text-right',
    },
  ];

  return (
    <NewCardContent>
      <div className="p-6 flex flex-col h-full max-h-[calc(100vh-8rem)]">
        <div className="space-y-4 overflow-y-auto flex-shrink min-h-0">
          <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border bg-card px-4 py-3">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="outline">{task.status}</Badge>
              {task.dispatch_state?.status === 'awaiting_human_review' && (
                <Badge variant="secondary">
                  {t('taskPanel.needsReview', 'Needs review')}
                </Badge>
              )}
              {task.last_attempt_failed && (
                <Badge variant="destructive">
                  {t('taskPanel.lastAttemptFailed', 'Last attempt failed')}
                </Badge>
              )}
            </div>
            <div className="flex flex-wrap items-center gap-2">
              {task.milestone_id && resolvedProjectId && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    navigate(
                      paths.milestoneWorkflow(resolvedProjectId, task.milestone_id!)
                    )
                  }
                >
                  {t('openMilestone', 'Open milestone')}
                </Button>
              )}
              {!isMilestoneEntry && (
                <Button
                  size="sm"
                  onClick={handleStartAttempt}
                  disabled={updateTask.isPending}
                >
                  {t('actionsMenu.createNewAttempt')}
                </Button>
              )}
            </div>
          </div>

          <WYSIWYGEditor value={titleContent} disabled taskId={task.id} />
          {descriptionContent && (
            <WYSIWYGEditor
              value={descriptionContent}
              disabled
              taskId={task.id}
            />
          )}

          <TaskLineageCard task={task} buildTaskPath={buildTaskPath} />
        </div>

        <div className="mt-6 flex-shrink-0 space-y-4">
          {task.parent_workspace_id && (
            <DataTable
              data={parentAttempt ? [parentAttempt] : []}
              columns={attemptColumns}
              keyExtractor={(attempt) => attempt.id}
              onRowClick={(attempt) => {
                if (resolvedProjectId) {
                  navigate(
                    attemptPath(resolvedProjectId, attempt.task_id, attempt.id)
                  );
                }
              }}
              isLoading={isParentLoading}
              headerContent={t('taskPanel.parentAttempt')}
            />
          )}

          {isAttemptsLoading ? (
            <div className="text-muted-foreground">
              {t('taskPanel.loadingAttempts')}
            </div>
          ) : isAttemptsError ? (
            <div className="text-destructive">
              {t('taskPanel.errorLoadingAttempts')}
            </div>
          ) : (
            <DataTable
              data={displayedAttempts}
              columns={attemptColumns}
              keyExtractor={(attempt) => attempt.id}
              onRowClick={(attempt) => {
                if (resolvedProjectId && task.id) {
                  navigate(attemptPath(resolvedProjectId, task.id, attempt.id));
                }
              }}
              emptyState={t('taskPanel.noAttempts')}
              headerContent={
                <div className="w-full flex text-left">
                  <span className="flex-1">
                    {t('taskPanel.attemptsCount', {
                      count: displayedAttempts.length,
                    })}
                  </span>
                  {!isMilestoneEntry && (
                    <span>
                      <Button
                        variant="icon"
                        onClick={() =>
                          CreateAttemptDialog.show({
                            taskId: task.id,
                          })
                        }
                      >
                        <PlusIcon size={16} />
                      </Button>
                    </span>
                  )}
                </div>
              }
            />
          )}
        </div>
      </div>
    </NewCardContent>
  );
};

export default TaskPanel;
