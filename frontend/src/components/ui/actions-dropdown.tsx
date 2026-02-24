import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { MoreHorizontal } from 'lucide-react';
import type { TaskWithAttemptStatus } from 'shared/types';
import type { Workspace } from 'shared/types';
import { useOpenInEditor } from '@/hooks/task-attempts/useOpenInEditor';
import { DeleteTaskConfirmationDialog } from '@/components/dialogs/tasks/DeleteTaskConfirmationDialog';
import { ViewProcessesDialog } from '@/components/dialogs/tasks/ViewProcessesDialog';
import { ViewRelatedTasksDialog } from '@/components/dialogs/tasks/ViewRelatedTasksDialog';
import { CreateAttemptDialog } from '@/components/dialogs/tasks/CreateAttemptDialog';
import { GitActionsDialog } from '@/components/dialogs/tasks/GitActionsDialog';
import { EditBranchNameDialog } from '@/components/dialogs/tasks/EditBranchNameDialog';
import { RemoveWorktreeDialog } from '@/components/dialogs/tasks/RemoveWorktreeDialog';
import { useProject } from '@/contexts/ProjectContext';
import { openTaskForm } from '@/lib/openTaskForm';
import { useExecutionProcesses } from '@/hooks/execution-processes/useExecutionProcesses';
import { useTaskAttempts } from '@/hooks/task-attempts/useTaskAttempts';

import { useLocation, useNavigate } from 'react-router-dom';

interface ActionsDropdownProps {
  task?: TaskWithAttemptStatus | null;
  attempt?: Workspace | null;
  context?: 'task' | 'attempt' | 'card';
}

export function ActionsDropdown({
  task,
  attempt,
  context = 'card',
}: ActionsDropdownProps) {
  const { t } = useTranslation('tasks');
  const { projectId } = useProject();
  const openInEditor = useOpenInEditor(attempt?.id);
  const navigate = useNavigate();
  const location = useLocation();
  const isOverviewRoute = location.pathname.startsWith('/tasks');
  const enableRemoveWorktree = context === 'task' || context === 'attempt';
  const attemptIdForStatus =
    context === 'attempt' && attempt?.id ? attempt.id : undefined;
  const { executionProcesses, isLoading: isExecutionLoading } =
    useExecutionProcesses(attemptIdForStatus, {
      showSoftDeleted: true,
    });
  const { data: attempts = [] } = useTaskAttempts(task?.id, {
    enabled: Boolean(context === 'task' && task?.id),
    refetchInterval: false,
  });

  const hasAttemptActions = Boolean(attempt);
  const hasTaskActions = Boolean(task);
  const eligibleAttempts = useMemo(() => {
    if (context !== 'task' || attempt) return [];
    return attempts.filter((attemptData) => attemptData.container_ref);
  }, [attempt, attempts, context]);
  const hasEligibleAttempt =
    context === 'attempt'
      ? Boolean(attempt?.container_ref)
      : eligibleAttempts.length > 0;
  const showRemoveWorktree = Boolean(
    enableRemoveWorktree && hasEligibleAttempt
  );
  const showAttemptSection = hasAttemptActions || showRemoveWorktree;
  const hasRunningProcesses = executionProcesses.some(
    (process) => process.status === 'running'
  );
  const removeWorktreeDisabled =
    Boolean(attempt) && (isExecutionLoading || hasRunningProcesses);

  const handleEdit = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!projectId || !task) return;
    openTaskForm({ mode: 'edit', projectId, task });
  };

  const handleDuplicate = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!projectId || !task) return;
    openTaskForm({ mode: 'duplicate', projectId, initialTask: task });
  };

  const handleDelete = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!projectId || !task) return;
    try {
      await DeleteTaskConfirmationDialog.show({
        task,
        projectId,
      });
    } catch {
      // User cancelled or error occurred
    }
  };

  const handleOpenInEditor = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!attempt?.id) return;
    openInEditor();
  };

  const handleViewProcesses = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!attempt?.id) return;
    ViewProcessesDialog.show({ attemptId: attempt.id });
  };

  const handleViewRelatedTasks = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!attempt?.id || !projectId) return;
    ViewRelatedTasksDialog.show({
      attemptId: attempt.id,
      projectId,
      attempt,
      allowCreateSubtask: !isOverviewRoute,
      onNavigateToTask: (taskId: string) => {
        if (projectId) {
          navigate(`/projects/${projectId}/tasks/${taskId}/attempts/latest`);
        }
      },
    });
  };

  const handleCreateNewAttempt = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!task?.id) return;
    CreateAttemptDialog.show({
      taskId: task.id,
    });
  };

  const handleCreateSubtask = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!projectId || !attempt) return;
    const baseBranch = attempt.branch;
    if (!baseBranch) return;
    openTaskForm({
      mode: 'subtask',
      projectId,
      parentTaskAttemptId: attempt.id,
      initialBaseBranch: baseBranch,
    });
  };

  const handleGitActions = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!attempt?.id || !task) return;
    GitActionsDialog.show({
      attemptId: attempt.id,
      task,
    });
  };

  const handleEditBranchName = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!attempt?.id) return;
    EditBranchNameDialog.show({
      attemptId: attempt.id,
      currentBranchName: attempt.branch,
    });
  };

  const handleRemoveWorktree = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!task) return;
    RemoveWorktreeDialog.show({ task, attempt });
  };
  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="icon"
            aria-label="Actions"
            onPointerDown={(e) => e.stopPropagation()}
            onMouseDown={(e) => e.stopPropagation()}
            onClick={(e) => e.stopPropagation()}
          >
            <MoreHorizontal className="h-4 w-4" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          {showAttemptSection && (
            <>
              <DropdownMenuLabel>{t('actionsMenu.attempt')}</DropdownMenuLabel>
              {hasAttemptActions && (
                <>
                  <DropdownMenuItem
                    disabled={!attempt?.id}
                    onClick={handleOpenInEditor}
                  >
                    {t('actionsMenu.openInIde')}
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    disabled={!attempt?.id}
                    onClick={handleViewProcesses}
                  >
                    {t('actionsMenu.viewProcesses')}
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    disabled={!attempt?.id}
                    onClick={handleViewRelatedTasks}
                  >
                    {t('actionsMenu.viewRelatedTasks')}
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={handleCreateNewAttempt}>
                    {t('actionsMenu.createNewAttempt')}
                  </DropdownMenuItem>
                  {!isOverviewRoute && (
                    <DropdownMenuItem
                      disabled={!projectId || !attempt}
                      onClick={handleCreateSubtask}
                    >
                      {t('actionsMenu.createSubtask')}
                    </DropdownMenuItem>
                  )}
                  <DropdownMenuItem
                    disabled={!attempt?.id || !task}
                    onClick={handleGitActions}
                  >
                    {t('actionsMenu.gitActions')}
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    disabled={!attempt?.id}
                    onClick={handleEditBranchName}
                  >
                    {t('actionsMenu.editBranchName')}
                  </DropdownMenuItem>
                </>
              )}
              {showRemoveWorktree && (
                <DropdownMenuItem
                  disabled={removeWorktreeDisabled}
                  onClick={handleRemoveWorktree}
                  title={
                    hasRunningProcesses
                      ? t('removeWorktreeDialog.runningDisabled')
                      : undefined
                  }
                  className="text-destructive"
                >
                  {t('actionsMenu.removeWorktree')}
                </DropdownMenuItem>
              )}
              <DropdownMenuSeparator />
            </>
          )}

          {hasTaskActions && (
            <>
              <DropdownMenuLabel>{t('actionsMenu.task')}</DropdownMenuLabel>
              <DropdownMenuItem disabled={!projectId} onClick={handleEdit}>
                {t('common:buttons.edit')}
              </DropdownMenuItem>
              <DropdownMenuItem disabled={!projectId} onClick={handleDuplicate}>
                {t('actionsMenu.duplicate')}
              </DropdownMenuItem>
              <DropdownMenuItem
                disabled={!projectId}
                onClick={handleDelete}
                className="text-destructive"
              >
                {t('common:buttons.delete')}
              </DropdownMenuItem>
            </>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  );
}
