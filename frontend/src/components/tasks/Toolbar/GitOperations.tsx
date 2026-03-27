import {
  ArrowRight,
  GitBranch as GitBranchIcon,
  GitPullRequest,
  RefreshCw,
  Settings,
  AlertTriangle,
  CheckCircle,
  Copy,
  Upload,
} from 'lucide-react';
import { Button } from '@/components/ui/button.tsx';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu.tsx';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip.tsx';
import { useCallback, useMemo, useState } from 'react';
import type {
  RepoBranchStatus,
  TaskWithAttemptStatus,
  Workspace,
} from 'shared/types';
import { ChangeTargetBranchDialog } from '@/components/dialogs/tasks/ChangeTargetBranchDialog';
import RepoSelector from '@/components/tasks/RepoSelector';
import { RebaseDialog } from '@/components/dialogs/tasks/RebaseDialog';
import { useTranslation } from 'react-i18next';
import { useAttemptRepo } from '@/hooks/task-attempts/useAttemptRepo';
import { useGitOperations } from '@/hooks/task-attempts/useGitOperations';
import { useRepoBranches } from '@/hooks';
import { useCopyToClipboard } from '@/hooks/utils/useCopyToClipboard';

function encodeCompareRef(ref: string): string {
  const trimmed = ref.trim();
  if (trimmed.startsWith('<') && trimmed.endsWith('>')) return trimmed;
  return encodeURIComponent(trimmed);
}

interface GitOperationsProps {
  selectedAttempt: Workspace;
  task: TaskWithAttemptStatus;
  branchStatus: RepoBranchStatus[] | null;
  isAttemptRunning: boolean;
  selectedBranch: string | null;
  layout?: 'horizontal' | 'vertical';
}

export type GitOperationsInputs = Omit<GitOperationsProps, 'selectedAttempt'>;

function GitOperations(props: GitOperationsProps) {
  const {
    selectedAttempt,
    branchStatus,
    isAttemptRunning,
    selectedBranch,
    layout = 'horizontal',
  } = props;
  const { t } = useTranslation('tasks');
  const copyToClipboard = useCopyToClipboard();

  const { repos, selectedRepoId, setSelectedRepoId } = useAttemptRepo(
    selectedAttempt.id
  );
  const git = useGitOperations(selectedAttempt.id, selectedRepoId ?? undefined);
  const { data: branches = [] } = useRepoBranches(selectedRepoId);
  const isChangingTargetBranch = git.states.changeTargetBranchPending;

  // Local state for git operations
  const [merging, setMerging] = useState(false);
  const [pushing, setPushing] = useState(false);
  const [rebasing, setRebasing] = useState(false);
  const [mergeSuccess, setMergeSuccess] = useState(false);
  const [pushSuccess, setPushSuccess] = useState(false);

  // Target branch change handlers
  const handleChangeTargetBranchClick = async (newBranch: string) => {
    const repoId = getSelectedRepoId();
    if (!repoId) return;
    await git.actions.changeTargetBranch({
      newTargetBranch: newBranch,
      repoId,
    });
  };

  const handleChangeTargetBranchDialogOpen = async () => {
    try {
      const result = await ChangeTargetBranchDialog.show({
        branches,
        isChangingTargetBranch: isChangingTargetBranch,
      });

      if (result.action === 'confirmed' && result.branchName) {
        await handleChangeTargetBranchClick(result.branchName);
      }
    } catch (error) {
      // User cancelled - do nothing
    }
  };

  const getSelectedRepoId = useCallback(() => {
    return selectedRepoId ?? repos[0]?.id;
  }, [selectedRepoId, repos]);

  const getSelectedRepoStatus = useCallback(() => {
    const repoId = getSelectedRepoId();
    return branchStatus?.find((r) => r.repo_id === repoId);
  }, [branchStatus, getSelectedRepoId]);

  // Memoize the selected repo status for use in button disabled states
  const selectedRepoStatus = useMemo(
    () => getSelectedRepoStatus(),
    [getSelectedRepoStatus]
  );

  const hasConflictsCalculated =
    (selectedRepoStatus?.conflicted_files?.length ?? 0) > 0;

  const mergeButtonLabel = useMemo(() => {
    if (mergeSuccess) return t('git.states.merged');
    if (merging) return t('git.states.merging');
    return t('git.states.merge');
  }, [mergeSuccess, merging, t]);

  const pushButtonLabel = useMemo(() => {
    if (pushSuccess) return t('git.states.pushed');
    if (pushing) return t('git.states.pushing');
    return t('git.states.push');
  }, [pushSuccess, pushing, t]);

  const rebaseButtonLabel = useMemo(() => {
    if (rebasing) return t('git.states.rebasing');
    return t('git.states.rebase');
  }, [rebasing, t]);

  const handleMergeClick = async () => {
    // Directly perform merge without checking branch status
    await performMerge();
  };

  const handlePushClick = async () => {
    try {
      setPushing(true);
      const repoId = getSelectedRepoId();
      if (!repoId) return;
      await git.actions.push({ repo_id: repoId });
      setPushSuccess(true);
      setTimeout(() => setPushSuccess(false), 2000);
    } finally {
      setPushing(false);
    }
  };

  const performMerge = async () => {
    try {
      setMerging(true);
      const repoId = getSelectedRepoId();
      if (!repoId) return;
      await git.actions.merge({
        repoId,
      });
      setMergeSuccess(true);
      setTimeout(() => setMergeSuccess(false), 2000);
    } finally {
      setMerging(false);
    }
  };

  const handleRebaseWithNewBranchAndUpstream = async (
    newBaseBranch: string,
    selectedUpstream: string
  ) => {
    setRebasing(true);
    try {
      const repoId = getSelectedRepoId();
      if (!repoId) return;
      await git.actions.rebase({
        repoId,
        newBaseBranch: newBaseBranch,
        oldBaseBranch: selectedUpstream,
      });
    } finally {
      setRebasing(false);
    }
  };

  const handleRebaseDialogOpen = async () => {
    try {
      const defaultTargetBranch = getSelectedRepoStatus()?.target_branch_name;
      const result = await RebaseDialog.show({
        branches,
        isRebasing: rebasing,
        initialTargetBranch: defaultTargetBranch,
        initialUpstreamBranch: defaultTargetBranch,
      });
      if (
        result.action === 'confirmed' &&
        result.branchName &&
        result.upstreamBranch
      ) {
        await handleRebaseWithNewBranchAndUpstream(
          result.branchName,
          result.upstreamBranch
        );
      }
    } catch (error) {
      // User cancelled - do nothing
    }
  };

  const isVertical = layout === 'vertical';

  const containerClasses = isVertical
    ? 'grid grid-cols-1 items-start gap-3 overflow-hidden'
    : 'flex items-center gap-2 overflow-hidden';

  const settingsBtnClasses = isVertical
    ? 'inline-flex h-5 w-5 p-0 hover:bg-muted'
    : 'hidden md:inline-flex h-5 w-5 p-0 hover:bg-muted';

  const actionsClasses = isVertical
    ? 'flex flex-wrap items-center gap-2'
    : 'shrink-0 flex flex-wrap items-center gap-2 overflow-y-hidden overflow-x-visible max-h-8';

  const statusChips = (
    <div className="flex items-center gap-2 text-xs min-w-0 overflow-hidden whitespace-nowrap">
      {(() => {
        const commitsAhead = selectedRepoStatus?.commits_ahead ?? 0;
        const commitsBehind = selectedRepoStatus?.commits_behind ?? 0;

        if (hasConflictsCalculated) {
          return (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-amber-100/60 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300">
              <AlertTriangle className="h-3.5 w-3.5" />
              {t('git.status.conflicts')}
            </span>
          );
        }

        if (selectedRepoStatus?.is_rebase_in_progress) {
          return (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-amber-100/60 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300">
              <RefreshCw className="h-3.5 w-3.5 animate-spin" />
              {t('git.states.rebasing')}
            </span>
          );
        }

        if (mergeSuccess) {
          return (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-emerald-100/70 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300">
              <CheckCircle className="h-3.5 w-3.5" />
              {t('git.states.merged')}
            </span>
          );
        }

        const chips: React.ReactNode[] = [];
        if (commitsAhead > 0) {
          chips.push(
            <span
              key="ahead"
              className="hidden sm:inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-emerald-100/70 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300"
            >
              +{commitsAhead} {t('git.status.commits', { count: commitsAhead })}{' '}
              {t('git.status.ahead')}
            </span>
          );
        }
        if (commitsBehind > 0) {
          chips.push(
            <span
              key="behind"
              className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-amber-100/60 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300"
            >
              {commitsBehind}{' '}
              {t('git.status.commits', { count: commitsBehind })}{' '}
              {t('git.status.behind')}
            </span>
          );
        }
        if (chips.length > 0)
          return <div className="flex items-center gap-2">{chips}</div>;

        return (
          <span className="text-muted-foreground hidden sm:inline">
            {t('git.status.upToDate')}
          </span>
        );
      })()}
    </div>
  );

  const taskBranch = selectedAttempt.branch || '<task_branch>';
  const targetBranchName =
    getSelectedRepoStatus()?.target_branch_name ??
    selectedBranch ??
    '<base_branch>';
  const compareUrlTemplate = `https://<git-host>/<owner>/<repo>/compare/${encodeCompareRef(targetBranchName)}...${encodeCompareRef(taskBranch)}?expand=1`;
  const suggestedCommands = `git push -u origin ${taskBranch}\n${compareUrlTemplate}`;

  const copyBranchLabel = t('git.prInfo.branchName');
  const copyCompareUrlLabel = t('git.prInfo.compareUrlTemplate');
  const copyCommandsLabel = t('git.prInfo.suggestedCommands');

  const branchChips = (
    <>
      {/* Task branch chip */}
      <TooltipProvider>
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="hidden sm:inline-flex items-center gap-1.5 max-w-[280px] px-2 py-0.5 rounded-full bg-muted text-xs font-medium min-w-0">
              <GitBranchIcon className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
              <span className="truncate">{selectedAttempt.branch}</span>
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {t('git.labels.taskBranch')}
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>

      <ArrowRight className="hidden sm:inline h-4 w-4 text-muted-foreground" />

      {/* Target branch chip + change button */}
      <div className="flex items-center gap-1 min-w-0">
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="inline-flex items-center gap-1.5 max-w-[280px] px-2 py-0.5 rounded-full bg-muted text-xs font-medium min-w-0">
                <GitBranchIcon className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                <span className="truncate">
                  {getSelectedRepoStatus()?.target_branch_name ||
                    selectedBranch ||
                    t('git.branch.current')}
                </span>
              </span>
            </TooltipTrigger>
            <TooltipContent side="bottom">
              {t('rebase.dialog.targetLabel')}
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>

        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="xs"
                onClick={handleChangeTargetBranchDialogOpen}
                disabled={isAttemptRunning || hasConflictsCalculated}
                className={settingsBtnClasses}
                aria-label={t('branches.changeTarget.dialog.title')}
              >
                <Settings className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom">
              {t('branches.changeTarget.dialog.title')}
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>
    </>
  );

  return (
    <div className="w-full border-b py-2">
      <div className={containerClasses}>
        {isVertical ? (
          <>
            {repos.length > 1 && (
              <RepoSelector
                repos={repos}
                selectedRepoId={getSelectedRepoId() ?? null}
                onRepoSelect={setSelectedRepoId}
                disabled={isAttemptRunning}
                placeholder={t('repos.selector.placeholder', 'Select repo')}
              />
            )}
            <div className="flex flex-wrap items-center gap-2 min-w-0">
              {branchChips}
              {statusChips}
            </div>
          </>
        ) : (
          <>
            {repos.length > 0 && (
              <RepoSelector
                repos={repos}
                selectedRepoId={getSelectedRepoId() ?? null}
                onRepoSelect={setSelectedRepoId}
                disabled={isAttemptRunning}
                placeholder={t('repos.selector.placeholder', 'Select repo')}
                className="w-auto max-w-[200px] rounded-full bg-muted border-0 h-6 px-2 py-0.5 text-xs font-medium"
              />
            )}
            <div className="flex flex-1 items-center justify-center gap-2 min-w-0 overflow-hidden">
              <div className="flex items-center gap-2 min-w-0 overflow-hidden">
                {branchChips}
              </div>
              {statusChips}
            </div>
          </>
        )}

        {/* Right: Actions */}
        {selectedRepoStatus && (
          <div className={actionsClasses}>
            <Button
              onClick={handleMergeClick}
              disabled={
                merging ||
                hasConflictsCalculated ||
                isAttemptRunning ||
                ((selectedRepoStatus?.commits_ahead ?? 0) === 0 &&
                  !pushSuccess &&
                  !mergeSuccess)
              }
              variant="outline"
              size="xs"
              className="border-success text-success hover:bg-success gap-1 shrink-0"
              aria-label={mergeButtonLabel}
            >
              <GitBranchIcon className="h-3.5 w-3.5" />
              <span className="truncate max-w-[10ch]">{mergeButtonLabel}</span>
            </Button>

            <Button
              onClick={handlePushClick}
              disabled={
                pushing ||
                isAttemptRunning ||
                hasConflictsCalculated ||
                ((selectedRepoStatus?.commits_ahead ?? 0) === 0 &&
                  !pushSuccess &&
                  !mergeSuccess)
              }
              variant="outline"
              size="xs"
              className="border-info text-info hover:bg-info gap-1 shrink-0"
              aria-label={pushButtonLabel}
            >
              <Upload className="h-3.5 w-3.5" />
              <span className="truncate max-w-[10ch]">{pushButtonLabel}</span>
            </Button>

            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="outline"
                  size="xs"
                  className="gap-1 shrink-0"
                  aria-label={t('git.prInfo.title')}
                >
                  <GitPullRequest className="h-3.5 w-3.5" />
                  <span className="truncate max-w-[10ch]">
                    {t('git.states.prInfo')}
                  </span>
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem
                  onClick={() => copyToClipboard(copyBranchLabel, taskBranch)}
                >
                  <Copy className="h-3.5 w-3.5" />
                  {t('git.prInfo.copyBranch')}
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() =>
                    copyToClipboard(copyCompareUrlLabel, compareUrlTemplate)
                  }
                >
                  <Copy className="h-3.5 w-3.5" />
                  {t('git.prInfo.copyCompareUrlTemplate')}
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  onClick={() =>
                    copyToClipboard(copyCommandsLabel, suggestedCommands)
                  }
                >
                  <Copy className="h-3.5 w-3.5" />
                  {t('git.prInfo.copySuggestedCommands')}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>

            <Button
              onClick={handleRebaseDialogOpen}
              disabled={rebasing || isAttemptRunning || hasConflictsCalculated}
              variant="outline"
              size="xs"
              className="border-warning text-warning hover:bg-warning gap-1 shrink-0"
              aria-label={rebaseButtonLabel}
            >
              <RefreshCw
                className={`h-3.5 w-3.5 ${rebasing ? 'animate-spin' : ''}`}
              />
              <span className="truncate max-w-[10ch]">{rebaseButtonLabel}</span>
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}

export default GitOperations;
