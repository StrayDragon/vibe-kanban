import { useCallback, useState } from 'react';
import { sessionsApi } from '@/lib/api';
import type {
  CreateFollowUpAttempt,
  ExecutionProcessPublic as ExecutionProcess,
} from 'shared/types';
import { useOptimisticExecutionProcessesStore } from '@/stores/useOptimisticExecutionProcessesStore';

type Args = {
  sessionId?: string;
  attemptId?: string;
  message: string;
  conflictMarkdown: string | null;
  reviewMarkdown: string;
  clickedMarkdown?: string;
  selectedVariant: string | null;
  clearComments: () => void;
  clearClickedElements?: () => void;
  onAfterSendCleanup: () => void;
  resyncExecutionProcesses?: (reason?: string) => void;
};

export function useFollowUpSend({
  sessionId,
  attemptId,
  message,
  conflictMarkdown,
  reviewMarkdown,
  clickedMarkdown,
  selectedVariant,
  clearComments,
  clearClickedElements,
  onAfterSendCleanup,
  resyncExecutionProcesses,
}: Args) {
  const [isSendingFollowUp, setIsSendingFollowUp] = useState(false);
  const [followUpError, setFollowUpError] = useState<string | null>(null);

  const onSendFollowUp = useCallback(
    async (messageOverride?: string) => {
      if (!sessionId) return;
      const override =
        typeof messageOverride === 'string' ? messageOverride : undefined;
      const extraMessage = (override ?? message).trim();
      const finalPrompt = [
        conflictMarkdown,
        clickedMarkdown?.trim(),
        reviewMarkdown?.trim(),
        extraMessage,
      ]
        .filter(Boolean)
        .join('\n\n');
      if (!finalPrompt) return;
      try {
        setIsSendingFollowUp(true);
        setFollowUpError(null);
        const body: CreateFollowUpAttempt = {
          prompt: finalPrompt,
          variant: selectedVariant,
          retry_process_id: null,
          force_when_dirty: null,
          perform_git_reset: null,
        };
        const process: ExecutionProcess = await sessionsApi.followUp(
          sessionId,
          body
        );
        if (attemptId) {
          useOptimisticExecutionProcessesStore
            .getState()
            .insert(attemptId, process);
        }
        resyncExecutionProcesses?.('follow-up-sent');
        clearComments();
        clearClickedElements?.();
        onAfterSendCleanup();
        // Don't call jumpToLogsTab() - preserves focus on the follow-up editor
      } catch (error: unknown) {
        const err = error as { message?: string };
        setFollowUpError(
          `Failed to start follow-up execution: ${err.message ?? 'Unknown error'}`
        );
      } finally {
        setIsSendingFollowUp(false);
      }
    },
    [
      sessionId,
      attemptId,
      message,
      conflictMarkdown,
      reviewMarkdown,
      clickedMarkdown,
      selectedVariant,
      clearComments,
      clearClickedElements,
      onAfterSendCleanup,
      resyncExecutionProcesses,
    ]
  );

  return {
    isSendingFollowUp,
    followUpError,
    setFollowUpError,
    onSendFollowUp,
  } as const;
}
