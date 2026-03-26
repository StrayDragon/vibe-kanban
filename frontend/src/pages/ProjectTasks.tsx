import { useCallback, useEffect, useMemo, useRef } from 'react';
import { useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { AlertTriangle, Plus } from 'lucide-react';
import { Loader } from '@/components/ui/loader';
import { ApiError, tasksApi } from '@/lib/api';
import type { RepoBranchStatus, Workspace } from 'shared/types';
import { openTaskForm } from '@/lib/openTaskForm';

import { useSearch } from '@/contexts/SearchContext';
import { useProject } from '@/contexts/ProjectContext';
import { useTaskAttempts } from '@/hooks/task-attempts/useTaskAttempts';
import { useTaskAttemptWithSession } from '@/hooks/task-attempts/useTaskAttempt';
import { useMediaQuery } from '@/hooks/utils/useMediaQuery';
import { useBranchStatus, useAttemptExecution } from '@/hooks';
import { useLayoutMode } from '@/hooks/utils/useLayoutMode';
import { paths } from '@/lib/paths';
import { ExecutionProcessesProvider } from '@/contexts/ExecutionProcessesContext';
import { ClickedElementsProvider } from '@/contexts/ClickedElementsProvider';
import { ReviewProvider } from '@/contexts/ReviewProvider';
import {
  GitOperationsProvider,
  useGitOperationsError,
} from '@/contexts/GitOperationsContext';
import {
  useKeyCreate,
  useKeyExit,
  useKeyFocusSearch,
  useKeyNavUp,
  useKeyNavDown,
  useKeyNavLeft,
  useKeyNavRight,
  useKeyOpenDetails,
  Scope,
  useKeyDeleteTask,
  useKeyCycleViewBackward,
} from '@/keyboard';

import TaskKanbanBoard, {
  type KanbanColumnItem,
} from '@/components/tasks/TaskKanbanBoard';
import type { DragEndEvent } from '@/components/ui/shadcn-io/kanban';
import { useProjectTasks } from '@/hooks/projects/useProjectTasks';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { useHotkeysContext } from 'react-hotkeys-hook';
import { TasksLayout, type LayoutMode } from '@/components/layout/TasksLayout';
import { PreviewPanel } from '@/components/panels/PreviewPanel';
import { DiffsPanel } from '@/components/panels/DiffsPanel';
import TaskAttemptPanel from '@/components/panels/TaskAttemptPanel';
import TaskPanel from '@/components/panels/TaskPanel';
import TodoPanel from '@/components/tasks/TodoPanel';
import { AttemptCompletionTasksResync } from '@/components/tasks/AttemptCompletionTasksResync';
import { NewCard, NewCardHeader } from '@/components/ui/new-card';
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbList,
  BreadcrumbLink,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb';
import { AttemptHeaderActions } from '@/components/panels/AttemptHeaderActions';
import { TaskPanelHeaderActions } from '@/components/panels/TaskPanelHeaderActions';
import {
  getMilestoneId,
  isMilestoneEntry,
  isMilestoneSubtask,
} from '@/utils/milestone';
import { toast } from '@/components/ui/toast';
import { useOptimisticTasksStore } from '@/stores/useOptimisticTasksStore';

import type { TaskWithAttemptStatus, TaskStatus } from 'shared/types';

type Task = TaskWithAttemptStatus;

const TASK_STATUSES = [
  'todo',
  'inprogress',
  'inreview',
  'done',
  'cancelled',
] as const;

const normalizeStatus = (status: string): TaskStatus =>
  status.toLowerCase() as TaskStatus;

function GitErrorBanner() {
  const { error: gitError } = useGitOperationsError();

  if (!gitError) return null;

  return (
    <div className="mx-4 mt-4 p-3 border border-destructive rounded">
      <div className="text-destructive text-sm">{gitError}</div>
    </div>
  );
}

function DiffsPanelContainer({
  attempt,
  selectedTask,
  branchStatus,
}: {
  attempt: Workspace | null;
  selectedTask: TaskWithAttemptStatus | null;
  branchStatus: RepoBranchStatus[] | null;
}) {
  const { isAttemptRunning } = useAttemptExecution(attempt?.id);

  return (
    <DiffsPanel
      key={attempt?.id}
      selectedAttempt={attempt}
      gitOps={
        attempt && selectedTask && !isMilestoneEntry(selectedTask)
          ? {
              task: selectedTask,
              branchStatus: branchStatus ?? null,
              isAttemptRunning,
              selectedBranch: branchStatus?.[0]?.target_branch_name ?? null,
            }
          : undefined
      }
    />
  );
}

export function ProjectTasks() {
  const { t } = useTranslation(['tasks', 'common', 'projects']);
  const { taskId, attemptId } = useParams<{
    projectId: string;
    taskId?: string;
    attemptId?: string;
  }>();
  const navigate = useNavigate();
  const { enableScope, disableScope, activeScopes } = useHotkeysContext();
  const [searchParams, setSearchParams] = useSearchParams();
  const isXL = useMediaQuery('(min-width: 1280px)');
  const isMobile = !isXL;

  const {
    projectId,
    project,
    isLoading: projectLoading,
    error: projectError,
  } = useProject();

  useEffect(() => {
    enableScope(Scope.KANBAN);

    return () => {
      disableScope(Scope.KANBAN);
    };
  }, [enableScope, disableScope]);

  const handleCreateTask = useCallback(() => {
    if (projectId) {
      openTaskForm({ mode: 'create', projectId });
    }
  }, [projectId]);

  const handleCreateMilestone = useCallback(() => {
    if (projectId) {
      openTaskForm({ mode: 'create', projectId, kind: 'milestone' });
    }
  }, [projectId]);
  const {
    query: searchQuery,
    focusInput,
    clear: clearSearch,
    setReviewInbox,
    clearReviewInbox,
  } = useSearch();

  const {
    tasks,
    tasksById,
    tasksByStatus,
    isLoading,
    error: streamError,
    resync: resyncTasks,
  } = useProjectTasks(projectId || '');

  const selectedTask = useMemo(
    () => (taskId ? (tasksById[taskId] ?? null) : null),
    [taskId, tasksById]
  );
  const selectedMilestoneId = useMemo(
    () => (selectedTask ? getMilestoneId(selectedTask) : null),
    [selectedTask]
  );

  const isPanelOpen = Boolean(taskId && selectedTask);

  const isLatest = attemptId === 'latest';
  const { data: attempts = [], isLoading: isAttemptsLoading } = useTaskAttempts(
    taskId,
    {
      enabled: !!taskId && isLatest,
    }
  );

  const latestAttemptId = useMemo(() => {
    if (!attempts?.length) return undefined;
    return [...attempts].sort((a, b) => {
      const diff =
        new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
      if (diff !== 0) return diff;
      return a.id.localeCompare(b.id);
    })[0].id;
  }, [attempts]);

  const navigateWithSearch = useCallback(
    (pathname: string, options?: { replace?: boolean }) => {
      const search = searchParams.toString();
      navigate({ pathname, search: search ? `?${search}` : '' }, options);
    },
    [navigate, searchParams]
  );

  useEffect(() => {
    if (!projectId || !taskId) return;
    if (selectedMilestoneId) return;
    if (!isLatest) return;
    if (isAttemptsLoading) return;

    if (!latestAttemptId) {
      navigateWithSearch(paths.task(projectId, taskId), { replace: true });
      return;
    }

    navigateWithSearch(paths.attempt(projectId, taskId, latestAttemptId), {
      replace: true,
    });
  }, [
    projectId,
    taskId,
    isLatest,
    isAttemptsLoading,
    latestAttemptId,
    navigate,
    navigateWithSearch,
    selectedMilestoneId,
  ]);

  useEffect(() => {
    if (!selectedMilestoneId || !projectId) return;
    navigateWithSearch(
      paths.milestoneWorkflow(projectId, selectedMilestoneId),
      {
        replace: true,
      }
    );
  }, [navigateWithSearch, projectId, selectedMilestoneId]);

  useEffect(() => {
    if (!projectId || !taskId || isLoading) return;
    if (selectedTask === null) {
      navigate(`/projects/${projectId}/tasks`, { replace: true });
    }
  }, [projectId, taskId, isLoading, selectedTask, navigate]);

  const effectiveAttemptId = attemptId === 'latest' ? undefined : attemptId;
  const isTaskView = !!taskId && !effectiveAttemptId;
  const attemptQuery = useTaskAttemptWithSession(effectiveAttemptId);
  const attemptNotFound =
    attemptQuery.error instanceof ApiError &&
    attemptQuery.error.statusCode === 404;
  const attemptNotFoundMessage =
    attemptNotFound && attemptQuery.error instanceof Error
      ? attemptQuery.error.message
      : undefined;
  const attempt = attemptNotFound ? undefined : attemptQuery.data;

  useEffect(() => {
    if (!attemptNotFound) return;
    if (!projectId || !taskId || !effectiveAttemptId) return;
    toast({
      variant: 'destructive',
      title: t('common:notFound.title', 'Not found'),
      description: attemptNotFoundMessage,
    });
    navigateWithSearch(paths.task(projectId, taskId), { replace: true });
  }, [
    attemptNotFoundMessage,
    attemptNotFound,
    effectiveAttemptId,
    navigateWithSearch,
    projectId,
    taskId,
    t,
  ]);

  const { data: branchStatus } = useBranchStatus(attempt?.id);

  const { mode, setMode } = useLayoutMode(searchParams, setSearchParams);

  const handleCreateNewTask = useCallback(() => {
    handleCreateTask();
  }, [handleCreateTask]);

  useKeyCreate(handleCreateNewTask, {
    scope: Scope.KANBAN,
    preventDefault: true,
  });

  useKeyFocusSearch(
    () => {
      focusInput();
    },
    {
      scope: Scope.KANBAN,
      preventDefault: true,
    }
  );

  useKeyExit(
    () => {
      if (isPanelOpen) {
        handleClosePanel();
      } else {
        navigate('/tasks');
      }
    },
    { scope: Scope.KANBAN }
  );

  const hasSearch = Boolean(searchQuery.trim());
  const normalizedSearch = searchQuery.trim().toLowerCase();

  const matchesTaskSearch = useCallback(
    (task: Task): boolean => {
      if (!hasSearch) return true;
      const lowerTitle = task.title.toLowerCase();
      const lowerDescription = task.description?.toLowerCase() ?? '';
      return (
        lowerTitle.includes(normalizedSearch) ||
        lowerDescription.includes(normalizedSearch)
      );
    },
    [hasSearch, normalizedSearch]
  );

  const orchestrationScopeTasks = useMemo(
    () =>
      tasks.filter(
        (task) => !isMilestoneSubtask(task) && matchesTaskSearch(task)
      ),
    [matchesTaskSearch, tasks]
  );

  const kanbanColumns = useMemo(() => {
    const columns: Record<TaskStatus, KanbanColumnItem[]> = {
      todo: [],
      inprogress: [],
      inreview: [],
      done: [],
      cancelled: [],
    };

    (Object.entries(tasksByStatus) as [TaskStatus, Task[]][]).forEach(
      ([status, items]) => {
        if (!columns[status]) {
          columns[status] = [];
        }
        items.forEach((task) => {
          if (isMilestoneSubtask(task)) {
            return;
          }
          if (!matchesTaskSearch(task)) {
            return;
          }

          columns[status].push(task);
        });
      }
    );

    const getTimestamp = (item: KanbanColumnItem) => {
      return new Date(item.created_at).getTime();
    };

    TASK_STATUSES.forEach((status) => {
      columns[status].sort((a, b) => getTimestamp(b) - getTimestamp(a));
    });

    return columns;
  }, [matchesTaskSearch, tasksByStatus]);

  const hasVisibleTasks = useMemo(
    () =>
      Object.values(kanbanColumns).some((items) => items && items.length > 0),
    [kanbanColumns]
  );

  useKeyNavUp(
    () => {
      selectPreviousTask();
    },
    {
      scope: Scope.KANBAN,
      preventDefault: true,
    }
  );

  useKeyNavDown(
    () => {
      selectNextTask();
    },
    {
      scope: Scope.KANBAN,
      preventDefault: true,
    }
  );

  useKeyNavLeft(
    () => {
      selectPreviousColumn();
    },
    {
      scope: Scope.KANBAN,
      preventDefault: true,
    }
  );

  useKeyNavRight(
    () => {
      selectNextColumn();
    },
    {
      scope: Scope.KANBAN,
      preventDefault: true,
    }
  );

  /**
   * Cycle the attempt area view.
   * - When panel is closed: opens task details (if a task is selected)
   * - When panel is open: cycles among [attempt, preview, diffs]
   */
  const cycleView = useCallback(
    (direction: 'forward' | 'backward' = 'forward') => {
      const order: LayoutMode[] = [null, 'preview', 'diffs'];
      const idx = order.indexOf(mode);
      const next =
        direction === 'forward'
          ? order[(idx + 1) % order.length]
          : order[(idx - 1 + order.length) % order.length];
      setMode(next);
    },
    [mode, setMode]
  );

  const cycleViewForward = useCallback(() => cycleView('forward'), [cycleView]);
  const cycleViewBackward = useCallback(
    () => cycleView('backward'),
    [cycleView]
  );

  // meta/ctrl+enter → open details or cycle forward
  const isFollowUpReadyActive = activeScopes.includes(Scope.FOLLOW_UP_READY);

  useKeyOpenDetails(
    () => {
      if (isPanelOpen) {
        cycleViewForward();
      } else if (selectedTask) {
        handleViewTaskDetails(selectedTask);
      }
    },
    { scope: Scope.KANBAN, when: () => !isFollowUpReadyActive }
  );

  // meta/ctrl+shift+enter → cycle backward
  useKeyCycleViewBackward(
    () => {
      if (isPanelOpen) {
        cycleViewBackward();
      }
    },
    { scope: Scope.KANBAN, preventDefault: true }
  );

  useKeyDeleteTask(
    () => {
      // Note: Delete is now handled by TaskActionsDropdown
      // This keyboard shortcut could trigger the dropdown action if needed
    },
    {
      scope: Scope.KANBAN,
      preventDefault: true,
    }
  );

  const handleClosePanel = useCallback(() => {
    if (projectId) {
      navigate(`/projects/${projectId}/tasks`, { replace: true });
    }
  }, [projectId, navigate]);

  const handleViewTaskDetails = useCallback(
    (task: Task, attemptIdToShow?: string) => {
      if (!projectId) return;
      const milestoneId = getMilestoneId(task);
      if (milestoneId) {
        navigateWithSearch(paths.milestoneWorkflow(projectId, milestoneId));
        return;
      }

      if (attemptIdToShow) {
        navigateWithSearch(paths.attempt(projectId, task.id, attemptIdToShow));
      } else {
        navigateWithSearch(`${paths.task(projectId, task.id)}/attempts/latest`);
      }
    },
    [projectId, navigateWithSearch]
  );

  const selectNextTask = useCallback(() => {
    if (selectedTask) {
      const statusKey = normalizeStatus(selectedTask.status);
      const tasksInStatus = kanbanColumns[statusKey] || [];
      const currentIndex = tasksInStatus.findIndex(
        (task) => task.id === selectedTask.id
      );
      if (currentIndex >= 0 && currentIndex < tasksInStatus.length - 1) {
        handleViewTaskDetails(tasksInStatus[currentIndex + 1]);
      }
    } else {
      for (const status of TASK_STATUSES) {
        const tasks = kanbanColumns[status];
        if (tasks && tasks.length > 0) {
          handleViewTaskDetails(tasks[0]);
          break;
        }
      }
    }
  }, [selectedTask, kanbanColumns, handleViewTaskDetails]);

  const selectPreviousTask = useCallback(() => {
    if (selectedTask) {
      const statusKey = normalizeStatus(selectedTask.status);
      const tasksInStatus = kanbanColumns[statusKey] || [];
      const currentIndex = tasksInStatus.findIndex(
        (task) => task.id === selectedTask.id
      );
      if (currentIndex > 0) {
        handleViewTaskDetails(tasksInStatus[currentIndex - 1]);
      }
    } else {
      for (const status of TASK_STATUSES) {
        const tasks = kanbanColumns[status];
        if (tasks && tasks.length > 0) {
          handleViewTaskDetails(tasks[0]);
          break;
        }
      }
    }
  }, [selectedTask, kanbanColumns, handleViewTaskDetails]);

  const selectNextColumn = useCallback(() => {
    if (selectedTask) {
      const currentStatus = normalizeStatus(selectedTask.status);
      const currentIndex = TASK_STATUSES.findIndex(
        (status) => status === currentStatus
      );
      for (let i = currentIndex + 1; i < TASK_STATUSES.length; i++) {
        const tasks = kanbanColumns[TASK_STATUSES[i]];
        if (tasks && tasks.length > 0) {
          handleViewTaskDetails(tasks[0]);
          return;
        }
      }
    } else {
      for (const status of TASK_STATUSES) {
        const tasks = kanbanColumns[status];
        if (tasks && tasks.length > 0) {
          handleViewTaskDetails(tasks[0]);
          break;
        }
      }
    }
  }, [selectedTask, kanbanColumns, handleViewTaskDetails]);

  const selectPreviousColumn = useCallback(() => {
    if (selectedTask) {
      const currentStatus = normalizeStatus(selectedTask.status);
      const currentIndex = TASK_STATUSES.findIndex(
        (status) => status === currentStatus
      );
      for (let i = currentIndex - 1; i >= 0; i--) {
        const tasks = kanbanColumns[TASK_STATUSES[i]];
        if (tasks && tasks.length > 0) {
          handleViewTaskDetails(tasks[0]);
          return;
        }
      }
    } else {
      for (const status of TASK_STATUSES) {
        const tasks = kanbanColumns[status];
        if (tasks && tasks.length > 0) {
          handleViewTaskDetails(tasks[0]);
          break;
        }
      }
    }
  }, [selectedTask, kanbanColumns, handleViewTaskDetails]);

  const reviewInboxTasks = useMemo(
    () =>
      orchestrationScopeTasks.filter(
        (task) =>
          task.status === 'inreview' ||
          task.dispatch_state?.status === 'awaiting_human_review'
      ),
    [orchestrationScopeTasks]
  );

  useEffect(() => {
    setReviewInbox({
      tasks: reviewInboxTasks,
      onSelectTask: handleViewTaskDetails,
    });

    return () => {
      clearReviewInbox();
    };
  }, [
    clearReviewInbox,
    handleViewTaskDetails,
    reviewInboxTasks,
    setReviewInbox,
  ]);

  const dragSeqRef = useRef<Record<string, number>>({});

  const handleDragEnd = useCallback(
    async (event: DragEndEvent) => {
      const { active, over } = event;
      if (!over || !active.data.current) return;

      const draggedTaskId = active.id as string;
      const newStatus = over.id as Task['status'];
      const task = tasksById[draggedTaskId];
      if (!task || task.status === newStatus) return;

      const seq = (dragSeqRef.current[draggedTaskId] ?? 0) + 1;
      dragSeqRef.current[draggedTaskId] = seq;

      const store = useOptimisticTasksStore.getState();
      const snapshot = store.getSnapshot(draggedTaskId);
      const baseUpdatedAtMs = Date.parse(task.updated_at);
      store.setOverride(
        draggedTaskId,
        { status: newStatus },
        {
          baseUpdatedAtMs: Number.isFinite(baseUpdatedAtMs)
            ? baseUpdatedAtMs
            : null,
        }
      );

      try {
        await tasksApi.update(draggedTaskId, {
          title: task.title,
          description: task.description,
          status: newStatus,
          parent_workspace_id: task.parent_workspace_id,
          image_ids: null,
          continuation_turns_override: task.continuation_turns_override,
        });
      } catch (err) {
        console.error('Failed to update task status:', err);
        if (dragSeqRef.current[draggedTaskId] !== seq) return;
        store.restoreSnapshot(draggedTaskId, snapshot);
        toast({
          variant: 'destructive',
          title: t('common:states.error'),
          description: t('tasks:errors.updateStatusFailed'),
        });
      }
    },
    [t, tasksById]
  );

  const isInitialTasksLoad = isLoading && tasks.length === 0;
  const isProjectOrphaned = Boolean(
    projectId && !projectLoading && !project && !projectError
  );

  if (projectError) {
    return (
      <div className="p-4">
        <Alert>
          <AlertTitle className="flex items-center gap-2">
            <AlertTriangle size="16" />
            {t('common:states.error')}
          </AlertTitle>
          <AlertDescription>
            {projectError.message || t('errors.loadProjectFailed')}
          </AlertDescription>
        </Alert>
      </div>
    );
  }

  if (projectLoading && isInitialTasksLoad) {
    return <Loader message={t('loading')} size={32} className="py-8" />;
  }

  const truncateTitle = (title: string | undefined, maxLength = 20) => {
    if (!title) return t('labels.task');
    if (title.length <= maxLength) return title;

    const truncated = title.substring(0, maxLength);
    const lastSpace = truncated.lastIndexOf(' ');

    return lastSpace > 0
      ? `${truncated.substring(0, lastSpace)}...`
      : `${truncated}...`;
  };

  const hasActiveTaskFilters = Boolean(searchQuery.trim());

  const handleResetVisibleFilters = () => {
    clearSearch();
  };

  const kanbanBody =
    tasks.length === 0 ? (
      <div className="max-w-7xl mx-auto mt-8">
        <Card>
          <CardContent className="text-center py-8">
            <p className="text-muted-foreground">{t('empty.noTasks')}</p>
            <div className="mt-4 flex flex-wrap items-center justify-center gap-2">
              <Button onClick={handleCreateNewTask}>
                <Plus className="h-4 w-4 mr-2" />
                {t('empty.createFirst')}
              </Button>
              <Button variant="outline" onClick={handleCreateMilestone}>
                <Plus className="h-4 w-4 mr-2" />
                {t('actions.createMilestone', 'Create milestone')}
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    ) : !hasVisibleTasks ? (
      <div className="max-w-7xl mx-auto mt-8">
        <Card>
          <CardContent className="text-center py-8">
            <p className="text-muted-foreground">
              {t('empty.noFilteredResults')}
            </p>
            {hasActiveTaskFilters && (
              <Button
                variant="outline"
                className="mt-4"
                onClick={handleResetVisibleFilters}
              >
                {t('common:buttons.reset')}
              </Button>
            )}
          </CardContent>
        </Card>
      </div>
    ) : (
      <div className="w-full h-full overflow-x-auto overflow-y-auto overscroll-x-contain">
        <TaskKanbanBoard
          columns={kanbanColumns}
          onDragEnd={handleDragEnd}
          onViewTaskDetails={handleViewTaskDetails}
          selectedTaskId={selectedTask?.id}
          onCreateTask={handleCreateNewTask}
          onCreateMilestone={handleCreateMilestone}
          projectId={projectId!}
        />
      </div>
    );

  const kanbanContent = (
    <div className="flex h-full min-h-0 flex-col">
      <div className="min-h-0 flex-1">{kanbanBody}</div>
    </div>
  );

  const rightHeader = selectedTask ? (
    <NewCardHeader
      className="shrink-0"
      actions={
        isTaskView ? (
          <TaskPanelHeaderActions
            task={selectedTask}
            onClose={() =>
              navigate(`/projects/${projectId}/tasks`, { replace: true })
            }
          />
        ) : (
          <AttemptHeaderActions
            mode={mode}
            onModeChange={setMode}
            task={selectedTask}
            attempt={attempt ?? null}
            onClose={() =>
              navigate(`/projects/${projectId}/tasks`, { replace: true })
            }
          />
        )
      }
    >
      <div className="mx-auto w-full">
        <Breadcrumb>
          <BreadcrumbList>
            <BreadcrumbItem>
              {isTaskView ? (
                <BreadcrumbPage>
                  {truncateTitle(selectedTask?.title)}
                </BreadcrumbPage>
              ) : (
                <BreadcrumbLink
                  className="cursor-pointer hover:underline"
                  onClick={() =>
                    navigateWithSearch(paths.task(projectId!, taskId!))
                  }
                >
                  {truncateTitle(selectedTask?.title)}
                </BreadcrumbLink>
              )}
            </BreadcrumbItem>
            {!isTaskView && (
              <>
                <BreadcrumbSeparator />
                <BreadcrumbItem>
                  <BreadcrumbPage>
                    {attempt?.branch || 'Task Attempt'}
                  </BreadcrumbPage>
                </BreadcrumbItem>
              </>
            )}
          </BreadcrumbList>
        </Breadcrumb>
      </div>
    </NewCardHeader>
  ) : null;

  const attemptContent = selectedTask ? (
    <NewCard className="h-full min-h-0 flex flex-col bg-diagonal-lines bg-muted border-0">
      {isTaskView ? (
        <TaskPanel task={selectedTask} buildTaskPath={paths.task} />
      ) : (
        <TaskAttemptPanel attempt={attempt} task={selectedTask}>
          {({ logs, followUp }) => (
            <>
              <GitErrorBanner />
              <div className="flex-1 min-h-0 flex flex-col">
                <div className="flex-1 min-h-0 flex flex-col">{logs}</div>

                <div className="shrink-0 border-t">
                  <div className="mx-auto w-full max-w-[50rem]">
                    <TodoPanel />
                  </div>
                </div>

                <div className="min-h-0 max-h-[50%] border-t overflow-hidden bg-background">
                  <div className="mx-auto w-full max-w-[50rem] h-full min-h-0">
                    {followUp}
                  </div>
                </div>
              </div>
            </>
          )}
        </TaskAttemptPanel>
      )}
    </NewCard>
  ) : null;

  const auxContent =
    selectedTask && attempt ? (
      <div className="relative h-full w-full">
        {mode === 'preview' && <PreviewPanel />}
        {mode === 'diffs' && (
          <DiffsPanelContainer
            attempt={attempt}
            selectedTask={selectedTask}
            branchStatus={branchStatus ?? null}
          />
        )}
      </div>
    ) : (
      <div className="relative h-full w-full" />
    );

  const attemptArea = (
    <GitOperationsProvider attemptId={attempt?.id}>
      <ClickedElementsProvider attempt={attempt}>
        <ReviewProvider attemptId={attempt?.id}>
          <ExecutionProcessesProvider attemptId={attempt?.id}>
            <AttemptCompletionTasksResync
              attemptId={attempt?.id}
              taskId={selectedTask?.id}
              taskStatus={selectedTask?.status}
              resyncTasks={resyncTasks}
            />
            <TasksLayout
              kanban={kanbanContent}
              attempt={attemptContent}
              aux={auxContent}
              isPanelOpen={isPanelOpen}
              mode={mode}
              isMobile={isMobile}
              rightHeader={rightHeader}
            />
          </ExecutionProcessesProvider>
        </ReviewProvider>
      </ClickedElementsProvider>
    </GitOperationsProvider>
  );

  return (
    <div className="min-h-full h-full flex flex-col">
      {streamError && (
        <Alert className="w-full z-30 xl:sticky xl:top-0">
          <AlertTitle className="flex items-center gap-2">
            <AlertTriangle size="16" />
            {t('common:states.reconnecting')}
          </AlertTitle>
          <AlertDescription>{streamError}</AlertDescription>
        </Alert>
      )}
      {isProjectOrphaned && (
        <Alert className="w-full z-20 xl:sticky xl:top-0">
          <AlertTitle className="flex items-center gap-2">
            <AlertTriangle size="16" />
            {t('projects:orphanedProject', 'Unknown project')}
          </AlertTitle>
          <AlertDescription>
            {t(
              'projects:orphanedProjectHint',
              'This project id is not present in projects.yaml. You can view historical tasks, but to run new work add it to projects.yaml (or projects.d) and reload.'
            )}{' '}
            <span className="font-mono">{projectId}</span>
          </AlertDescription>
        </Alert>
      )}

      <div className="flex-1 min-h-0">{attemptArea}</div>
    </div>
  );
}
