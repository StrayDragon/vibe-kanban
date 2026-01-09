import { useCallback, useMemo, useState } from 'react';
import { imagesApi } from '@/lib/api';
import { useLatest } from '@/hooks/useLatest';
import type { QueuedMessage } from 'shared/types';

interface UseFollowUpEditorOptions {
  workspaceId?: string;
  isQueued: boolean;
  queuedMessage: QueuedMessage | null;
  cancelQueue: () => void;
  setFollowUpMessage: (value: string) => void;
}

export function useFollowUpEditor({
  workspaceId,
  isQueued,
  queuedMessage,
  cancelQueue,
  setFollowUpMessage,
}: UseFollowUpEditorOptions) {
  const [localMessage, setLocalMessage] = useState('');
  const queueStateRef = useLatest({ isQueued, queuedMessage, cancelQueue });

  const displayMessage = useMemo(() => {
    if (isQueued && queuedMessage) return queuedMessage.data.message;
    return localMessage;
  }, [isQueued, queuedMessage, localMessage]);

  const updateMessage = useCallback(
    (value: string) => {
      const queueState = queueStateRef.current;
      if (queueState.isQueued) {
        queueState.cancelQueue();
      }
      setLocalMessage(value);
      setFollowUpMessage(value);
    },
    [queueStateRef, setFollowUpMessage]
  );

  const appendToMessage = useCallback(
    (snippet: string) => {
      const queueState = queueStateRef.current;
      const isQueueActive = queueState.isQueued;
      const queuedBase = queueState.queuedMessage?.data.message;
      if (isQueueActive) {
        queueState.cancelQueue();
      }
      setLocalMessage((prev) => {
        const base = isQueueActive ? queuedBase ?? prev : prev;
        const next = base ? `${base}\n\n${snippet}` : snippet;
        setFollowUpMessage(next);
        return next;
      });
    },
    [queueStateRef, setFollowUpMessage]
  );

  const handlePasteFiles = useCallback(
    async (files: File[]) => {
      if (!workspaceId) return;

      for (const file of files) {
        try {
          const response = await imagesApi.uploadForAttempt(workspaceId, file);
          const imageMarkdown = `![${response.original_name}](${response.file_path})`;
          appendToMessage(imageMarkdown);
        } catch (error) {
          console.error('Failed to upload image:', error);
        }
      }
    },
    [workspaceId, appendToMessage]
  );

  return {
    localMessage,
    setLocalMessage,
    displayMessage,
    updateMessage,
    appendToMessage,
    handlePasteFiles,
  };
}
