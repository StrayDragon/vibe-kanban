import { useCallback, useEffect, useMemo, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { isEqual } from 'lodash';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Checkbox } from '@/components/ui/checkbox';
import { Switch } from '@/components/ui/switch';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Loader2, Plus, Trash2 } from 'lucide-react';
import { useProjects } from '@/hooks/projects/useProjects';
import { useProjectMutations } from '@/hooks/projects/useProjectMutations';
import { useScriptPlaceholders } from '@/hooks/config/useScriptPlaceholders';
import { CopyFilesField } from '@/components/projects/CopyFilesField';
import { AutoExpandingTextarea } from '@/components/ui/auto-expanding-textarea';
import { RepoPickerDialog } from '@/components/dialogs/shared/RepoPickerDialog';
import { attemptsApi, projectsApi, tasksApi } from '@/lib/api';
import { repoBranchKeys } from '@/hooks/task-attempts/useRepoBranches';
import { WorkspaceHookMenuSummary } from '@/components/tasks/WorkspaceHookMenuSummary';
import { getWorkspaceHookOutcome } from '@/utils/workspaceHooks';
import { projectKeys } from '@/query-keys/projectKeys';
import type {
  Project,
  ProjectRepo,
  Repo,
  UpdateProject,
  TaskWithAttemptStatus,
  WorkspaceLifecycleHookConfig,
  WorkspaceLifecycleHookFailurePolicy,
  WorkspaceLifecycleHookRunMode,
} from 'shared/types';
import type { WorkspaceWithSession } from '@/types/attempt';

interface AfterPrepareHookFormState {
  enabled: boolean;
  command: string;
  working_dir: string;
  failure_policy: Extract<
    WorkspaceLifecycleHookFailurePolicy,
    'block_start' | 'warn_only'
  >;
  run_mode: WorkspaceLifecycleHookRunMode;
}

interface BeforeCleanupHookFormState {
  enabled: boolean;
  command: string;
  working_dir: string;
  failure_policy: Extract<
    WorkspaceLifecycleHookFailurePolicy,
    'warn_only' | 'block_cleanup'
  >;
}

interface ProjectFormState {
  name: string;
  dev_script: string;
  dev_script_working_dir: string;
  default_agent_working_dir: string;
  git_no_verify_override: GitNoVerifyOverrideMode;
  scheduler_max_concurrent: string;
  scheduler_max_retries: string;
  after_prepare_hook: AfterPrepareHookFormState;
  before_cleanup_hook: BeforeCleanupHookFormState;
}

type GitNoVerifyOverrideMode = 'INHERIT' | 'ENABLED' | 'DISABLED';

interface RepoScriptsFormState {
  setup_script: string;
  parallel_setup_script: boolean;
  cleanup_script: string;
  copy_files: string;
}

function gitNoVerifyOverrideToMode(
  value: boolean | null | undefined
): GitNoVerifyOverrideMode {
  if (value === true) return 'ENABLED';
  if (value === false) return 'DISABLED';
  return 'INHERIT';
}

function gitNoVerifyModeToOverride(
  mode: GitNoVerifyOverrideMode
): boolean | null {
  switch (mode) {
    case 'ENABLED':
      return true;
    case 'DISABLED':
      return false;
    case 'INHERIT':
      return null;
  }
}

function hookConfigToAfterPrepareFormState(
  hook: WorkspaceLifecycleHookConfig | null
): AfterPrepareHookFormState {
  return {
    enabled: !!hook,
    command: hook?.command ?? '',
    working_dir: hook?.working_dir ?? '',
    failure_policy:
      hook?.failure_policy === 'warn_only' ? 'warn_only' : 'block_start',
    run_mode: hook?.run_mode ?? 'once_per_workspace',
  };
}

function hookConfigToBeforeCleanupFormState(
  hook: WorkspaceLifecycleHookConfig | null
): BeforeCleanupHookFormState {
  return {
    enabled: !!hook,
    command: hook?.command ?? '',
    working_dir: hook?.working_dir ?? '',
    failure_policy:
      hook?.failure_policy === 'block_cleanup' ? 'block_cleanup' : 'warn_only',
  };
}

function afterPrepareFormStateToHookConfig(
  hook: AfterPrepareHookFormState
): WorkspaceLifecycleHookConfig | null {
  if (!hook.enabled) {
    return null;
  }

  return {
    command: hook.command.trim(),
    working_dir: hook.working_dir.trim() || null,
    failure_policy: hook.failure_policy,
    run_mode: hook.run_mode,
  };
}

function beforeCleanupFormStateToHookConfig(
  hook: BeforeCleanupHookFormState
): WorkspaceLifecycleHookConfig | null {
  if (!hook.enabled) {
    return null;
  }

  return {
    command: hook.command.trim(),
    working_dir: hook.working_dir.trim() || null,
    failure_policy: hook.failure_policy,
    run_mode: null,
  };
}

function projectToFormState(project: Project): ProjectFormState {
  return {
    name: project.name,
    dev_script: project.dev_script ?? '',
    dev_script_working_dir: project.dev_script_working_dir ?? '',
    default_agent_working_dir: project.default_agent_working_dir ?? '',
    git_no_verify_override: gitNoVerifyOverrideToMode(
      project.git_no_verify_override
    ),
    scheduler_max_concurrent: String(project.scheduler_max_concurrent),
    scheduler_max_retries: String(project.scheduler_max_retries),
    after_prepare_hook: hookConfigToAfterPrepareFormState(
      project.after_prepare_hook
    ),
    before_cleanup_hook: hookConfigToBeforeCleanupFormState(
      project.before_cleanup_hook
    ),
  };
}

function projectRepoToScriptsFormState(
  projectRepo: ProjectRepo | null
): RepoScriptsFormState {
  return {
    setup_script: projectRepo?.setup_script ?? '',
    parallel_setup_script: projectRepo?.parallel_setup_script ?? false,
    cleanup_script: projectRepo?.cleanup_script ?? '',
    copy_files: projectRepo?.copy_files ?? '',
  };
}

export function ProjectSettings() {
  const [searchParams, setSearchParams] = useSearchParams();
  const projectIdParam = searchParams.get('projectId') ?? '';
  const { t } = useTranslation('settings');
  const queryClient = useQueryClient();

  // Fetch all projects
  const {
    projects,
    isLoading: projectsLoading,
    error: projectsError,
  } = useProjects();

  const projectNameCounts = useMemo(() => {
    const counts = new Map<string, number>();
    projects.forEach((project) => {
      counts.set(project.name, (counts.get(project.name) ?? 0) + 1);
    });
    return counts;
  }, [projects]);

  // Selected project state
  const [selectedProjectId, setSelectedProjectId] = useState<string>(
    searchParams.get('projectId') || ''
  );
  const [selectedProject, setSelectedProject] = useState<Project | null>(null);
  const [showLatestHookOutcome, setShowLatestHookOutcome] = useState(false);

  // Form state
  const [draft, setDraft] = useState<ProjectFormState | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  // Repositories state
  const [repositories, setRepositories] = useState<Repo[]>([]);
  const [loadingRepos, setLoadingRepos] = useState(false);
  const [repoError, setRepoError] = useState<string | null>(null);
  const [addingRepo, setAddingRepo] = useState(false);
  const [deletingRepoId, setDeletingRepoId] = useState<string | null>(null);

  // Scripts repo state (per-repo scripts)
  const [selectedScriptsRepoId, setSelectedScriptsRepoId] = useState<
    string | null
  >(null);
  const [selectedProjectRepo, setSelectedProjectRepo] =
    useState<ProjectRepo | null>(null);
  const [scriptsDraft, setScriptsDraft] = useState<RepoScriptsFormState | null>(
    null
  );
  const [loadingProjectRepo, setLoadingProjectRepo] = useState(false);
  const [savingScripts, setSavingScripts] = useState(false);
  const [scriptsSuccess, setScriptsSuccess] = useState(false);
  const [scriptsError, setScriptsError] = useState<string | null>(null);

  // Get OS-appropriate script placeholders
  const placeholders = useScriptPlaceholders();

  const hasConfiguredLifecycleHooks = Boolean(
    draft?.after_prepare_hook.enabled || draft?.before_cleanup_hook.enabled
  );

  useEffect(() => {
    setShowLatestHookOutcome(false);
  }, [selectedProjectId]);

  useEffect(() => {
    if (!hasConfiguredLifecycleHooks) {
      setShowLatestHookOutcome(false);
    }
  }, [hasConfiguredLifecycleHooks]);

  const latestLifecycleHookResult = useQuery<{
    attempt: WorkspaceWithSession;
    task: TaskWithAttemptStatus;
  } | null>({
    queryKey: projectKeys.latestLifecycleHookOutcome(selectedProjectId),
    enabled:
      showLatestHookOutcome &&
      hasConfiguredLifecycleHooks &&
      !!selectedProjectId,
    staleTime: 5_000,
    queryFn: async () => {
      const tasks = await tasksApi.getAll({ projectId: selectedProjectId });
      const lifecycleHookCandidateTasks = [...tasks]
        .sort(
          (left, right) =>
            new Date(right.updated_at).getTime() -
            new Date(left.updated_at).getTime()
        )
        .slice(0, 8);

      if (lifecycleHookCandidateTasks.length === 0) {
        return null;
      }

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

  // Check for unsaved changes (project name)
  const hasUnsavedProjectChanges = useMemo(() => {
    if (!draft || !selectedProject) return false;
    return !isEqual(draft, projectToFormState(selectedProject));
  }, [draft, selectedProject]);

  // Check for unsaved script changes
  const hasUnsavedScriptsChanges = useMemo(() => {
    if (!scriptsDraft || !selectedProjectRepo) return false;
    return !isEqual(
      scriptsDraft,
      projectRepoToScriptsFormState(selectedProjectRepo)
    );
  }, [scriptsDraft, selectedProjectRepo]);

  // Combined check for any unsaved changes
  const hasUnsavedChanges =
    hasUnsavedProjectChanges || hasUnsavedScriptsChanges;

  // Handle project selection from dropdown
  const handleProjectSelect = useCallback(
    (id: string) => {
      // No-op if same project
      if (id === selectedProjectId) return;

      // Confirm if there are unsaved changes
      if (hasUnsavedChanges) {
        const confirmed = window.confirm(
          t('settings.projects.save.confirmSwitch')
        );
        if (!confirmed) return;

        // Clear local state before switching
        setDraft(null);
        setSelectedProject(null);
        setSuccess(false);
        setError(null);
      }

      // Update state and URL
      setSelectedProjectId(id);
      if (id) {
        setSearchParams({ projectId: id });
      } else {
        setSearchParams({});
      }
    },
    [hasUnsavedChanges, selectedProjectId, setSearchParams, t]
  );

  // Sync selectedProjectId when URL changes (with unsaved changes prompt)
  useEffect(() => {
    if (projectIdParam === selectedProjectId) return;

    // Confirm if there are unsaved changes
    if (hasUnsavedChanges) {
      const confirmed = window.confirm(
        t('settings.projects.save.confirmSwitch')
      );
      if (!confirmed) {
        // Revert URL to previous value
        if (selectedProjectId) {
          setSearchParams({ projectId: selectedProjectId });
        } else {
          setSearchParams({});
        }
        return;
      }

      // Clear local state before switching
      setDraft(null);
      setSelectedProject(null);
      setSuccess(false);
      setError(null);
    }

    setSelectedProjectId(projectIdParam);
  }, [
    projectIdParam,
    hasUnsavedChanges,
    selectedProjectId,
    setSearchParams,
    t,
  ]);

  // Populate draft from server data
  useEffect(() => {
    if (!projects) return;

    const nextProject = selectedProjectId
      ? projects.find((p) => p.id === selectedProjectId)
      : null;

    if (!nextProject) {
      if (!hasUnsavedChanges) {
        setSelectedProject(null);
        setDraft(null);
      }
      return;
    }

    const nextUpdatedAtMs = (() => {
      const ms = new Date(
        nextProject.updated_at as unknown as string
      ).getTime();
      return Number.isFinite(ms) ? ms : 0;
    })();

    const selectedUpdatedAtMs =
      selectedProject?.id === nextProject.id
        ? (() => {
            const ms = new Date(
              selectedProject.updated_at as unknown as string
            ).getTime();
            return Number.isFinite(ms) ? ms : 0;
          })()
        : 0;

    const effectiveProject =
      selectedProject &&
      selectedProject.id === nextProject.id &&
      selectedUpdatedAtMs >= nextUpdatedAtMs
        ? selectedProject
        : nextProject;

    if (hasUnsavedChanges) {
      if (!selectedProject || selectedProject.id !== effectiveProject.id) {
        setSelectedProject(effectiveProject);
      }
      return;
    }

    setSelectedProject(effectiveProject);
    setDraft(projectToFormState(effectiveProject));
  }, [projects, selectedProjectId, hasUnsavedChanges, selectedProject]);

  // Warn on tab close/navigation with unsaved changes
  useEffect(() => {
    const handler = (e: BeforeUnloadEvent) => {
      if (hasUnsavedChanges) {
        e.preventDefault();
        e.returnValue = '';
      }
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, [hasUnsavedChanges]);

  // Fetch repositories when project changes
  useEffect(() => {
    if (!selectedProjectId) {
      setRepositories([]);
      return;
    }

    setLoadingRepos(true);
    setRepoError(null);
    projectsApi
      .getRepositories(selectedProjectId)
      .then(setRepositories)
      .catch((err) => {
        setRepoError(
          err instanceof Error ? err.message : 'Failed to load repositories'
        );
        setRepositories([]);
      })
      .finally(() => setLoadingRepos(false));
  }, [selectedProjectId]);

  // Auto-select first repository for scripts when repositories load
  useEffect(() => {
    if (repositories.length > 0 && !selectedScriptsRepoId) {
      setSelectedScriptsRepoId(repositories[0].id);
    }
    // Clear selection if repo was deleted
    if (
      selectedScriptsRepoId &&
      !repositories.some((r) => r.id === selectedScriptsRepoId)
    ) {
      setSelectedScriptsRepoId(repositories[0]?.id ?? null);
    }
  }, [repositories, selectedScriptsRepoId]);

  // Reset scripts selection when project changes
  useEffect(() => {
    setSelectedScriptsRepoId(null);
    setSelectedProjectRepo(null);
    setScriptsDraft(null);
    setScriptsError(null);
  }, [selectedProjectId]);

  // Fetch ProjectRepo scripts when selected scripts repo changes
  useEffect(() => {
    if (!selectedProjectId || !selectedScriptsRepoId) {
      setSelectedProjectRepo(null);
      setScriptsDraft(null);
      return;
    }

    setLoadingProjectRepo(true);
    setScriptsError(null);
    projectsApi
      .getRepository(selectedProjectId, selectedScriptsRepoId)
      .then((projectRepo) => {
        setSelectedProjectRepo(projectRepo);
        setScriptsDraft(projectRepoToScriptsFormState(projectRepo));
      })
      .catch((err) => {
        setScriptsError(
          err instanceof Error
            ? err.message
            : 'Failed to load repository scripts'
        );
        setSelectedProjectRepo(null);
        setScriptsDraft(null);
      })
      .finally(() => setLoadingProjectRepo(false));
  }, [selectedProjectId, selectedScriptsRepoId]);

  const handleAddRepository = async () => {
    if (!selectedProjectId) return;

    const repo = await RepoPickerDialog.show({
      title: 'Select Git Repository',
      description: 'Choose a git repository to add to this project',
    });

    if (!repo) return;

    if (repositories.some((r) => r.id === repo.id)) {
      return;
    }

    setAddingRepo(true);
    setRepoError(null);
    try {
      const newRepo = await projectsApi.addRepository(selectedProjectId, {
        display_name: repo.display_name,
        git_repo_path: repo.path,
      });
      setRepositories((prev) => [...prev, newRepo]);
      queryClient.invalidateQueries({
        queryKey: projectKeys.repositories(selectedProjectId),
      });
      queryClient.invalidateQueries({
        queryKey: repoBranchKeys.byRepo(newRepo.id),
      });
    } catch (err) {
      setRepoError(
        err instanceof Error ? err.message : 'Failed to add repository'
      );
    } finally {
      setAddingRepo(false);
    }
  };

  const handleDeleteRepository = async (repoId: string) => {
    if (!selectedProjectId) return;

    setDeletingRepoId(repoId);
    setRepoError(null);
    try {
      await projectsApi.deleteRepository(selectedProjectId, repoId);
      setRepositories((prev) => prev.filter((r) => r.id !== repoId));
      queryClient.invalidateQueries({
        queryKey: projectKeys.repositories(selectedProjectId),
      });
      queryClient.invalidateQueries({
        queryKey: repoBranchKeys.byRepo(repoId),
      });
    } catch (err) {
      setRepoError(
        err instanceof Error ? err.message : 'Failed to delete repository'
      );
    } finally {
      setDeletingRepoId(null);
    }
  };

  const { updateProject } = useProjectMutations({
    onUpdateSuccess: (updatedProject: Project) => {
      // Update local state with fresh data from server
      setSelectedProject(updatedProject);
      setDraft(projectToFormState(updatedProject));
      setSuccess(true);
      setTimeout(() => setSuccess(false), 3000);
      setSaving(false);
    },
    onUpdateError: (err) => {
      setError(
        err instanceof Error ? err.message : 'Failed to save project settings'
      );
      setSaving(false);
    },
  });

  const handleSave = async () => {
    if (!draft || !selectedProject) return;

    setSaving(true);
    setError(null);
    setSuccess(false);

    if (
      draft.after_prepare_hook.enabled &&
      !draft.after_prepare_hook.command.trim()
    ) {
      setError(
        t(
          'settings.projects.lifecycleHooks.validation.afterPrepareCommandRequired'
        )
      );
      setSaving(false);
      return;
    }

    if (
      draft.before_cleanup_hook.enabled &&
      !draft.before_cleanup_hook.command.trim()
    ) {
      setError(
        t(
          'settings.projects.lifecycleHooks.validation.beforeCleanupCommandRequired'
        )
      );
      setSaving(false);
      return;
    }

    try {
      const updateData: UpdateProject = {
        name: draft.name.trim(),
        dev_script: draft.dev_script.trim() || null,
        dev_script_working_dir: draft.dev_script_working_dir.trim() || null,
        default_agent_working_dir:
          draft.default_agent_working_dir.trim() || null,
        git_no_verify_override: gitNoVerifyModeToOverride(
          draft.git_no_verify_override
        ),
        scheduler_max_concurrent: Math.max(
          1,
          Number.parseInt(draft.scheduler_max_concurrent, 10) || 1
        ),
        scheduler_max_retries: Math.max(
          0,
          Number.parseInt(draft.scheduler_max_retries, 10) || 0
        ),
        default_continuation_turns: selectedProject.default_continuation_turns,
        after_prepare_hook: afterPrepareFormStateToHookConfig(
          draft.after_prepare_hook
        ),
        before_cleanup_hook: beforeCleanupFormStateToHookConfig(
          draft.before_cleanup_hook
        ),
      };

      updateProject.mutate({
        projectId: selectedProject.id,
        data: updateData,
      });
    } catch (err) {
      setError(t('settings.projects.save.error'));
      console.error('Error saving project settings:', err);
      setSaving(false);
    }
  };

  const handleSaveScripts = async () => {
    if (!scriptsDraft || !selectedProjectId || !selectedScriptsRepoId) return;

    setSavingScripts(true);
    setScriptsError(null);
    setScriptsSuccess(false);

    try {
      const updatedRepo = await projectsApi.updateRepository(
        selectedProjectId,
        selectedScriptsRepoId,
        {
          setup_script: scriptsDraft.setup_script.trim() || null,
          cleanup_script: scriptsDraft.cleanup_script.trim() || null,
          copy_files: scriptsDraft.copy_files.trim() || null,
          parallel_setup_script: scriptsDraft.parallel_setup_script,
        }
      );
      setSelectedProjectRepo(updatedRepo);
      setScriptsDraft(projectRepoToScriptsFormState(updatedRepo));
      setScriptsSuccess(true);
      setTimeout(() => setScriptsSuccess(false), 3000);
    } catch (err) {
      setScriptsError(
        err instanceof Error ? err.message : 'Failed to save scripts'
      );
    } finally {
      setSavingScripts(false);
    }
  };

  const handleDiscard = () => {
    if (!selectedProject) return;
    setDraft(projectToFormState(selectedProject));
  };

  const handleDiscardScripts = () => {
    if (!selectedProjectRepo) return;
    setScriptsDraft(projectRepoToScriptsFormState(selectedProjectRepo));
  };

  const updateDraft = (updates: Partial<ProjectFormState>) => {
    setDraft((prev) => {
      if (!prev) return prev;
      return { ...prev, ...updates };
    });
  };

  const updateAfterPrepareHookDraft = (
    updates: Partial<AfterPrepareHookFormState>
  ) => {
    setDraft((prev) => {
      if (!prev) return prev;
      return {
        ...prev,
        after_prepare_hook: { ...prev.after_prepare_hook, ...updates },
      };
    });
  };

  const updateBeforeCleanupHookDraft = (
    updates: Partial<BeforeCleanupHookFormState>
  ) => {
    setDraft((prev) => {
      if (!prev) return prev;
      return {
        ...prev,
        before_cleanup_hook: { ...prev.before_cleanup_hook, ...updates },
      };
    });
  };

  const updateScriptsDraft = (updates: Partial<RepoScriptsFormState>) => {
    setScriptsDraft((prev) => {
      if (!prev) return prev;
      return { ...prev, ...updates };
    });
  };

  if (projectsLoading) {
    return (
      <div className="flex items-center justify-center py-8">
        <Loader2 className="h-8 w-8 animate-spin" />
        <span className="ml-2">{t('settings.projects.loading')}</span>
      </div>
    );
  }

  if (projectsError) {
    return (
      <div className="py-8">
        <Alert variant="destructive">
          <AlertDescription>
            {projectsError instanceof Error
              ? projectsError.message
              : t('settings.projects.loadError')}
          </AlertDescription>
        </Alert>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      {success && (
        <Alert variant="success">
          <AlertDescription className="font-medium">
            {t('settings.projects.save.success')}
          </AlertDescription>
        </Alert>
      )}

      <Card>
        <CardHeader>
          <CardTitle>{t('settings.projects.title')}</CardTitle>
          <CardDescription>
            {t('settings.projects.description')}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="project-selector">
              {t('settings.projects.selector.label')}
            </Label>
            <Select
              value={selectedProjectId}
              onValueChange={handleProjectSelect}
            >
              <SelectTrigger id="project-selector">
                <SelectValue
                  placeholder={t('settings.projects.selector.placeholder')}
                />
              </SelectTrigger>
              <SelectContent>
                {projects && projects.length > 0 ? (
                  projects.map((project) => (
                    <SelectItem key={project.id} value={project.id}>
                      <div className="flex flex-col">
                        <span className="truncate">{project.name}</span>
                        {(projectNameCounts.get(project.name) ?? 0) > 1 && (
                          <span className="truncate text-xs font-mono text-muted-foreground">
                            {project.id}
                          </span>
                        )}
                      </div>
                    </SelectItem>
                  ))
                ) : (
                  <SelectItem value="no-projects" disabled>
                    {t('settings.projects.selector.noProjects')}
                  </SelectItem>
                )}
              </SelectContent>
            </Select>
            <p className="text-sm text-muted-foreground">
              {t('settings.projects.selector.helper')}
            </p>
          </div>
        </CardContent>
      </Card>

      {selectedProject && draft && (
        <>
          <Card>
            <CardHeader>
              <CardTitle>{t('settings.projects.general.title')}</CardTitle>
              <CardDescription>
                {t('settings.projects.general.description')}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="rounded-lg border border-border/60 bg-muted/10 p-4">
                <div className="text-xs font-medium text-muted-foreground">
                  {t('settings.projects.general.metadata.title')}
                </div>
                <div className="mt-3 grid gap-4 md:grid-cols-3">
                  <div className="space-y-1">
                    <div className="text-xs font-medium text-muted-foreground">
                      {t('settings.projects.general.metadata.projectId')}
                    </div>
                    <div className="break-all font-mono text-sm">
                      {selectedProject.id}
                    </div>
                  </div>
                  <div className="space-y-1">
                    <div className="text-xs font-medium text-muted-foreground">
                      {t('settings.projects.general.metadata.createdAt')}
                    </div>
                    <div className="text-sm">
                      {new Date(selectedProject.created_at).toLocaleString()}
                    </div>
                  </div>
                  <div className="space-y-1">
                    <div className="text-xs font-medium text-muted-foreground">
                      {t('settings.projects.general.metadata.updatedAt')}
                    </div>
                    <div className="text-sm">
                      {new Date(selectedProject.updated_at).toLocaleString()}
                    </div>
                  </div>
                </div>
              </div>

              <div className="space-y-2">
                <Label htmlFor="project-name">
                  {t('settings.projects.general.name.label')}
                </Label>
                <Input
                  id="project-name"
                  type="text"
                  value={draft.name}
                  onChange={(e) => updateDraft({ name: e.target.value })}
                  placeholder={t('settings.projects.general.name.placeholder')}
                  required
                />
                <p className="text-sm text-muted-foreground">
                  {t('settings.projects.general.name.helper')}
                </p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="dev-script">
                  {t('settings.projects.scripts.dev.label')}
                </Label>
                <AutoExpandingTextarea
                  id="dev-script"
                  value={draft.dev_script}
                  onChange={(e) => updateDraft({ dev_script: e.target.value })}
                  placeholder={placeholders.dev}
                  maxRows={12}
                  className="w-full px-3 py-2 border border-input bg-background text-foreground rounded-md focus:outline-none focus:ring-2 focus:ring-ring font-mono"
                />
                <p className="text-sm text-muted-foreground">
                  {t('settings.projects.scripts.dev.helper')}
                </p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="dev-script-working-dir">
                  {t('settings.projects.scripts.devWorkingDir.label')}
                </Label>
                <Input
                  id="dev-script-working-dir"
                  value={draft.dev_script_working_dir}
                  onChange={(e) =>
                    updateDraft({ dev_script_working_dir: e.target.value })
                  }
                  placeholder={t(
                    'settings.projects.scripts.devWorkingDir.placeholder'
                  )}
                  className="font-mono"
                />
                <p className="text-sm text-muted-foreground">
                  {t('settings.projects.scripts.devWorkingDir.helper')}
                </p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="agent-working-dir">
                  {t('settings.projects.scripts.agentWorkingDir.label')}
                </Label>
                <Input
                  id="agent-working-dir"
                  value={draft.default_agent_working_dir}
                  onChange={(e) =>
                    updateDraft({ default_agent_working_dir: e.target.value })
                  }
                  placeholder={t(
                    'settings.projects.scripts.agentWorkingDir.placeholder'
                  )}
                  className="font-mono"
                />
                <p className="text-sm text-muted-foreground">
                  {t('settings.projects.scripts.agentWorkingDir.helper')}
                </p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="project-git-no-verify">
                  {t('settings.projects.git.noVerify.label')}
                </Label>
                <Select
                  value={draft.git_no_verify_override}
                  onValueChange={(value) =>
                    updateDraft({
                      git_no_verify_override: value as GitNoVerifyOverrideMode,
                    })
                  }
                >
                  <SelectTrigger id="project-git-no-verify">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="INHERIT">
                      {t('settings.projects.git.noVerify.options.inherit')}
                    </SelectItem>
                    <SelectItem value="ENABLED">
                      {t('settings.projects.git.noVerify.options.enabled')}
                    </SelectItem>
                    <SelectItem value="DISABLED">
                      {t('settings.projects.git.noVerify.options.disabled')}
                    </SelectItem>
                  </SelectContent>
                </Select>
                <p className="text-sm text-muted-foreground">
                  {t('settings.projects.git.noVerify.helper')}
                </p>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <Label htmlFor="project-scheduler-max-concurrent">
                    {t('settings.projects.scheduler.maxConcurrent.label')}
                  </Label>
                  <Input
                    id="project-scheduler-max-concurrent"
                    type="number"
                    min={1}
                    value={draft.scheduler_max_concurrent}
                    onChange={(e) =>
                      updateDraft({
                        scheduler_max_concurrent: e.target.value,
                      })
                    }
                  />
                  <p className="text-sm text-muted-foreground">
                    {t('settings.projects.scheduler.maxConcurrent.helper')}
                  </p>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="project-scheduler-max-retries">
                    {t('settings.projects.scheduler.maxRetries.label')}
                  </Label>
                  <Input
                    id="project-scheduler-max-retries"
                    type="number"
                    min={0}
                    value={draft.scheduler_max_retries}
                    onChange={(e) =>
                      updateDraft({
                        scheduler_max_retries: e.target.value,
                      })
                    }
                  />
                  <p className="text-sm text-muted-foreground">
                    {t('settings.projects.scheduler.maxRetries.helper')}
                  </p>
                </div>
              </div>

              <div className="space-y-4 rounded-lg border border-border/60 p-4">
                <div className="space-y-1">
                  <h3 className="text-base font-semibold">
                    {t('settings.projects.lifecycleHooks.title')}
                  </h3>
                  <p className="text-sm text-muted-foreground">
                    {t('settings.projects.lifecycleHooks.description')}
                  </p>
                </div>

                <div className="grid gap-4 xl:grid-cols-2">
                  <div className="space-y-4 rounded-lg border border-border/60 p-4">
                    <div className="flex items-start justify-between gap-4">
                      <div className="space-y-1">
                        <Label htmlFor="project-after-prepare-hook-enabled">
                          {t(
                            'settings.projects.lifecycleHooks.afterPrepare.title'
                          )}
                        </Label>
                        <p className="text-sm text-muted-foreground">
                          {t(
                            'settings.projects.lifecycleHooks.afterPrepare.description'
                          )}
                        </p>
                      </div>
                      <Switch
                        id="project-after-prepare-hook-enabled"
                        checked={draft.after_prepare_hook.enabled}
                        onCheckedChange={(checked) =>
                          updateAfterPrepareHookDraft({ enabled: checked })
                        }
                      />
                    </div>

                    {draft.after_prepare_hook.enabled ? (
                      <div className="space-y-4">
                        <div className="space-y-2">
                          <Label htmlFor="project-after-prepare-hook-command">
                            {t(
                              'settings.projects.lifecycleHooks.shared.command.label'
                            )}
                          </Label>
                          <AutoExpandingTextarea
                            id="project-after-prepare-hook-command"
                            value={draft.after_prepare_hook.command}
                            onChange={(e) =>
                              updateAfterPrepareHookDraft({
                                command: e.target.value,
                              })
                            }
                            placeholder={t(
                              'settings.projects.lifecycleHooks.afterPrepare.command.placeholder'
                            )}
                            maxRows={8}
                            className="w-full rounded-md border border-input bg-background px-3 py-2 font-mono text-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                          />
                          <p className="text-sm text-muted-foreground">
                            {t(
                              'settings.projects.lifecycleHooks.shared.command.helper'
                            )}
                          </p>
                        </div>

                        <div className="space-y-2">
                          <Label htmlFor="project-after-prepare-hook-working-dir">
                            {t(
                              'settings.projects.lifecycleHooks.shared.workingDir.label'
                            )}
                          </Label>
                          <Input
                            id="project-after-prepare-hook-working-dir"
                            value={draft.after_prepare_hook.working_dir}
                            onChange={(e) =>
                              updateAfterPrepareHookDraft({
                                working_dir: e.target.value,
                              })
                            }
                            placeholder={t(
                              'settings.projects.lifecycleHooks.shared.workingDir.placeholder'
                            )}
                            className="font-mono"
                          />
                          <p className="text-sm text-muted-foreground">
                            {t(
                              'settings.projects.lifecycleHooks.shared.workingDir.helper'
                            )}
                          </p>
                        </div>

                        <div className="grid gap-4 md:grid-cols-2">
                          <div className="space-y-2">
                            <Label htmlFor="project-after-prepare-hook-failure-policy">
                              {t(
                                'settings.projects.lifecycleHooks.shared.failurePolicy.label'
                              )}
                            </Label>
                            <Select
                              value={draft.after_prepare_hook.failure_policy}
                              onValueChange={(value) =>
                                updateAfterPrepareHookDraft({
                                  failure_policy: value as Extract<
                                    WorkspaceLifecycleHookFailurePolicy,
                                    'block_start' | 'warn_only'
                                  >,
                                })
                              }
                            >
                              <SelectTrigger id="project-after-prepare-hook-failure-policy">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent>
                                <SelectItem value="block_start">
                                  {t(
                                    'settings.projects.lifecycleHooks.afterPrepare.failurePolicy.options.blockStart'
                                  )}
                                </SelectItem>
                                <SelectItem value="warn_only">
                                  {t(
                                    'settings.projects.lifecycleHooks.shared.failurePolicy.options.warnOnly'
                                  )}
                                </SelectItem>
                              </SelectContent>
                            </Select>
                          </div>

                          <div className="space-y-2">
                            <Label htmlFor="project-after-prepare-hook-run-mode">
                              {t(
                                'settings.projects.lifecycleHooks.afterPrepare.runMode.label'
                              )}
                            </Label>
                            <Select
                              value={draft.after_prepare_hook.run_mode}
                              onValueChange={(value) =>
                                updateAfterPrepareHookDraft({
                                  run_mode:
                                    value as WorkspaceLifecycleHookRunMode,
                                })
                              }
                            >
                              <SelectTrigger id="project-after-prepare-hook-run-mode">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent>
                                <SelectItem value="once_per_workspace">
                                  {t(
                                    'settings.projects.lifecycleHooks.afterPrepare.runMode.options.oncePerWorkspace'
                                  )}
                                </SelectItem>
                                <SelectItem value="every_prepare">
                                  {t(
                                    'settings.projects.lifecycleHooks.afterPrepare.runMode.options.everyPrepare'
                                  )}
                                </SelectItem>
                              </SelectContent>
                            </Select>
                          </div>
                        </div>
                      </div>
                    ) : null}
                  </div>

                  <div className="space-y-4 rounded-lg border border-border/60 p-4">
                    <div className="flex items-start justify-between gap-4">
                      <div className="space-y-1">
                        <Label htmlFor="project-before-cleanup-hook-enabled">
                          {t(
                            'settings.projects.lifecycleHooks.beforeCleanup.title'
                          )}
                        </Label>
                        <p className="text-sm text-muted-foreground">
                          {t(
                            'settings.projects.lifecycleHooks.beforeCleanup.description'
                          )}
                        </p>
                      </div>
                      <Switch
                        id="project-before-cleanup-hook-enabled"
                        checked={draft.before_cleanup_hook.enabled}
                        onCheckedChange={(checked) =>
                          updateBeforeCleanupHookDraft({ enabled: checked })
                        }
                      />
                    </div>

                    {draft.before_cleanup_hook.enabled ? (
                      <div className="space-y-4">
                        <div className="space-y-2">
                          <Label htmlFor="project-before-cleanup-hook-command">
                            {t(
                              'settings.projects.lifecycleHooks.shared.command.label'
                            )}
                          </Label>
                          <AutoExpandingTextarea
                            id="project-before-cleanup-hook-command"
                            value={draft.before_cleanup_hook.command}
                            onChange={(e) =>
                              updateBeforeCleanupHookDraft({
                                command: e.target.value,
                              })
                            }
                            placeholder={t(
                              'settings.projects.lifecycleHooks.beforeCleanup.command.placeholder'
                            )}
                            maxRows={8}
                            className="w-full rounded-md border border-input bg-background px-3 py-2 font-mono text-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                          />
                          <p className="text-sm text-muted-foreground">
                            {t(
                              'settings.projects.lifecycleHooks.shared.command.helper'
                            )}
                          </p>
                        </div>

                        <div className="space-y-2">
                          <Label htmlFor="project-before-cleanup-hook-working-dir">
                            {t(
                              'settings.projects.lifecycleHooks.shared.workingDir.label'
                            )}
                          </Label>
                          <Input
                            id="project-before-cleanup-hook-working-dir"
                            value={draft.before_cleanup_hook.working_dir}
                            onChange={(e) =>
                              updateBeforeCleanupHookDraft({
                                working_dir: e.target.value,
                              })
                            }
                            placeholder={t(
                              'settings.projects.lifecycleHooks.shared.workingDir.placeholder'
                            )}
                            className="font-mono"
                          />
                          <p className="text-sm text-muted-foreground">
                            {t(
                              'settings.projects.lifecycleHooks.shared.workingDir.helper'
                            )}
                          </p>
                        </div>

                        <div className="space-y-2">
                          <Label htmlFor="project-before-cleanup-hook-failure-policy">
                            {t(
                              'settings.projects.lifecycleHooks.shared.failurePolicy.label'
                            )}
                          </Label>
                          <Select
                            value={draft.before_cleanup_hook.failure_policy}
                            onValueChange={(value) =>
                              updateBeforeCleanupHookDraft({
                                failure_policy: value as Extract<
                                  WorkspaceLifecycleHookFailurePolicy,
                                  'warn_only' | 'block_cleanup'
                                >,
                              })
                            }
                          >
                            <SelectTrigger id="project-before-cleanup-hook-failure-policy">
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                              <SelectItem value="warn_only">
                                {t(
                                  'settings.projects.lifecycleHooks.shared.failurePolicy.options.warnOnly'
                                )}
                              </SelectItem>
                              <SelectItem value="block_cleanup">
                                {t(
                                  'settings.projects.lifecycleHooks.beforeCleanup.failurePolicy.options.blockCleanup'
                                )}
                              </SelectItem>
                            </SelectContent>
                          </Select>
                        </div>
                      </div>
                    ) : null}
                  </div>
                </div>

                <div className="border-t border-border/60 pt-4 space-y-3">
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="min-w-0 space-y-1">
                      <h4 className="text-sm font-medium text-muted-foreground">
                        {t('settings.projects.lifecycleHooks.summary.title')}
                      </h4>
                      <p className="text-sm text-muted-foreground">
                        {t(
                          'settings.projects.lifecycleHooks.summary.description'
                        )}
                      </p>
                    </div>

                    {hasConfiguredLifecycleHooks ? (
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={() =>
                          setShowLatestHookOutcome((prev) => !prev)
                        }
                      >
                        {showLatestHookOutcome
                          ? t(
                              'settings.projects.lifecycleHooks.summary.hideLatest'
                            )
                          : t(
                              'settings.projects.lifecycleHooks.summary.loadLatest'
                            )}
                      </Button>
                    ) : null}
                  </div>

                  {hasConfiguredLifecycleHooks ? (
                    showLatestHookOutcome ? (
                      <div className="rounded-lg border border-border/60 bg-muted/10 p-4">
                        {latestLifecycleHookResult.isLoading ||
                        latestLifecycleHookResult.isFetching ? (
                          <p className="text-sm text-muted-foreground">
                            {t(
                              'settings.projects.lifecycleHooks.summary.loadingLatest'
                            )}
                          </p>
                        ) : latestLifecycleHookResult.isError ? (
                          <div className="space-y-3">
                            <p className="text-sm text-destructive">
                              {t(
                                'settings.projects.lifecycleHooks.summary.errorLoadingLatest'
                              )}
                            </p>
                            <Button
                              type="button"
                              variant="outline"
                              size="sm"
                              onClick={() =>
                                void latestLifecycleHookResult.refetch()
                              }
                            >
                              {t(
                                'settings.projects.lifecycleHooks.summary.retryLatest'
                              )}
                            </Button>
                          </div>
                        ) : latestLifecycleHookResult.data ? (
                          <div className="space-y-2">
                            <p className="text-xs text-muted-foreground">
                              {t(
                                'settings.projects.lifecycleHooks.summary.latestTask',
                                {
                                  task:
                                    latestLifecycleHookResult.data.task.title ||
                                    t(
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
                            {t(
                              'settings.projects.lifecycleHooks.summary.noRuns'
                            )}
                          </p>
                        )}
                      </div>
                    ) : (
                      <div className="rounded-lg border-dashed border-border/60 bg-muted/5 p-4 text-sm text-muted-foreground">
                        {t(
                          'settings.projects.lifecycleHooks.summary.loadLatestHint'
                        )}
                      </div>
                    )
                  ) : (
                    <div className="rounded-lg border-dashed border-border/60 bg-muted/5 p-4 text-sm text-muted-foreground">
                      {t(
                        'settings.projects.lifecycleHooks.summary.noneConfigured'
                      )}
                    </div>
                  )}
                </div>
              </div>

              {/* Save Button */}
              <div className="flex items-center justify-between pt-4 border-t">
                {hasUnsavedProjectChanges ? (
                  <span className="text-sm text-muted-foreground">
                    {t('settings.projects.save.unsavedChanges')}
                  </span>
                ) : (
                  <span />
                )}
                <div className="flex gap-2">
                  <Button
                    variant="outline"
                    onClick={handleDiscard}
                    disabled={saving || !hasUnsavedProjectChanges}
                  >
                    {t('settings.projects.save.discard')}
                  </Button>
                  <Button
                    onClick={handleSave}
                    disabled={saving || !hasUnsavedProjectChanges}
                  >
                    {saving ? (
                      <>
                        <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                        {t('settings.projects.save.saving')}
                      </>
                    ) : (
                      t('settings.projects.save.button')
                    )}
                  </Button>
                </div>
              </div>
              {error && (
                <Alert variant="destructive">
                  <AlertDescription>{error}</AlertDescription>
                </Alert>
              )}
              {success && (
                <Alert>
                  <AlertDescription>
                    {t('settings.projects.save.success')}
                  </AlertDescription>
                </Alert>
              )}
            </CardContent>
          </Card>

          {/* Repositories Section */}
          <Card>
            <CardHeader>
              <CardTitle>Repositories</CardTitle>
              <CardDescription>
                Manage the git repositories in this project
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {repoError && (
                <Alert variant="destructive">
                  <AlertDescription>{repoError}</AlertDescription>
                </Alert>
              )}

              {loadingRepos ? (
                <div className="flex items-center justify-center py-4">
                  <Loader2 className="h-5 w-5 animate-spin" />
                  <span className="ml-2 text-sm text-muted-foreground">
                    Loading repositories...
                  </span>
                </div>
              ) : (
                <div className="space-y-2">
                  {repositories.map((repo) => (
                    <div
                      key={repo.id}
                      className="flex items-center justify-between p-3 border rounded-md"
                    >
                      <div className="min-w-0 flex-1">
                        <div className="font-medium">{repo.display_name}</div>
                        <div className="text-sm text-muted-foreground truncate">
                          {repo.path}
                        </div>
                      </div>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleDeleteRepository(repo.id)}
                        disabled={deletingRepoId === repo.id}
                        title="Delete repository"
                      >
                        {deletingRepoId === repo.id ? (
                          <Loader2 className="h-4 w-4 animate-spin" />
                        ) : (
                          <Trash2 className="h-4 w-4" />
                        )}
                      </Button>
                    </div>
                  ))}

                  {repositories.length === 0 && !loadingRepos && (
                    <div className="text-center py-4 text-sm text-muted-foreground">
                      No repositories configured
                    </div>
                  )}

                  <Button
                    variant="outline"
                    size="sm"
                    onClick={handleAddRepository}
                    disabled={addingRepo}
                    className="w-full"
                  >
                    {addingRepo ? (
                      <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                    ) : (
                      <Plus className="h-4 w-4 mr-2" />
                    )}
                    Add Repository
                  </Button>
                </div>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>{t('settings.projects.scripts.title')}</CardTitle>
              <CardDescription>
                {t('settings.projects.scripts.description')}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {scriptsError && (
                <Alert variant="destructive">
                  <AlertDescription>{scriptsError}</AlertDescription>
                </Alert>
              )}

              {scriptsSuccess && (
                <Alert variant="success">
                  <AlertDescription className="font-medium">
                    Scripts saved successfully
                  </AlertDescription>
                </Alert>
              )}

              {repositories.length === 0 ? (
                <div className="text-center py-4 text-sm text-muted-foreground">
                  Add a repository above to configure scripts
                </div>
              ) : (
                <>
                  {/* Repository Selector for Scripts */}
                  <div className="space-y-2">
                    <Label htmlFor="scripts-repo-selector">Repository</Label>
                    <Select
                      value={selectedScriptsRepoId ?? ''}
                      onValueChange={setSelectedScriptsRepoId}
                    >
                      <SelectTrigger id="scripts-repo-selector">
                        <SelectValue placeholder="Select a repository" />
                      </SelectTrigger>
                      <SelectContent>
                        {repositories.map((repo) => (
                          <SelectItem key={repo.id} value={repo.id}>
                            {repo.display_name}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <p className="text-sm text-muted-foreground">
                      Configure scripts for each repository separately
                    </p>
                  </div>

                  {loadingProjectRepo ? (
                    <div className="flex items-center justify-center py-4">
                      <Loader2 className="h-5 w-5 animate-spin" />
                      <span className="ml-2 text-sm text-muted-foreground">
                        Loading scripts...
                      </span>
                    </div>
                  ) : scriptsDraft ? (
                    <>
                      <div className="space-y-2">
                        <Label htmlFor="setup-script">
                          {t('settings.projects.scripts.setup.label')}
                        </Label>
                        <AutoExpandingTextarea
                          id="setup-script"
                          value={scriptsDraft.setup_script}
                          onChange={(e) =>
                            updateScriptsDraft({ setup_script: e.target.value })
                          }
                          placeholder={placeholders.setup}
                          maxRows={12}
                          className="w-full px-3 py-2 border border-input bg-background text-foreground rounded-md focus:outline-none focus:ring-2 focus:ring-ring font-mono"
                        />
                        <p className="text-sm text-muted-foreground">
                          {t('settings.projects.scripts.setup.helper')}
                        </p>

                        <div className="flex items-center space-x-2 pt-2">
                          <Checkbox
                            id="parallel-setup-script"
                            checked={scriptsDraft.parallel_setup_script}
                            onCheckedChange={(checked) =>
                              updateScriptsDraft({
                                parallel_setup_script: checked === true,
                              })
                            }
                            disabled={!scriptsDraft.setup_script.trim()}
                          />
                          <Label
                            htmlFor="parallel-setup-script"
                            className="text-sm font-normal cursor-pointer"
                          >
                            {t('settings.projects.scripts.setup.parallelLabel')}
                          </Label>
                        </div>
                        <p className="text-sm text-muted-foreground pl-6">
                          {t('settings.projects.scripts.setup.parallelHelper')}
                        </p>
                      </div>

                      <div className="space-y-2">
                        <Label htmlFor="cleanup-script">
                          {t('settings.projects.scripts.cleanup.label')}
                        </Label>
                        <AutoExpandingTextarea
                          id="cleanup-script"
                          value={scriptsDraft.cleanup_script}
                          onChange={(e) =>
                            updateScriptsDraft({
                              cleanup_script: e.target.value,
                            })
                          }
                          placeholder={placeholders.cleanup}
                          maxRows={12}
                          className="w-full px-3 py-2 border border-input bg-background text-foreground rounded-md focus:outline-none focus:ring-2 focus:ring-ring font-mono"
                        />
                        <p className="text-sm text-muted-foreground">
                          {t('settings.projects.scripts.cleanup.helper')}
                        </p>
                      </div>

                      <div className="space-y-2">
                        <Label htmlFor="project-copy-files">
                          {t('settings.projects.scripts.copyFiles.label')}
                        </Label>
                        <CopyFilesField
                          id="project-copy-files"
                          name="copy_files"
                          value={scriptsDraft.copy_files}
                          onChange={(value) =>
                            updateScriptsDraft({ copy_files: value })
                          }
                          projectId={selectedProject.id}
                        />
                        <p className="text-sm text-muted-foreground">
                          {t('settings.projects.scripts.copyFiles.helper')}
                        </p>
                      </div>

                      {/* Scripts Save Buttons */}
                      <div className="flex items-center justify-between pt-4 border-t">
                        {hasUnsavedScriptsChanges ? (
                          <span className="text-sm text-muted-foreground">
                            {t('settings.projects.save.unsavedChanges')}
                          </span>
                        ) : (
                          <span />
                        )}
                        <div className="flex gap-2">
                          <Button
                            variant="outline"
                            onClick={handleDiscardScripts}
                            disabled={
                              !hasUnsavedScriptsChanges || savingScripts
                            }
                          >
                            {t('settings.projects.save.discard')}
                          </Button>
                          <Button
                            onClick={handleSaveScripts}
                            disabled={
                              !hasUnsavedScriptsChanges || savingScripts
                            }
                          >
                            {savingScripts && (
                              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                            )}
                            Save Scripts
                          </Button>
                        </div>
                      </div>
                    </>
                  ) : null}
                </>
              )}
            </CardContent>
          </Card>

          {/* Sticky Save Button for Project Name */}
          {hasUnsavedProjectChanges && (
            <div className="sticky bottom-0 z-10 bg-background/80 backdrop-blur-sm border-t py-4">
              <div className="flex items-center justify-between">
                <span className="text-sm text-muted-foreground">
                  {t('settings.projects.save.unsavedChanges')}
                </span>
                <div className="flex gap-2">
                  <Button
                    variant="outline"
                    onClick={handleDiscard}
                    disabled={saving}
                  >
                    {t('settings.projects.save.discard')}
                  </Button>
                  <Button onClick={handleSave} disabled={saving}>
                    {saving && (
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    )}
                    {t('settings.projects.save.button')}
                  </Button>
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
