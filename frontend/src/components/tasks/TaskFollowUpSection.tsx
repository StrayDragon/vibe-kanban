import { Loader2, AlertCircle, Clock } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { useQuery } from '@tanstack/react-query';
//
import { useEffect, useMemo, useRef, useState, useCallback } from 'react';
import {
  ScratchType,
  type ProjectRepo,
  type TaskWithAttemptStatus,
} from 'shared/types';
import { useBranchStatus } from '@/hooks';
import { useAttemptRepo } from '@/hooks/task-attempts/useAttemptRepo';
import { useAttemptExecution } from '@/hooks/task-attempts/useAttemptExecution';
import { useUserSystem } from '@/components/ConfigProvider';
import { cn } from '@/lib/utils';
//
import { useReview } from '@/contexts/ReviewProvider';
import { useClickedElements } from '@/contexts/ClickedElementsProvider';
import { useEntries } from '@/contexts/EntriesContext';
import { useKeySubmitFollowUp, Scope } from '@/keyboard';
import { useHotkeysContext } from 'react-hotkeys-hook';
import { useProject } from '@/contexts/ProjectContext';
//
import { useAttemptBranch } from '@/hooks/task-attempts/useAttemptBranch';
import { FollowUpConflictSection } from '@/components/tasks/follow-up/FollowUpConflictSection';
import { FollowUpActionBar } from '@/components/tasks/follow-up/FollowUpActionBar';
import { ClickedElementsBanner } from '@/components/tasks/ClickedElementsBanner';
import WYSIWYGEditor from '@/components/ui/wysiwyg';
import { useRetryUi } from '@/contexts/RetryUiContext';
import { useFollowUpEditor } from '@/hooks/task-attempts/useFollowUpEditor';
import { useFollowUpSend } from '@/hooks/task-attempts/useFollowUpSend';
import { useVariant } from '@/hooks/config/useVariant';
import type {
  DraftFollowUpData,
  ExecutorAction,
  ExecutorProfileId,
} from 'shared/types';
import { buildResolveConflictsInstructions } from '@/lib/conflicts';
import { useTranslation } from 'react-i18next';
import { useScratch } from '@/hooks/scratch/useScratch';
import { useDebouncedCallback } from '@/hooks/utils/useDebouncedCallback';
import { useQueueStatus } from '@/hooks/sessions/useQueueStatus';
import { attemptsApi, projectsApi } from '@/lib/api';
import { GitHubCommentsDialog } from '@/components/dialogs/tasks/GitHubCommentsDialog';
import { ConfirmDialog } from '@/components/dialogs';
import type { NormalizedComment } from '@/components/ui/wysiwyg/nodes/github-comment-node';
import type { Session } from 'shared/types';

interface TaskFollowUpSectionProps {
  task: TaskWithAttemptStatus;
  session?: Session;
}

export function TaskFollowUpSection({
  task,
  session,
}: TaskFollowUpSectionProps) {
  const { t } = useTranslation('tasks');
  const { projectId } = useProject();

  // Derive IDs from session
  const workspaceId = session?.workspace_id;
  const sessionId = session?.id;

  const {
    isAttemptRunning,
    stopExecution,
    forceStopExecution,
    isStopping,
    processes,
  } = useAttemptExecution(workspaceId, task.id);

  const { data: branchStatus, refetch: refetchBranchStatus } =
    useBranchStatus(workspaceId);
  const { repos, selectedRepoId } = useAttemptRepo(workspaceId);

  const getSelectedRepoId = useCallback(() => {
    return selectedRepoId ?? repos[0]?.id;
  }, [selectedRepoId, repos]);

  const repoIds = useMemo(() => repos.map((repo) => repo.id), [repos]);
  const { data: projectRepoScripts = [] } = useQuery<ProjectRepo[]>({
    queryKey: ['projectRepoScripts', projectId, repoIds],
    queryFn: async () => {
      if (!projectId) return [];
      return Promise.all(
        repoIds.map((repoId) => projectsApi.getRepository(projectId, repoId))
      );
    },
    enabled: !!projectId && repoIds.length > 0,
  });

  const hasSetupScript = useMemo(
    () => projectRepoScripts.some((repo) => repo.setup_script?.trim()),
    [projectRepoScripts]
  );
  const hasCleanupScript = useMemo(
    () => projectRepoScripts.some((repo) => repo.cleanup_script?.trim()),
    [projectRepoScripts]
  );
  const hasAnyScript = hasSetupScript || hasCleanupScript;

  const repoWithConflicts = useMemo(
    () =>
      branchStatus?.find(
        (r) => r.is_rebase_in_progress || (r.conflicted_files?.length ?? 0) > 0
      ),
    [branchStatus]
  );
  const { branch: attemptBranch, refetch: refetchAttemptBranch } =
    useAttemptBranch(workspaceId);
  const { profiles } = useUserSystem();
  const { comments, generateReviewMarkdown, clearComments } = useReview();
  const {
    generateMarkdown: generateClickedMarkdown,
    clearElements: clearClickedElements,
  } = useClickedElements();
  const { enableScope, disableScope } = useHotkeysContext();
  const handleForceStop = useCallback(async () => {
    if (!isAttemptRunning || isStopping) return;
    const result = await ConfirmDialog.show({
      title: t('followUp.forceStopTitle', 'Force stop this run?'),
      message: t(
        'followUp.forceStopMessage',
        'This will interrupt the running process and mark it as stopped. Use this if the normal stop action is not working.'
      ),
      confirmText: t('followUp.forceStop', 'Force stop'),
      variant: 'destructive',
    });

    if (result === 'confirmed') {
      try {
        await forceStopExecution();
      } catch (error) {
        console.error('Failed to force stop executions:', error);
      }
    }
  }, [forceStopExecution, isAttemptRunning, isStopping, t]);

  const reviewMarkdown = useMemo(
    () => generateReviewMarkdown(),
    [generateReviewMarkdown]
  );

  const clickedMarkdown = useMemo(
    () => generateClickedMarkdown(),
    [generateClickedMarkdown]
  );

  // Non-editable conflict resolution instructions (derived, like review comments)
  const conflictResolutionInstructions = useMemo(() => {
    if (!repoWithConflicts?.conflicted_files?.length) return null;
    return buildResolveConflictsInstructions(
      attemptBranch,
      repoWithConflicts.target_branch_name,
      repoWithConflicts.conflicted_files,
      repoWithConflicts.conflict_op ?? null,
      repoWithConflicts.repo_name
    );
  }, [attemptBranch, repoWithConflicts]);

  // Editor state (persisted via scratch)
  const {
    scratch,
    updateScratch,
    isLoading: isScratchLoading,
  } = useScratch(ScratchType.DRAFT_FOLLOW_UP, sessionId);

  // Derive the message and variant from scratch
  const scratchData: DraftFollowUpData | undefined =
    scratch?.payload?.type === 'DRAFT_FOLLOW_UP'
      ? scratch.payload.data
      : undefined;

  // Track whether the follow-up textarea is focused
  const [isTextareaFocused, setIsTextareaFocused] = useState(false);

  // Variant selection - derive default from latest process
  const latestProfileId = useMemo<ExecutorProfileId | null>(() => {
    if (!processes?.length) return null;

    const extractProfile = (
      action: ExecutorAction | null
    ): ExecutorProfileId | null => {
      let curr: ExecutorAction | null = action;
      while (curr) {
        const typ = curr.typ;
        switch (typ.type) {
          case 'CodingAgentInitialRequest':
          case 'CodingAgentFollowUpRequest':
            return typ.executor_profile_id;
          case 'ScriptRequest':
            curr = curr.next_action;
            continue;
        }
      }
      return null;
    };
    return (
      processes
        .slice()
        .reverse()
        .map((p) => extractProfile(p.executor_action ?? null))
        .find((pid) => pid !== null) ?? null
    );
  }, [processes]);

  const processVariant = latestProfileId?.variant ?? null;

  const currentProfile = useMemo(() => {
    if (!latestProfileId) return null;
    return profiles?.[latestProfileId.executor] ?? null;
  }, [latestProfileId, profiles]);

  // Variant selection with priority: user selection > scratch > process
  const { selectedVariant, setSelectedVariant: setVariantFromHook } =
    useVariant({
      processVariant,
      scratchVariant: scratchData?.variant,
    });

  // Save scratch helper (used for both message and variant changes)
  const saveToScratch = useCallback(
    async (message: string, variant: string | null) => {
      if (!workspaceId) return;
      // Don't create empty scratch entries - only save if there's actual content,
      // a variant is selected, or scratch already exists (to allow clearing a draft)
      if (!message.trim() && !variant && !scratch) return;
      try {
        await updateScratch({
          payload: {
            type: 'DRAFT_FOLLOW_UP',
            data: { message, variant },
          },
        });
      } catch (e) {
        console.error('Failed to save follow-up draft', e);
      }
    },
    [workspaceId, updateScratch, scratch]
  );

  // Debounced save for message changes
  const { debounced: setFollowUpMessage, cancel: cancelDebouncedSave } =
    useDebouncedCallback(
      useCallback(
        (value: string) => saveToScratch(value, selectedVariant),
        [saveToScratch, selectedVariant]
      ),
      500
    );

  // During retry, follow-up box is greyed/disabled (not hidden)
  // Use RetryUi context so optimistic retry immediately disables this box
  const { activeRetryProcessId } = useRetryUi();
  const isRetryActive = !!activeRetryProcessId;

  // Queue status for queuing follow-up messages while agent is running
  const {
    isQueued,
    queuedMessage,
    isLoading: isQueueLoading,
    queueMessage,
    cancelQueue,
    refresh: refreshQueueStatus,
  } = useQueueStatus(sessionId);

  const {
    localMessage,
    setLocalMessage,
    displayMessage,
    updateMessage,
    appendToMessage,
    handlePasteFiles,
  } = useFollowUpEditor({
    workspaceId,
    isQueued,
    queuedMessage,
    cancelQueue,
    setFollowUpMessage,
  });

  // Wrapper to update variant and save to scratch immediately
  const setSelectedVariant = useCallback(
    (variant: string | null) => {
      setVariantFromHook(variant);
      // Save immediately when user changes variant
      saveToScratch(localMessage, variant);
    },
    [setVariantFromHook, saveToScratch, localMessage]
  );

  // Sync local message from scratch when it loads (but not while user is typing)
  useEffect(() => {
    if (isScratchLoading) return;
    if (isTextareaFocused) return; // Don't overwrite while user is typing
    setLocalMessage(scratchData?.message ?? '');
  }, [
    isScratchLoading,
    scratchData?.message,
    isTextareaFocused,
    setLocalMessage,
  ]);

  // Track previous process count to detect new processes
  const prevProcessCountRef = useRef(processes.length);

  // Refresh queue status when execution stops OR when a new process starts
  useEffect(() => {
    const prevCount = prevProcessCountRef.current;
    prevProcessCountRef.current = processes.length;

    if (!workspaceId) return;

    // Refresh when execution stops
    if (!isAttemptRunning) {
      refreshQueueStatus();
      return;
    }

    // Refresh when a new process starts (could be queued message consumption or follow-up)
    if (processes.length > prevCount) {
      refreshQueueStatus();
      // Re-sync local message from current scratch state
      // If scratch was deleted, scratchData will be undefined, so localMessage becomes ''
      setLocalMessage(scratchData?.message ?? '');
    }
  }, [
    isAttemptRunning,
    workspaceId,
    processes.length,
    refreshQueueStatus,
    scratchData?.message,
    setLocalMessage,
  ]);

  // Check if there's a pending approval - users shouldn't be able to type during approvals
  const { entries } = useEntries();
  const hasPendingApproval = useMemo(() => {
    return entries.some((entry) => {
      if (entry.type !== 'NORMALIZED_ENTRY') return false;
      const entryType = entry.content.entry_type;
      return (
        entryType.type === 'tool_use' &&
        entryType.status.status === 'pending_approval'
      );
    });
  }, [entries]);

  // Send follow-up action
  const { isSendingFollowUp, followUpError, setFollowUpError, onSendFollowUp } =
    useFollowUpSend({
      sessionId,
      message: localMessage,
      conflictMarkdown: conflictResolutionInstructions,
      reviewMarkdown,
      clickedMarkdown,
      selectedVariant,
      clearComments,
      clearClickedElements,
      onAfterSendCleanup: () => {
        cancelDebouncedSave(); // Cancel any pending debounced save to avoid race condition
        setLocalMessage(''); // Clear local state immediately
        // Scratch deletion is handled by the backend when the queued message is consumed
      },
    });

  // Separate logic for when textarea should be disabled vs when send button should be disabled
  const canTypeFollowUp = useMemo(() => {
    if (!workspaceId || processes.length === 0 || isSendingFollowUp) {
      return false;
    }

    if (isRetryActive) return false; // disable typing while retry editor is active
    if (hasPendingApproval) return false; // disable typing during approval
    // Note: isQueued no longer blocks typing - editing auto-cancels the queue
    return true;
  }, [
    workspaceId,
    processes.length,
    isSendingFollowUp,
    isRetryActive,
    hasPendingApproval,
  ]);

  const canSendFollowUp = useMemo(() => {
    if (!canTypeFollowUp) {
      return false;
    }

    // Allow sending if conflict instructions, review comments, clicked elements, or message is present
    return Boolean(
      conflictResolutionInstructions ||
        reviewMarkdown ||
        clickedMarkdown ||
        localMessage.trim()
    );
  }, [
    canTypeFollowUp,
    conflictResolutionInstructions,
    reviewMarkdown,
    clickedMarkdown,
    localMessage,
  ]);
  const isEditable = !isRetryActive && !hasPendingApproval;

  const handleRunSetupScript = useCallback(async () => {
    if (!workspaceId || isAttemptRunning || !hasSetupScript) return;
    try {
      await attemptsApi.runSetupScript(workspaceId);
    } catch (error) {
      console.error('Failed to run setup script:', error);
    }
  }, [workspaceId, isAttemptRunning, hasSetupScript]);

  const handleRunCleanupScript = useCallback(async () => {
    if (!workspaceId || isAttemptRunning || !hasCleanupScript) return;
    try {
      await attemptsApi.runCleanupScript(workspaceId);
    } catch (error) {
      console.error('Failed to run cleanup script:', error);
    }
  }, [workspaceId, isAttemptRunning, hasCleanupScript]);

  // Handler to queue the current message for execution after agent finishes
  const handleQueueMessage = useCallback(async () => {
    if (
      !localMessage.trim() &&
      !conflictResolutionInstructions &&
      !reviewMarkdown &&
      !clickedMarkdown
    ) {
      return;
    }

    // Cancel any pending debounced save and save immediately before queueing
    // This prevents the race condition where the debounce fires after queueing
    cancelDebouncedSave();
    await saveToScratch(localMessage, selectedVariant);

    // Combine all the content that would be sent (same as follow-up send)
    const parts = [
      conflictResolutionInstructions,
      clickedMarkdown,
      reviewMarkdown,
      localMessage,
    ].filter(Boolean);
    const combinedMessage = parts.join('\n\n');
    await queueMessage(combinedMessage, selectedVariant);
  }, [
    localMessage,
    conflictResolutionInstructions,
    reviewMarkdown,
    clickedMarkdown,
    selectedVariant,
    queueMessage,
    cancelDebouncedSave,
    saveToScratch,
  ]);

  // Keyboard shortcut handler - send follow-up or queue depending on state
  const handleSubmitShortcut = useCallback(
    (e?: KeyboardEvent) => {
      e?.preventDefault();
      if (isAttemptRunning) {
        // When running, CMD+Enter queues the message (if not already queued)
        if (!isQueued) {
          handleQueueMessage();
        }
      } else {
        onSendFollowUp();
      }
    },
    [isAttemptRunning, isQueued, handleQueueMessage, onSendFollowUp]
  );

  // Attachment button - file input ref and handlers
  const fileInputRef = useRef<HTMLInputElement>(null);
  const handleAttachClick = useCallback(() => {
    fileInputRef.current?.click();
  }, []);
  const handleFileInputChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = Array.from(e.target.files || []).filter((f) =>
        f.type.startsWith('image/')
      );
      if (files.length > 0) {
        handlePasteFiles(files);
      }
      // Reset input so same file can be selected again
      e.target.value = '';
    },
    [handlePasteFiles]
  );

  // Handler for GitHub comments insertion
  const handleGitHubCommentClick = useCallback(async () => {
    if (!workspaceId) return;
    const repoId = getSelectedRepoId();
    if (!repoId) return;

    const result = await GitHubCommentsDialog.show({
      attemptId: workspaceId,
      repoId,
    });
    if (result.comments.length > 0) {
      // Build markdown for all selected comments
      const markdownBlocks = result.comments.map((comment) => {
        const payload: NormalizedComment = {
          id:
            comment.comment_type === 'general'
              ? comment.id
              : comment.id.toString(),
          comment_type: comment.comment_type,
          author: comment.author,
          body: comment.body,
          created_at: comment.created_at,
          url: comment.url,
          // Include review-specific fields when available
          ...(comment.comment_type === 'review' && {
            path: comment.path,
            line: comment.line != null ? Number(comment.line) : null,
            diff_hunk: comment.diff_hunk,
          }),
        };
        return '```gh-comment\n' + JSON.stringify(payload, null, 2) + '\n```';
      });

      const markdown = markdownBlocks.join('\n\n');
      appendToMessage(markdown);
    }
  }, [workspaceId, getSelectedRepoId, appendToMessage]);

  // Stable onChange handler for WYSIWYGEditor
  const handleEditorChange = useCallback(
    (value: string) => {
      updateMessage(value);
      if (followUpError) setFollowUpError(null);
    },
    [updateMessage, followUpError, setFollowUpError]
  );

  // Memoize placeholder to avoid re-renders
  const hasExtraContext = !!(reviewMarkdown || conflictResolutionInstructions);
  const editorPlaceholder = useMemo(
    () =>
      hasExtraContext
        ? '(Optional) Add additional instructions... Type @ to insert tags or search files.'
        : 'Continue working on this task attempt... Type @ to insert tags or search files.',
    [hasExtraContext]
  );

  // Register keyboard shortcuts
  useKeySubmitFollowUp(handleSubmitShortcut, {
    scope: Scope.FOLLOW_UP_READY,
    enableOnFormTags: ['textarea', 'TEXTAREA'],
    when: canSendFollowUp && isEditable,
  });

  // Enable FOLLOW_UP scope when textarea is focused AND editable
  useEffect(() => {
    if (isEditable && isTextareaFocused) {
      enableScope(Scope.FOLLOW_UP);
    } else {
      disableScope(Scope.FOLLOW_UP);
    }
    return () => {
      disableScope(Scope.FOLLOW_UP);
    };
  }, [isEditable, isTextareaFocused, enableScope, disableScope]);

  // Enable FOLLOW_UP_READY scope when ready to send
  useEffect(() => {
    const isReady = isTextareaFocused && isEditable;

    if (isReady) {
      enableScope(Scope.FOLLOW_UP_READY);
    } else {
      disableScope(Scope.FOLLOW_UP_READY);
    }
    return () => {
      disableScope(Scope.FOLLOW_UP_READY);
    };
  }, [isTextareaFocused, isEditable, enableScope, disableScope]);

  // When a process completes (e.g., agent resolved conflicts), refresh branch status promptly
  const prevRunningRef = useRef<boolean>(isAttemptRunning);
  useEffect(() => {
    if (prevRunningRef.current && !isAttemptRunning && workspaceId) {
      refetchBranchStatus();
      refetchAttemptBranch();
    }
    prevRunningRef.current = isAttemptRunning;
  }, [
    isAttemptRunning,
    workspaceId,
    refetchBranchStatus,
    refetchAttemptBranch,
  ]);

  if (!workspaceId) return null;

  if (isScratchLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 className="animate-spin h-6 w-6" />
      </div>
    );
  }

  return (
    <div
      className={cn(
        'grid h-full min-h-0 grid-rows-[minmax(0,1fr)_auto] overflow-hidden',
        isRetryActive && 'opacity-50'
      )}
    >
      {/* Scrollable content area */}
      <div className="overflow-y-auto min-h-0 p-4">
        <div className="space-y-2">
          {followUpError && (
            <Alert variant="destructive">
              <AlertCircle className="h-4 w-4" />
              <AlertDescription>{followUpError}</AlertDescription>
            </Alert>
          )}
          <div className="space-y-2">
            {/* Review comments preview */}
            {reviewMarkdown && (
              <div className="mb-4">
                <div className="text-sm whitespace-pre-wrap break-words rounded-md border bg-muted p-3">
                  {reviewMarkdown}
                </div>
              </div>
            )}

            {/* Conflict notice and actions (optional UI) */}
            {branchStatus && (
              <FollowUpConflictSection
                workspaceId={workspaceId}
                attemptBranch={attemptBranch}
                branchStatus={branchStatus}
                isEditable={isEditable}
                onResolve={onSendFollowUp}
                enableResolve={
                  canSendFollowUp && !isAttemptRunning && isEditable
                }
                enableAbort={canSendFollowUp && !isAttemptRunning}
                conflictResolutionInstructions={conflictResolutionInstructions}
              />
            )}

            {/* Clicked elements notice and actions */}
            <ClickedElementsBanner />

            {/* Queued message indicator */}
            {isQueued && queuedMessage && (
              <div className="flex items-center gap-2 text-sm text-muted-foreground bg-muted p-3 rounded-md border">
                <Clock className="h-4 w-4 flex-shrink-0" />
                <div className="font-medium">
                  {t(
                    'followUp.queuedMessage',
                    'Message queued - will execute when current run finishes'
                  )}
                </div>
              </div>
            )}

            <div
              className="flex flex-col gap-2"
              onFocus={() => setIsTextareaFocused(true)}
              onBlur={(e) => {
                // Only blur if focus is leaving the container entirely
                if (!e.currentTarget.contains(e.relatedTarget)) {
                  setIsTextareaFocused(false);
                }
              }}
            >
              <WYSIWYGEditor
                placeholder={editorPlaceholder}
                value={displayMessage}
                onChange={handleEditorChange}
                disabled={!isEditable}
                onPasteFiles={handlePasteFiles}
                projectId={projectId}
                taskAttemptId={workspaceId}
                taskId={task.id}
                onCmdEnter={handleSubmitShortcut}
                className="min-h-[40px]"
              />
            </div>
          </div>
        </div>
      </div>

      {/* Always-visible action bar */}
      <div className="p-4">
        <FollowUpActionBar
          currentProfile={currentProfile}
          selectedVariant={selectedVariant}
          onVariantChange={setSelectedVariant}
          isEditable={isEditable}
          fileInputRef={fileInputRef}
          onAttachClick={handleAttachClick}
          onFileInputChange={handleFileInputChange}
          onGitHubCommentClick={handleGitHubCommentClick}
          hasAnyScript={hasAnyScript}
          hasSetupScript={hasSetupScript}
          hasCleanupScript={hasCleanupScript}
          isAttemptRunning={isAttemptRunning}
          onRunSetupScript={handleRunSetupScript}
          onRunCleanupScript={handleRunCleanupScript}
          isQueued={isQueued}
          isQueueLoading={isQueueLoading}
          canQueueMessage={
            Boolean(
              localMessage.trim() ||
                conflictResolutionInstructions ||
                reviewMarkdown ||
                clickedMarkdown
            )
          }
          onQueueMessage={handleQueueMessage}
          onCancelQueue={cancelQueue}
          onStopExecution={stopExecution}
          onForceStopExecution={handleForceStop}
          isStopping={isStopping}
          reviewCommentCount={comments.length}
          onClearComments={clearComments}
          onSendFollowUp={onSendFollowUp}
          canSendFollowUp={canSendFollowUp}
          isSendingFollowUp={isSendingFollowUp}
          hasConflictInstructions={Boolean(conflictResolutionInstructions)}
          t={(key, fallback) => (fallback ? t(key, fallback) : t(key))}
        />
      </div>
    </div>
  );
}
