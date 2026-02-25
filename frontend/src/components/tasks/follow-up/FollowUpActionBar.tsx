import {
  AlertTriangle,
  Clock,
  Loader2,
  MessageSquare,
  Paperclip,
  Send,
  StopCircle,
  Terminal,
  X,
} from 'lucide-react';
import type { ChangeEvent, ReactNode, RefObject } from 'react';
import { Button } from '@/components/ui/button';
import type { ExecutorConfig } from 'shared/types';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { VariantSelector } from '@/components/tasks/VariantSelector';

interface FollowUpActionBarProps {
  currentProfile: ExecutorConfig | null;
  selectedVariant: string | null;
  onVariantChange: (variant: string | null) => void;
  isEditable: boolean;
  fileInputRef: RefObject<HTMLInputElement>;
  onAttachClick: () => void;
  onFileInputChange: (event: ChangeEvent<HTMLInputElement>) => void;
  onGitHubCommentClick: () => void;
  hasAnyScript: boolean;
  hasSetupScript: boolean;
  hasCleanupScript: boolean;
  isAttemptRunning: boolean;
  onRunSetupScript: () => void;
  onRunCleanupScript: () => void;
  isQueued: boolean;
  isQueueLoading: boolean;
  canQueueMessage: boolean;
  onQueueMessage: () => void;
  onCancelQueue: () => void;
  onStopExecution: () => void;
  onForceStopExecution: () => void;
  isStopping: boolean;
  reviewCommentCount: number;
  onClearComments: () => void;
  onSendFollowUp: () => void;
  canSendFollowUp: boolean;
  isSendingFollowUp: boolean;
  hasConflictInstructions: boolean;
  t: (key: string, defaultValue?: string) => string;
}

export function FollowUpActionBar({
  currentProfile,
  selectedVariant,
  onVariantChange,
  isEditable,
  fileInputRef,
  onAttachClick,
  onFileInputChange,
  onGitHubCommentClick,
  hasAnyScript,
  hasSetupScript,
  hasCleanupScript,
  isAttemptRunning,
  onRunSetupScript,
  onRunCleanupScript,
  isQueued,
  isQueueLoading,
  canQueueMessage,
  onQueueMessage,
  onCancelQueue,
  onStopExecution,
  onForceStopExecution,
  isStopping,
  reviewCommentCount,
  onClearComments,
  onSendFollowUp,
  canSendFollowUp,
  isSendingFollowUp,
  hasConflictInstructions,
  t,
}: FollowUpActionBarProps) {
  const scriptsTooltip: ReactNode = isAttemptRunning ? (
    <TooltipContent side="bottom">
      {t('followUp.scriptsDisabledWhileRunning')}
    </TooltipContent>
  ) : null;

  return (
    <div className="flex flex-row gap-2 items-center">
      <div className="flex-1 flex gap-2">
        <VariantSelector
          currentProfile={currentProfile}
          selectedVariant={selectedVariant}
          onChange={onVariantChange}
          disabled={!isEditable}
        />
      </div>

      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        multiple
        name="followUpAttachments"
        className="hidden"
        onChange={onFileInputChange}
      />

      <Button
        onClick={onAttachClick}
        disabled={!isEditable}
        size="sm"
        variant="outline"
        title="Attach image"
        aria-label="Attach image"
      >
        <Paperclip className="h-4 w-4" />
      </Button>

      <Button
        onClick={onGitHubCommentClick}
        disabled={!isEditable}
        size="sm"
        variant="outline"
        title="Insert GitHub comment"
        aria-label="Insert GitHub comment"
      >
        <MessageSquare className="h-4 w-4" />
      </Button>

      {hasAnyScript && (
        <DropdownMenu>
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <DropdownMenuTrigger asChild>
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={isAttemptRunning}
                    aria-label="Run scripts"
                  >
                    <Terminal className="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
              </TooltipTrigger>
              {scriptsTooltip}
            </Tooltip>
          </TooltipProvider>
          <DropdownMenuContent align="end">
            <DropdownMenuItem
              onClick={onRunSetupScript}
              disabled={!hasSetupScript}
              title={hasSetupScript ? undefined : t('followUp.noSetupScript')}
            >
              {t('followUp.runSetupScript')}
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={onRunCleanupScript}
              disabled={!hasCleanupScript}
              title={hasCleanupScript ? undefined : t('followUp.noCleanupScript')}
            >
              {t('followUp.runCleanupScript')}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      )}

      {isAttemptRunning ? (
        <div className="flex items-center gap-2">
          {isQueued ? (
            <Button
              onClick={onCancelQueue}
              disabled={isQueueLoading}
              size="sm"
              variant="outline"
            >
              {isQueueLoading ? (
                <Loader2 className="animate-spin h-4 w-4 mr-2" />
              ) : (
                <>
                  <X className="h-4 w-4 mr-2" />
                  {t('followUp.cancelQueue', 'Cancel Queue')}
                </>
              )}
            </Button>
          ) : (
            <Button
              onClick={onQueueMessage}
              disabled={isQueueLoading || !canQueueMessage}
              size="sm"
            >
              {isQueueLoading ? (
                <Loader2 className="animate-spin h-4 w-4 mr-2" />
              ) : (
                <>
                  <Clock className="h-4 w-4 mr-2" />
                  {t('followUp.queue', 'Queue')}
                </>
              )}
            </Button>
          )}
          <Button
            onClick={onStopExecution}
            disabled={isStopping}
            size="sm"
            variant="destructive"
          >
            {isStopping ? (
              <Loader2 className="animate-spin h-4 w-4 mr-2" />
            ) : (
              <>
                <StopCircle className="h-4 w-4 mr-2" />
                {t('followUp.stop')}
              </>
            )}
          </Button>
          <Button
            onClick={onForceStopExecution}
            disabled={isStopping}
            size="sm"
            variant="destructive"
          >
            <AlertTriangle className="h-4 w-4 mr-2" />
            {t('followUp.forceStop', 'Force stop')}
          </Button>
        </div>
      ) : (
        <div className="flex items-center gap-2">
          {reviewCommentCount > 0 && (
            <Button
              onClick={onClearComments}
              size="sm"
              variant="destructive"
              disabled={!isEditable}
            >
              {t('followUp.clearReviewComments')}
            </Button>
          )}
          <Button
            onClick={onSendFollowUp}
            disabled={!canSendFollowUp || !isEditable}
            size="sm"
          >
            {isSendingFollowUp ? (
              <Loader2 className="animate-spin h-4 w-4 mr-2" />
            ) : (
              <>
                <Send className="h-4 w-4 mr-2" />
                {hasConflictInstructions
                  ? t('followUp.resolveConflicts')
                  : t('followUp.send')}
              </>
            )}
          </Button>
        </div>
      )}
    </div>
  );
}
