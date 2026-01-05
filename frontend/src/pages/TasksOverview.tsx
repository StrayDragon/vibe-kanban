import { useCallback, useEffect, useMemo, useRef } from 'react';
import { useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useHotkeysContext } from 'react-hotkeys-hook';
import { AlertTriangle, Loader2, XCircle } from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Loader } from '@/components/ui/loader';
import { NewCard, NewCardHeader } from '@/components/ui/new-card';
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb';
import { TasksLayout, type LayoutMode } from '@/components/layout/TasksLayout';
import { AttemptHeaderActions } from '@/components/panels/AttemptHeaderActions';
import { TaskPanelHeaderActions } from '@/components/panels/TaskPanelHeaderActions';
import TaskAttemptPanel from '@/components/panels/TaskAttemptPanel';
import TaskPanel from '@/components/panels/TaskPanel';
import { PreviewPanel } from '@/components/panels/PreviewPanel';
import { DiffsPanel } from '@/components/panels/DiffsPanel';
import TodoPanel from '@/components/tasks/TodoPanel';
import { CreateAttemptDialog } from '@/components/dialogs/tasks/CreateAttemptDialog';

import { useSearch } from '@/contexts/SearchContext';
import { useProjects } from '@/hooks/useProjects';
import { useAllTasks } from '@/hooks/useAllTasks';
import { useMediaQuery } from '@/hooks/useMediaQuery';
import { useTaskAttempts } from '@/hooks/useTaskAttempts';
import { useTaskAttemptWithSession } from '@/hooks/useTaskAttempt';
import {
  useAttemptExecution,
  useBranchStatus,
  useNavigateWithSearch,
} from '@/hooks';
import {
  GitOperationsProvider,
  useGitOperationsError,
} from '@/contexts/GitOperationsContext';
import { ClickedElementsProvider } from '@/contexts/ClickedElementsProvider';
import { ReviewProvider } from '@/contexts/ReviewProvider';
import { ExecutionProcessesProvider } from '@/contexts/ExecutionProcessesContext';
import {
  Scope,
  useKeyCycleViewBackward,
  useKeyExit,
  useKeyFocusSearch,
  useKeyNavDown,
  useKeyNavUp,
  useKeyOpenDetails,
} from '@/keyboard';
import { cn } from '@/lib/utils';
import { paths } from '@/lib/paths';
import { statusBoardColors, statusLabels } from '@/utils/statusLabels';

import type {
  RepoBranchStatus,
  TaskStatus,
  TaskWithAttemptStatus,
  Workspace,
} from 'shared/types';

type Task = TaskWithAttemptStatus;

const STATUS_ORDER: TaskStatus[] = [
  'todo',
  'inprogress',
  'inreview',
  'done',
  'cancelled',
];

const STATUS_RANK = new Map(
  STATUS_ORDER.map((status, index) => [status, index])
);

const getStatusRank = (status: TaskStatus) => STATUS_RANK.get(status) ?? 0;

function TaskStatusBadge({
  status,
  count,
  className,
}: {
  status: TaskStatus;
  count?: number;
  className?: string;
}) {
  const colorVar = statusBoardColors[status];

  return (
    <Badge
      variant="outline"
      className={cn(
        'uppercase tracking-[0.08em] text-[10px] font-semibold',
        className
      )}
      style={{
        color: `hsl(var(${colorVar}))`,
        borderColor: `hsl(var(${colorVar}) / 0.4)`,
        backgroundColor: `hsl(var(${colorVar}) / 0.08)`,
      }}
    >
      {statusLabels[status]}
      {typeof count === 'number' && (
        <span className="ml-1 text-[10px] font-semibold">{count}</span>
      )}
    </Badge>
  );
}

function TaskListItem({
  task,
  isSelected,
  onSelect,
  agentLabel,
}: {
  task: Task;
  isSelected: boolean;
  onSelect: (task: Task) => void;
  agentLabel: string;
}) {
  const ref = useRef<HTMLButtonElement | null>(null);
  const description = task.description?.trim();
  const isMuted = task.status === 'done' || task.status === 'cancelled';

  useEffect(() => {
    if (!isSelected || !ref.current) return;
    ref.current.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }, [isSelected]);

  return (
    <button
      ref={ref}
      type="button"
      onClick={() => onSelect(task)}
      className={cn(
        'w-full text-left px-4 py-3 transition flex flex-col gap-2',
        isSelected ? 'bg-muted/70' : 'hover:bg-muted/40',
        isMuted && 'opacity-70'
      )}
      aria-selected={isSelected}
    >
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 min-w-0">
            <span className="text-sm font-medium line-clamp-1">
              {task.title || 'Task'}
            </span>
            {task.has_in_progress_attempt && (
              <Loader2 className="h-4 w-4 animate-spin text-blue-500" />
            )}
            {task.last_attempt_failed && (
              <XCircle className="h-4 w-4 text-destructive" />
            )}
          </div>
          {description && (
            <p className="text-xs text-muted-foreground line-clamp-2">
              {description}
            </p>
          )}
          {task.executor && (
            <p className="text-[11px] text-muted-foreground">
              {agentLabel}: {task.executor}
            </p>
          )}
        </div>
        <TaskStatusBadge status={task.status} />
      </div>
    </button>
  );
}

function GitErrorBanner() {
  const { error: gitError } = useGitOperationsError();

  if (!gitError) return null;

  return (
    <div className="mx-4 mt-4 p-3 border border-destructive rounded">
      <div className="text-destructive text-sm">{gitError}</div>
    </div>
  );
}

function NoAttemptsPanel({ taskId }: { taskId: string }) {
  const { t } = useTranslation('tasks');

  return (
    <div className="h-full w-full flex flex-col items-center justify-center gap-4 p-6 text-center">
      <p className="text-sm text-muted-foreground">
        {t('taskPanel.noAttempts')}
      </p>
      <Button onClick={() => CreateAttemptDialog.show({ taskId })}>
        {t('actionsMenu.createNewAttempt')}
      </Button>
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
        attempt && selectedTask
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

export function TasksOverview() {
  const { t } = useTranslation(['tasks', 'common']);
  const { projectId, taskId, attemptId } = useParams<{
    projectId?: string;
    taskId?: string;
    attemptId?: string;
  }>();
  const navigate = useNavigate();
  const navigateWithSearch = useNavigateWithSearch();
  const [searchParams, setSearchParams] = useSearchParams();
  const { enableScope, disableScope, activeScopes } = useHotkeysContext();
  const isXL = useMediaQuery('(min-width: 1280px)');
  const isMobile = !isXL;

  const { query: searchQuery, focusInput } = useSearch();
  const agentLabel = t('attempt.agent');
  const {
    projects,
    projectsById,
    error: projectsError,
  } = useProjects();
  const {
    tasks,
    tasksById,
    isLoading: tasksLoading,
    error: streamError,
  } = useAllTasks();

  useEffect(() => {
    enableScope(Scope.KANBAN);

    return () => {
      disableScope(Scope.KANBAN);
    };
  }, [enableScope, disableScope]);

  const selectedTask = useMemo(
    () => (taskId ? (tasksById[taskId] ?? null) : null),
    [taskId, tasksById]
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

  useEffect(() => {
    if (!projectId || !taskId) return;
    if (!isLatest) return;
    if (isAttemptsLoading) return;

    if (!latestAttemptId) return;

    navigateWithSearch(
      paths.overviewAttempt(projectId, taskId, latestAttemptId),
      { replace: true }
    );
  }, [
    projectId,
    taskId,
    isLatest,
    isAttemptsLoading,
    latestAttemptId,
    navigateWithSearch,
  ]);

  useEffect(() => {
    if (!taskId || tasksLoading) return;
    if (selectedTask === null) {
      navigate(paths.overview(), { replace: true });
    }
  }, [taskId, tasksLoading, selectedTask, navigate]);

  const isLatestAttemptRoute = attemptId === 'latest';
  const effectiveAttemptId = isLatestAttemptRoute ? undefined : attemptId;
  const isAttemptView = Boolean(attemptId);
  const isTaskView = !!taskId && !isAttemptView;
  const showNoAttemptsPanel =
    isLatestAttemptRoute && !isAttemptsLoading && !latestAttemptId;
  const { data: attempt } = useTaskAttemptWithSession(effectiveAttemptId);
  const { data: branchStatus } = useBranchStatus(attempt?.id);

  const rawMode = searchParams.get('view') as LayoutMode;
  const mode: LayoutMode =
    rawMode === 'preview' || rawMode === 'diffs' ? rawMode : null;
  const activeMode: LayoutMode = showNoAttemptsPanel ? null : mode;

  // TODO: Remove this redirect after v0.1.0 (legacy URL support for bookmarked links)
  // Migrates old `view=logs` to `view=diffs`
  useEffect(() => {
    const view = searchParams.get('view');
    if (view === 'logs') {
      const params = new URLSearchParams(searchParams);
      params.set('view', 'diffs');
      setSearchParams(params, { replace: true });
    }
  }, [searchParams, setSearchParams]);

  const setMode = useCallback(
    (newMode: LayoutMode) => {
      const params = new URLSearchParams(searchParams);
      if (newMode === null) {
        params.delete('view');
      } else {
        params.set('view', newMode);
      }
      setSearchParams(params, { replace: true });
    },
    [searchParams, setSearchParams]
  );

  const handleClosePanel = useCallback(() => {
    navigate(paths.overview(), { replace: true });
  }, [navigate]);

  const handleViewTaskDetails = useCallback(
    (task: Task, attemptIdToShow?: string) => {
      if (attemptIdToShow) {
        navigateWithSearch(
          paths.overviewAttempt(task.project_id, task.id, attemptIdToShow)
        );
      } else {
        navigateWithSearch(
          paths.overviewAttempt(task.project_id, task.id, 'latest')
        );
      }
    },
    [navigateWithSearch]
  );

  const hasSearch = Boolean(searchQuery.trim());
  const normalizedSearch = searchQuery.trim().toLowerCase();

  const filteredTasks = useMemo(() => {
    if (!hasSearch) return tasks;
    return tasks.filter((task) => {
      const title = task.title.toLowerCase();
      const description = task.description?.toLowerCase() ?? '';
      return (
        title.includes(normalizedSearch) ||
        description.includes(normalizedSearch)
      );
    });
  }, [hasSearch, normalizedSearch, tasks]);

  const tasksByProject = useMemo(() => {
    const grouped: Record<string, Task[]> = {};

    filteredTasks.forEach((task) => {
      if (!grouped[task.project_id]) {
        grouped[task.project_id] = [];
      }
      grouped[task.project_id].push(task);
    });

    Object.values(grouped).forEach((group) => {
      group.sort((a, b) => {
        if (a.has_in_progress_attempt !== b.has_in_progress_attempt) {
          return a.has_in_progress_attempt ? -1 : 1;
        }
        const statusDiff = getStatusRank(a.status) - getStatusRank(b.status);
        if (statusDiff !== 0) return statusDiff;
        return (
          new Date(b.created_at).getTime() -
          new Date(a.created_at).getTime()
        );
      });
    });

    return grouped;
  }, [filteredTasks]);

  const orderedProjectIds = useMemo(() => {
    const idsWithTasks = new Set(Object.keys(tasksByProject));
    const ordered = projects
      .map((project) => project.id)
      .filter((id) => idsWithTasks.has(id));
    const remaining = [...idsWithTasks].filter(
      (id) => !ordered.includes(id)
    );
    return [...ordered, ...remaining];
  }, [projects, tasksByProject]);

  const orderedTasks = useMemo(
    () =>
      orderedProjectIds.flatMap((id) => tasksByProject[id] ?? []),
    [orderedProjectIds, tasksByProject]
  );

  const selectNextTask = useCallback(() => {
    if (selectedTask) {
      const currentIndex = orderedTasks.findIndex(
        (task) => task.id === selectedTask.id
      );
      if (currentIndex >= 0 && currentIndex < orderedTasks.length - 1) {
        handleViewTaskDetails(orderedTasks[currentIndex + 1]);
      }
    } else if (orderedTasks.length > 0) {
      handleViewTaskDetails(orderedTasks[0]);
    }
  }, [selectedTask, orderedTasks, handleViewTaskDetails]);

  const selectPreviousTask = useCallback(() => {
    if (selectedTask) {
      const currentIndex = orderedTasks.findIndex(
        (task) => task.id === selectedTask.id
      );
      if (currentIndex > 0) {
        handleViewTaskDetails(orderedTasks[currentIndex - 1]);
      }
    } else if (orderedTasks.length > 0) {
      handleViewTaskDetails(orderedTasks[0]);
    }
  }, [selectedTask, orderedTasks, handleViewTaskDetails]);

  useKeyExit(
    () => {
      if (isPanelOpen) {
        handleClosePanel();
      } else {
        navigate('/projects');
      }
    },
    { scope: Scope.KANBAN }
  );

  useKeyFocusSearch(
    () => {
      focusInput();
    },
    {
      scope: Scope.KANBAN,
      preventDefault: true,
    }
  );

  useKeyNavUp(selectPreviousTask, {
    scope: Scope.KANBAN,
    preventDefault: true,
  });

  useKeyNavDown(selectNextTask, {
    scope: Scope.KANBAN,
    preventDefault: true,
  });

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

  const isFollowUpReadyActive = activeScopes.includes(Scope.FOLLOW_UP_READY);

  useKeyOpenDetails(
    () => {
      if (isPanelOpen) {
        cycleViewForward();
      } else if (selectedTask) {
        handleViewTaskDetails(selectedTask);
      } else if (orderedTasks.length > 0) {
        handleViewTaskDetails(orderedTasks[0]);
      }
    },
    { scope: Scope.KANBAN, when: () => !isFollowUpReadyActive }
  );

  useKeyCycleViewBackward(
    () => {
      if (isPanelOpen) {
        cycleViewBackward();
      }
    },
    { scope: Scope.KANBAN, preventDefault: true }
  );

  const isInitialTasksLoad = tasksLoading && tasks.length === 0;

  if (isInitialTasksLoad) {
    return <Loader message={t('loading')} size={32} className="py-8" />;
  }

  const hasVisibleTasks = filteredTasks.length > 0;

  const listContent =
    tasks.length === 0 ? (
      <div className="max-w-5xl mx-auto mt-8 px-6">
        <Card>
          <CardContent className="text-center py-8">
            <p className="text-muted-foreground">{t('overview.empty')}</p>
          </CardContent>
        </Card>
      </div>
    ) : !hasVisibleTasks ? (
      <div className="max-w-5xl mx-auto mt-8 px-6">
        <Card>
          <CardContent className="text-center py-8">
            <p className="text-muted-foreground">
              {t('overview.noSearchResults')}
            </p>
          </CardContent>
        </Card>
      </div>
    ) : (
      <div className="h-full w-full overflow-y-auto">
        <div className="max-w-5xl mx-auto px-6 py-6 space-y-6">
          <div className="space-y-1">
            <h1 className="text-2xl font-semibold tracking-tight">
              {t('overview.title')}
            </h1>
            <p className="text-sm text-muted-foreground">
              {t('overview.subtitle')}
            </p>
          </div>

          {orderedProjectIds.map((projectIdKey) => {
            const project = projectsById[projectIdKey];
            const projectTasks = tasksByProject[projectIdKey] ?? [];
            if (projectTasks.length === 0) return null;

            const statusCounts = projectTasks.reduce(
              (acc, task) => {
                acc[task.status] += 1;
                return acc;
              },
              {
                todo: 0,
                inprogress: 0,
                inreview: 0,
                done: 0,
                cancelled: 0,
              } as Record<TaskStatus, number>
            );

            return (
              <section key={projectIdKey} className="space-y-3">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <h2 className="text-sm font-semibold">
                      {project?.name ?? 'Unknown Project'}
                    </h2>
                    <span className="text-xs text-muted-foreground">
                      {projectTasks.length} tasks
                    </span>
                  </div>
                  <div className="flex flex-wrap items-center gap-2">
                    {STATUS_ORDER.map((status) =>
                      statusCounts[status] > 0 ? (
                        <TaskStatusBadge
                          key={status}
                          status={status}
                          count={statusCounts[status]}
                          className="text-[9px]"
                        />
                      ) : null
                    )}
                  </div>
                </div>

                <div className="rounded-lg border bg-card divide-y">
                  {projectTasks.map((task) => (
                    <TaskListItem
                      key={task.id}
                      task={task}
                      isSelected={selectedTask?.id === task.id}
                      onSelect={handleViewTaskDetails}
                      agentLabel={agentLabel}
                    />
                  ))}
                </div>
              </section>
            );
          })}
        </div>
      </div>
    );

  const truncateTitle = (title: string | undefined, maxLength = 20) => {
    if (!title) return 'Task';
    if (title.length <= maxLength) return title;

    const truncated = title.substring(0, maxLength);
    const lastSpace = truncated.lastIndexOf(' ');

    return lastSpace > 0
      ? `${truncated.substring(0, lastSpace)}...`
      : `${truncated}...`;
  };

  const projectName = selectedTask
    ? projectsById[selectedTask.project_id]?.name ?? 'Project'
    : 'Project';

  const rightHeader = selectedTask ? (
    <NewCardHeader
      className="shrink-0"
      actions={
        isTaskView ? (
          <TaskPanelHeaderActions
            task={selectedTask}
            onClose={handleClosePanel}
          />
        ) : (
          <AttemptHeaderActions
            mode={showNoAttemptsPanel ? undefined : activeMode}
            onModeChange={showNoAttemptsPanel ? undefined : setMode}
            task={selectedTask}
            attempt={attempt ?? null}
            onClose={handleClosePanel}
          />
        )
      }
    >
      <div className="mx-auto w-full flex items-center gap-3 min-w-0">
        <Breadcrumb>
          <BreadcrumbList>
            <BreadcrumbItem>
              <BreadcrumbPage>{projectName}</BreadcrumbPage>
            </BreadcrumbItem>
            <BreadcrumbSeparator />
            <BreadcrumbItem>
              {isTaskView ? (
                <BreadcrumbPage>
                  {truncateTitle(selectedTask?.title)}
                </BreadcrumbPage>
              ) : (
                <BreadcrumbLink
                  className="cursor-pointer hover:underline"
                  onClick={() =>
                    navigateWithSearch(
                      paths.overviewTask(
                        selectedTask.project_id,
                        selectedTask.id
                      )
                    )
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
        <TaskStatusBadge
          status={selectedTask.status}
          className="shrink-0"
        />
      </div>
    </NewCardHeader>
  ) : null;

  const attemptContent = selectedTask ? (
    <NewCard className="h-full min-h-0 flex flex-col bg-diagonal-lines bg-muted border-0">
      {isTaskView ? (
        <TaskPanel
          task={selectedTask}
          projectId={selectedTask.project_id}
          buildAttemptPath={paths.overviewAttempt}
        />
      ) : showNoAttemptsPanel ? (
        <NoAttemptsPanel taskId={selectedTask.id} />
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
        {activeMode === 'preview' && <PreviewPanel />}
        {activeMode === 'diffs' && (
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
            <TasksLayout
              kanban={listContent}
              attempt={attemptContent}
              aux={auxContent}
              isPanelOpen={isPanelOpen}
              mode={activeMode}
              isMobile={isMobile}
              rightHeader={rightHeader}
              kanbanLabel="Task list"
            />
          </ExecutionProcessesProvider>
        </ReviewProvider>
      </ClickedElementsProvider>
    </GitOperationsProvider>
  );

  return (
    <div className="min-h-full h-full flex flex-col">
      {projectsError && (
        <Alert className="w-full z-30 xl:sticky xl:top-0" variant="destructive">
          <AlertTitle className="flex items-center gap-2">
            <AlertTriangle size="16" />
            {t('common:states.error')}
          </AlertTitle>
          <AlertDescription>
            {projectsError.message || 'Failed to load projects'}
          </AlertDescription>
        </Alert>
      )}

      {streamError && (
        <Alert className="w-full z-30 xl:sticky xl:top-0">
          <AlertTitle className="flex items-center gap-2">
            <AlertTriangle size="16" />
            {t('common:states.reconnecting')}
          </AlertTitle>
          <AlertDescription>{streamError}</AlertDescription>
        </Alert>
      )}

      <div className="flex-1 min-h-0">{attemptArea}</div>
    </div>
  );
}
