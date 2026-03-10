import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Button } from '@/components/ui/button';
import { useNavigateWithSearch, useProjectRepos } from '@/hooks';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { attemptsApi, projectsApi } from '@/lib/api';
import { WorkspaceHookMenuSummary } from '@/components/tasks/WorkspaceHookMenuSummary';
import { useProjects } from '@/hooks/projects/useProjects';
import { useProjectTasks } from '@/hooks/projects/useProjectTasks';
import { ConfirmDialog } from '@/components/dialogs';
import {
  AlertCircle,
  ArrowLeft,
  Calendar,
  CheckSquare,
  Clock,
  Edit,
  Loader2,
  Trash2,
} from 'lucide-react';
import { getWorkspaceHookOutcome } from '@/utils/workspaceHooks';
import type {
  TaskWithAttemptStatus,
  WorkspaceLifecycleHookConfig,
} from 'shared/types';
import type { WorkspaceWithSession } from '@/types/attempt';

interface ProjectDetailProps {
  projectId: string;
  onBack: () => void;
}

export function ProjectDetail({ projectId, onBack }: ProjectDetailProps) {
  const { t } = useTranslation('projects');
  const { t: tSettings } = useTranslation('settings');
  const navigate = useNavigateWithSearch();
  const { projectsById, isLoading, error: projectsError } = useProjects();
  const [deleteError, setDeleteError] = useState('');

  const project = projectsById[projectId] || null;
  const hasConfiguredLifecycleHooks = Boolean(
    project?.after_prepare_hook || project?.before_cleanup_hook
  );
  const { tasks: projectTasks, isLoading: projectTasksLoading } =
    useProjectTasks(projectId);
  const { data: repos } = useProjectRepos(projectId);
  const lifecycleHookCandidateTasks = useMemo(
    () =>
      [...projectTasks]
        .sort(
          (left, right) =>
            new Date(right.updated_at).getTime() -
            new Date(left.updated_at).getTime()
        )
        .slice(0, 8),
    [projectTasks]
  );
  const latestLifecycleHookResult = useQuery<{
    attempt: WorkspaceWithSession;
    task: TaskWithAttemptStatus;
  } | null>({
    queryKey: [
      'projectLatestLifecycleHook',
      projectId,
      lifecycleHookCandidateTasks.map((task) => task.id),
    ],
    enabled:
      hasConfiguredLifecycleHooks && lifecycleHookCandidateTasks.length > 0,
    staleTime: 5_000,
    queryFn: async () => {
      const results = await Promise.allSettled(
        lifecycleHookCandidateTasks.map(async (task) => ({
          task,
          attempts: await attemptsApi.getAllWithSessions(task.id),
        }))
      );

      const candidates = results.flatMap((result) => {
        if (result.status !== 'fulfilled') {
          return [];
        }

        return result.value.attempts
          .filter((attempt) => getWorkspaceHookOutcome(attempt))
          .map((attempt) => ({
            attempt,
            task: result.value.task,
          }));
      });

      candidates.sort((left, right) => {
        const leftRun = getWorkspaceHookOutcome(left.attempt);
        const rightRun = getWorkspaceHookOutcome(right.attempt);
        return (
          new Date(rightRun?.ran_at ?? 0).getTime() -
          new Date(leftRun?.ran_at ?? 0).getTime()
        );
      });

      return candidates[0] ?? null;
    },
  });

  const renderLifecycleHookConfig = (
    title: string,
    description: string,
    hook: WorkspaceLifecycleHookConfig | null,
    phase: 'after_prepare' | 'before_cleanup'
  ) => {
    const isEnabled = Boolean(hook);
    const failurePolicyLabel = hook
      ? hook.failure_policy === 'block_start'
        ? tSettings(
            'settings.projects.lifecycleHooks.afterPrepare.failurePolicy.options.blockStart'
          )
        : hook.failure_policy === 'block_cleanup'
          ? tSettings(
              'settings.projects.lifecycleHooks.beforeCleanup.failurePolicy.options.blockCleanup'
            )
          : tSettings(
              'settings.projects.lifecycleHooks.shared.failurePolicy.options.warnOnly'
            )
      : null;
    const runModeLabel =
      phase === 'after_prepare' && hook
        ? hook.run_mode === 'every_prepare'
          ? tSettings(
              'settings.projects.lifecycleHooks.afterPrepare.runMode.options.everyPrepare'
            )
          : tSettings(
              'settings.projects.lifecycleHooks.afterPrepare.runMode.options.oncePerWorkspace'
            )
        : null;

    return (
      <div className="rounded-lg border border-border/60 bg-muted/10 px-4 py-3">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0 space-y-1">
            <div className="flex flex-wrap items-center gap-2">
              <h4 className="text-sm font-medium">{title}</h4>
              <Badge variant={isEnabled ? 'secondary' : 'outline'}>
                {isEnabled
                  ? tSettings(
                      'settings.projects.lifecycleHooks.summary.enabled'
                    )
                  : tSettings(
                      'settings.projects.lifecycleHooks.summary.disabled'
                    )}
              </Badge>
            </div>
            <p className="text-xs text-muted-foreground">{description}</p>
          </div>
        </div>

        {hook ? (
          <div className="mt-3 space-y-3">
            <code className="block rounded-md bg-background px-3 py-2 text-[11px] font-mono leading-relaxed break-all">
              {hook.command}
            </code>
            <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
              <span className="inline-flex items-center rounded-full border border-border/60 bg-background px-2.5 py-1">
                {tSettings(
                  'settings.projects.lifecycleHooks.shared.workingDir.label'
                )}
                {': '}
                <span className="ml-1 font-mono text-foreground">
                  {hook.working_dir?.trim() ||
                    tSettings(
                      'settings.projects.lifecycleHooks.summary.workspaceRoot'
                    )}
                </span>
              </span>
              <span className="inline-flex items-center rounded-full border border-border/60 bg-background px-2.5 py-1">
                {tSettings(
                  'settings.projects.lifecycleHooks.shared.failurePolicy.label'
                )}
                {': '}
                <span className="ml-1 text-foreground">
                  {failurePolicyLabel}
                </span>
              </span>
              {runModeLabel ? (
                <span className="inline-flex items-center rounded-full border border-border/60 bg-background px-2.5 py-1">
                  {tSettings(
                    'settings.projects.lifecycleHooks.afterPrepare.runMode.label'
                  )}
                  {': '}
                  <span className="ml-1 text-foreground">{runModeLabel}</span>
                </span>
              ) : null}
            </div>
          </div>
        ) : null}
      </div>
    );
  };

  const handleDelete = async () => {
    if (!project) return;

    const repoPath = repos?.[0]?.path ?? '';
    const repoPathLine = repoPath
      ? `\nRepo: ${repoPath}${repos && repos.length > 1 ? ` (+${repos.length - 1} more)` : ''}`
      : '';

    const result = await ConfirmDialog.show({
      title: t('delete.confirmTitle'),
      message: t('delete.confirmMessage', {
        name: project.name,
        id: project.id,
        repoPathLine,
      }),
      confirmText: t('common:buttons.delete'),
      cancelText: t('common:buttons.cancel'),
      variant: 'destructive',
    });

    if (result !== 'confirmed') return;

    try {
      await projectsApi.delete(projectId);
      onBack();
    } catch (error) {
      console.error('Failed to delete project:', error);
      // @ts-expect-error it is type ApiError
      setDeleteError(error.message || t('errors.deleteFailed'));
      setTimeout(() => setDeleteError(''), 5000);
    }
  };

  const handleEditClick = () => {
    navigate(`/settings/projects?projectId=${projectId}`);
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
        Loading project...
      </div>
    );
  }

  if ((!project && !isLoading) || projectsError) {
    const errorMsg = projectsError
      ? projectsError.message
      : t('projectNotFound');
    return (
      <div className="space-y-4 py-12 px-4">
        <Button variant="outline" onClick={onBack}>
          <ArrowLeft className="mr-2 h-4 w-4" />
          Back to Projects
        </Button>
        <Card>
          <CardContent className="py-12 text-center">
            <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-lg bg-muted">
              <AlertCircle className="h-6 w-6 text-muted-foreground" />
            </div>
            <h3 className="mt-4 text-lg font-semibold">Project not found</h3>
            <p className="mt-2 text-sm text-muted-foreground">{errorMsg}</p>
            <Button className="mt-4" onClick={onBack}>
              Back to Projects
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6 py-12 px-4">
      <div className="flex justify-between items-start">
        <div className="flex items-center space-x-4">
          <Button variant="outline" onClick={onBack}>
            <ArrowLeft className="mr-2 h-4 w-4" />
            Back to Projects
          </Button>
          <div>
            <div className="flex items-center gap-3">
              <h1 className="text-2xl font-bold">{project.name}</h1>
            </div>
            <p className="text-sm text-muted-foreground">
              Project details and settings
            </p>
          </div>
        </div>
        <div className="flex gap-2">
          <Button onClick={() => navigate(`/projects/${projectId}/tasks`)}>
            <CheckSquare className="mr-2 h-4 w-4" />
            View Tasks
          </Button>
          <Button variant="outline" onClick={handleEditClick}>
            <Edit className="mr-2 h-4 w-4" />
            Edit
          </Button>
          <Button
            variant="outline"
            onClick={handleDelete}
            className="text-destructive hover:text-destructive-foreground hover:bg-destructive/10"
          >
            <Trash2 className="mr-2 h-4 w-4" />
            Delete
          </Button>
        </div>
      </div>

      {deleteError && (
        <Alert variant="destructive">
          <AlertCircle className="h-4 w-4" />
          <AlertDescription>{deleteError}</AlertDescription>
        </Alert>
      )}

      <div className="grid gap-6 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center">
              <Calendar className="mr-2 h-5 w-5" />
              Project Information
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium text-muted-foreground">
                Status
              </span>
              <Badge variant="secondary">Active</Badge>
            </div>
            <div className="space-y-2">
              <div className="flex items-center justify-between text-sm">
                <span className="text-muted-foreground">
                  Scheduler concurrency:
                </span>
                <span>{project.scheduler_max_concurrent}</span>
              </div>
              <div className="flex items-center justify-between text-sm">
                <span className="text-muted-foreground">
                  Scheduler retries:
                </span>
                <span>{project.scheduler_max_retries}</span>
              </div>
              <div className="flex items-center text-sm">
                <Calendar className="mr-2 h-4 w-4 text-muted-foreground" />
                <span className="text-muted-foreground">Created:</span>
                <span className="ml-2">
                  {new Date(project.created_at).toLocaleDateString()}
                </span>
              </div>
              <div className="flex items-center text-sm">
                <Clock className="mr-2 h-4 w-4 text-muted-foreground" />
                <span className="text-muted-foreground">Last Updated:</span>
                <span className="ml-2">
                  {new Date(project.updated_at).toLocaleDateString()}
                </span>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Project Details</CardTitle>
            <CardDescription>
              Technical information about this project
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div>
              <h4 className="text-sm font-medium text-muted-foreground">
                Project ID
              </h4>
              <code className="mt-1 block text-xs bg-muted p-2 rounded font-mono">
                {project.id}
              </code>
            </div>
            <div>
              <h4 className="text-sm font-medium text-muted-foreground">
                Created At
              </h4>
              <p className="mt-1 text-sm">
                {new Date(project.created_at).toLocaleString()}
              </p>
            </div>
            <div>
              <h4 className="text-sm font-medium text-muted-foreground">
                Last Modified
              </h4>
              <p className="mt-1 text-sm">
                {new Date(project.updated_at).toLocaleString()}
              </p>
            </div>

            <div className="border-t pt-4 space-y-4">
              <div>
                <h4 className="text-sm font-medium text-muted-foreground">
                  {tSettings('settings.projects.lifecycleHooks.summary.title')}
                </h4>
                <p className="mt-1 text-sm text-muted-foreground">
                  {tSettings(
                    'settings.projects.lifecycleHooks.summary.description'
                  )}
                </p>
              </div>

              <div className="grid gap-3 lg:grid-cols-2">
                {renderLifecycleHookConfig(
                  tSettings(
                    'settings.projects.lifecycleHooks.afterPrepare.title'
                  ),
                  tSettings(
                    'settings.projects.lifecycleHooks.afterPrepare.description'
                  ),
                  project.after_prepare_hook,
                  'after_prepare'
                )}
                {renderLifecycleHookConfig(
                  tSettings(
                    'settings.projects.lifecycleHooks.beforeCleanup.title'
                  ),
                  tSettings(
                    'settings.projects.lifecycleHooks.beforeCleanup.description'
                  ),
                  project.before_cleanup_hook,
                  'before_cleanup'
                )}
              </div>

              {hasConfiguredLifecycleHooks ? (
                <div className="rounded-lg border border-border/60 bg-muted/10 p-4">
                  {latestLifecycleHookResult.isLoading ||
                  projectTasksLoading ? (
                    <p className="text-sm text-muted-foreground">
                      {tSettings(
                        'settings.projects.lifecycleHooks.summary.loadingLatest'
                      )}
                    </p>
                  ) : latestLifecycleHookResult.data ? (
                    <div className="space-y-2">
                      <p className="text-xs text-muted-foreground">
                        {tSettings(
                          'settings.projects.lifecycleHooks.summary.latestTask',
                          {
                            task:
                              latestLifecycleHookResult.data.task.title ||
                              tSettings(
                                'settings.projects.lifecycleHooks.summary.untitledTask'
                              ),
                          }
                        )}
                      </p>
                      <WorkspaceHookMenuSummary
                        workspace={latestLifecycleHookResult.data.attempt}
                      />
                    </div>
                  ) : (
                    <p className="text-sm text-muted-foreground">
                      {tSettings(
                        'settings.projects.lifecycleHooks.summary.noRuns'
                      )}
                    </p>
                  )}
                </div>
              ) : (
                <div className="rounded-lg border border-dashed border-border/60 bg-muted/5 p-4 text-sm text-muted-foreground">
                  {tSettings(
                    'settings.projects.lifecycleHooks.summary.noneConfigured'
                  )}
                </div>
              )}
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
