import { useState } from 'react';
import { BaseAgentCapability } from 'shared/types';
import type { WorkspaceWithSession } from '@/types/attempt';
import { useUserSystem } from '@/components/ConfigProvider';
import { useRetryUi } from '@/contexts/RetryUiContext';
import { useAttemptExecution } from '@/hooks/task-attempts/useAttemptExecution';
import { RetryEditorInline } from './RetryEditorInline';
import TranslatableContent from './TranslatableContent';

const UserMessage = ({
  content,
  executionProcessId,
  taskAttempt,
  taskId,
  entryKey,
}: {
  content: string;
  executionProcessId?: string;
  taskAttempt?: WorkspaceWithSession;
  taskId?: string;
  entryKey: string;
}) => {
  const [isEditing, setIsEditing] = useState(false);
  const { capabilities } = useUserSystem();
  const { activeRetryProcessId, setActiveRetryProcessId, isProcessGreyed } =
    useRetryUi();
  const { isAttemptRunning } = useAttemptExecution(taskAttempt?.id);

  const canFork = !!(
    taskAttempt?.session?.executor &&
    capabilities?.[taskAttempt.session.executor]?.includes(
      BaseAgentCapability.SESSION_FORK
    )
  );

  const startRetry = () => {
    if (!executionProcessId || !taskAttempt) return;
    setIsEditing(true);
    setActiveRetryProcessId(executionProcessId);
  };

  const onCancelled = () => {
    setIsEditing(false);
    setActiveRetryProcessId(null);
  };

  const showRetryEditor =
    !!executionProcessId &&
    isEditing &&
    activeRetryProcessId === executionProcessId;
  const greyed =
    !!executionProcessId &&
    isProcessGreyed(executionProcessId) &&
    !showRetryEditor;

  // Only show retry button when allowed (has process, can fork, not running)
  const canRetry = executionProcessId && canFork && !isAttemptRunning;

  return (
    <div className={`py-2 ${greyed ? 'opacity-50 pointer-events-none' : ''}`}>
      <div className="bg-background px-4 py-2 text-sm">
        <div className="py-3">
          {showRetryEditor && taskAttempt ? (
            <RetryEditorInline
              attempt={taskAttempt}
              executionProcessId={executionProcessId}
              initialContent={content}
              onCancelled={onCancelled}
              taskId={taskId}
            />
          ) : (
            <TranslatableContent
              entryKey={entryKey}
              content={content}
              markdown
              contentClassName="whitespace-pre-wrap break-words flex flex-col gap-1 font-light"
              taskAttemptId={taskAttempt?.id}
              taskId={taskId}
              canTranslate
              onEdit={canRetry ? startRetry : undefined}
            />
          )}
        </div>
      </div>
    </div>
  );
};

export default UserMessage;
