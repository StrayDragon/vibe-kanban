import { useState, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import RepoBranchSelector from '@/components/tasks/RepoBranchSelector';
import { ExecutorProfileSelector } from '@/components/settings';
import { useAttemptCreation } from '@/hooks/useAttemptCreation';
import {
  useNavigateWithSearch,
  useTask,
  useAttempt,
  useRepoBranchSelection,
  useProjectRepos,
} from '@/hooks';
import { useTaskGroup } from '@/hooks/useTaskGroup';
import { useTaskAttemptsWithSessions } from '@/hooks/useTaskAttempts';
import { useProject } from '@/contexts/ProjectContext';
import { useUserSystem } from '@/components/ConfigProvider';
import { paths } from '@/lib/paths';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/lib/modals';
import type { ExecutorProfileId, BaseCodingAgent } from 'shared/types';
import { useKeySubmitTask, Scope } from '@/keyboard';
import { useCliDependencyPreflight } from '@/hooks/useCliDependencyPreflight';
import type {
  TaskGroupGraphNode,
  TaskGroupNodeBaseStrategy,
} from '@/types/task-group';

const getNodeTaskId = (node: TaskGroupGraphNode): string | undefined =>
  node.task_id ?? node.taskId;

const getNodeExecutorProfileId = (
  node: TaskGroupGraphNode
): ExecutorProfileId | null =>
  node.executor_profile_id ?? node.executorProfileId ?? null;

const getNodeBaseStrategy = (
  node: TaskGroupGraphNode
): TaskGroupNodeBaseStrategy =>
  node.base_strategy ?? node.baseStrategy ?? 'topology';

export interface CreateAttemptDialogProps {
  taskId: string;
}

const CreateAttemptDialogImpl = NiceModal.create<CreateAttemptDialogProps>(
  ({ taskId }) => {
    const modal = useModal();
    const navigate = useNavigateWithSearch();
    const { projectId } = useProject();
    const { t } = useTranslation('tasks');
    const { profiles, config } = useUserSystem();
    const { createAttempt, isCreating, error } = useAttemptCreation({
      taskId,
      onSuccess: (attempt) => {
        if (projectId) {
          navigate(paths.attempt(projectId, taskId, attempt.id));
        }
      },
    });

    const [userSelectedProfile, setUserSelectedProfile] =
      useState<ExecutorProfileId | null>(null);

    const { data: attempts = [], isLoading: isLoadingAttempts } =
      useTaskAttemptsWithSessions(taskId, {
        enabled: modal.visible,
      });

    const { data: task, isLoading: isLoadingTask } = useTask(taskId, {
      enabled: modal.visible,
    });
    const taskGroupId = task?.task_group_id ?? null;
    const { data: taskGroup } = useTaskGroup(taskGroupId ?? undefined, {
      enabled: modal.visible && !!taskGroupId,
    });

    const parentAttemptId = task?.parent_workspace_id ?? undefined;
    const { data: parentAttempt, isLoading: isLoadingParent } = useAttempt(
      parentAttemptId,
      { enabled: modal.visible && !!parentAttemptId }
    );

    const { data: projectRepos = [], isLoading: isLoadingRepos } =
      useProjectRepos(projectId, { enabled: modal.visible });

    const taskGroupGraph = taskGroup?.graph ?? taskGroup?.graph_json;
    const taskGroupNode = useMemo(() => {
      if (!taskGroupGraph || !task) return null;
      return taskGroupGraph.nodes.find(
        (node) => getNodeTaskId(node) === task.id
      );
    }, [taskGroupGraph, task]);

    const nodeExecutorProfile = taskGroupNode
      ? getNodeExecutorProfileId(taskGroupNode)
      : null;
    const nodeBaseStrategy = taskGroupNode
      ? getNodeBaseStrategy(taskGroupNode)
      : 'topology';
    const baselineRef = taskGroup?.baseline_ref ?? null;
    const hasBaselineRef = Boolean(baselineRef && baselineRef.trim().length > 0);
    const isTaskGroupNode = Boolean(
      task?.task_group_id && task?.task_group_node_id
    );
    const usesBaselineRef =
      isTaskGroupNode && nodeBaseStrategy === 'baseline' && hasBaselineRef;
    const usesTopologyBase =
      isTaskGroupNode && nodeBaseStrategy === 'topology';
    const usesFixedBase = isTaskGroupNode && (usesBaselineRef || usesTopologyBase);

    const {
      configs: repoBranchConfigs,
      isLoading: isLoadingBranches,
      setRepoBranch,
      getWorkspaceRepoInputs,
      reset: resetBranchSelection,
    } = useRepoBranchSelection({
      repos: projectRepos,
      initialBranch: usesFixedBase ? baselineRef ?? undefined : parentAttempt?.branch,
      enabled: modal.visible && projectRepos.length > 0 && !usesFixedBase,
    });

    const latestAttempt = useMemo(() => {
      if (attempts.length === 0) return null;
      return attempts.reduce((latest, attempt) =>
        new Date(attempt.created_at) > new Date(latest.created_at)
          ? attempt
          : latest
      );
    }, [attempts]);

    useEffect(() => {
      if (!modal.visible) {
        setUserSelectedProfile(null);
        resetBranchSelection();
      }
    }, [modal.visible, resetBranchSelection]);

    const defaultProfile: ExecutorProfileId | null = useMemo(() => {
      if (nodeExecutorProfile) {
        return nodeExecutorProfile;
      }
      if (latestAttempt?.session?.executor) {
        const lastExec = latestAttempt.session.executor as BaseCodingAgent;
        // If the last attempt used the same executor as the user's current preference,
        // we assume they want to use their preferred variant as well.
        // Otherwise, we default to the "default" variant (null) since we don't know
        // what variant they used last time (TaskAttempt doesn't store it).
        const variant =
          config?.executor_profile?.executor === lastExec
            ? config.executor_profile.variant
            : null;

        return {
          executor: lastExec,
          variant,
        };
      }
      return config?.executor_profile ?? null;
    }, [nodeExecutorProfile, latestAttempt?.session?.executor, config?.executor_profile]);

    const isNodeProfileLocked = Boolean(nodeExecutorProfile);
    const effectiveProfile = isNodeProfileLocked
      ? nodeExecutorProfile
      : userSelectedProfile ?? defaultProfile;

    const selectedAgent = effectiveProfile?.executor ?? null;
    const { data: cliPreflight, isLoading: preflightLoading } =
      useCliDependencyPreflight(selectedAgent, modal.visible);

    const isLoadingInitial =
      isLoadingRepos ||
      (!usesFixedBase && isLoadingBranches) ||
      isLoadingAttempts ||
      isLoadingTask ||
      isLoadingParent;

    const allBranchesSelected = usesFixedBase
      ? projectRepos.length > 0
      : repoBranchConfigs.every((c) => c.targetBranch !== null);

    const canCreate = Boolean(
      effectiveProfile &&
        allBranchesSelected &&
        projectRepos.length > 0 &&
        !isCreating &&
        !isLoadingInitial
    );

    const handleCreate = async () => {
      if (
        !effectiveProfile ||
        !allBranchesSelected ||
        projectRepos.length === 0
      )
        return;
      try {
        const repos = usesFixedBase
          ? projectRepos.map((repo) => ({
              repo_id: repo.id,
              target_branch: baselineRef!.trim(),
            }))
          : getWorkspaceRepoInputs();

        await createAttempt({
          profile: effectiveProfile,
          repos,
        });

        modal.hide();
      } catch (err) {
        console.error('Failed to create attempt:', err);
      }
    };

    const handleOpenChange = (open: boolean) => {
      if (!open) modal.hide();
    };

    useKeySubmitTask(handleCreate, {
      enabled: modal.visible && canCreate,
      scope: Scope.DIALOG,
      preventDefault: true,
    });

    return (
      <Dialog open={modal.visible} onOpenChange={handleOpenChange}>
        <DialogContent className="sm:max-w-[500px]">
          <DialogHeader>
            <DialogTitle>{t('createAttemptDialog.title')}</DialogTitle>
            <DialogDescription>
              {t('createAttemptDialog.description')}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            {profiles && (
              <div className="space-y-2">
                <ExecutorProfileSelector
                  profiles={profiles}
                  selectedProfile={effectiveProfile}
                  onProfileSelect={setUserSelectedProfile}
                  showLabel={true}
                  disabled={isNodeProfileLocked}
                />
                {isNodeProfileLocked && (
                  <div className="text-xs text-muted-foreground">
                    Profile is set by the workflow node.
                  </div>
                )}
              </div>
            )}

            {selectedAgent && (
              <Alert
                variant={
                  cliPreflight?.agent.type === 'NOT_FOUND'
                    ? 'destructive'
                    : 'default'
                }
              >
                <AlertTitle>{t('createAttemptDialog.preflight.title')}</AlertTitle>
                <AlertDescription>
                  {preflightLoading
                    ? t('createAttemptDialog.preflight.checking')
                    : cliPreflight?.agent.type === 'LOGIN_DETECTED'
                      ? t('createAttemptDialog.preflight.agentReady')
                      : cliPreflight?.agent.type === 'INSTALLATION_FOUND'
                        ? t('createAttemptDialog.preflight.agentInstalled')
                        : cliPreflight?.agent.type === 'NOT_FOUND'
                          ? t('createAttemptDialog.preflight.agentNotFound')
                          : null}
                </AlertDescription>
              </Alert>
            )}

            {usesTopologyBase ? (
              <div className="space-y-1 text-xs text-muted-foreground">
                <div className="font-medium text-foreground">Base branch</div>
                <div className="rounded-md border bg-muted/30 px-3 py-2 text-xs">
                  Derived from upstream nodes
                </div>
                {hasBaselineRef && (
                  <div className="text-[11px] text-muted-foreground">
                    Fallback: {baselineRef}
                  </div>
                )}
              </div>
            ) : usesBaselineRef ? (
              <div className="space-y-1 text-xs text-muted-foreground">
                <div className="font-medium text-foreground">
                  {t('taskFormDialog.baselineLabel', 'Baseline branch')}
                </div>
                <div className="rounded-md border bg-muted/30 px-3 py-2 text-xs">
                  {baselineRef}
                </div>
              </div>
            ) : (
              <RepoBranchSelector
                configs={repoBranchConfigs}
                onBranchChange={setRepoBranch}
                isLoading={isLoadingBranches}
                className="space-y-2"
              />
            )}

            {error && (
              <div className="text-sm text-destructive">
                {t('createAttemptDialog.error')}
              </div>
            )}
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => modal.hide()}
              disabled={isCreating}
            >
              {t('common:buttons.cancel')}
            </Button>
            <Button onClick={handleCreate} disabled={!canCreate}>
              {isCreating
                ? t('createAttemptDialog.creating')
                : t('createAttemptDialog.start')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const CreateAttemptDialog = defineModal<CreateAttemptDialogProps, void>(
  CreateAttemptDialogImpl
);
