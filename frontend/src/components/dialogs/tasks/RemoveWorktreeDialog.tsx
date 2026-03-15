import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { useQueryClient } from '@tanstack/react-query';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import type { TaskWithAttemptStatus, Workspace } from 'shared/types';
import { defineModal } from '@/lib/modals';
import { attemptsApi } from '@/lib/api';
import {
  useTaskAttempts,
  taskAttemptKeys,
} from '@/hooks/task-attempts/useTaskAttempts';
import { useExecutionProcesses } from '@/hooks/execution-processes/useExecutionProcesses';
import { formatTimeAgo } from '@/lib/formatTimeAgo';

export interface RemoveWorktreeDialogProps {
  task: TaskWithAttemptStatus;
  attempt?: Workspace | null;
}

const RemoveWorktreeDialogImpl = NiceModal.create<RemoveWorktreeDialogProps>(
  ({ task, attempt }) => {
    const modal = useModal();
    const { t } = useTranslation('tasks');
    const queryClient = useQueryClient();
    const [selectedAttemptId, setSelectedAttemptId] = useState<string | null>(
      attempt?.id ?? null
    );
    const [isRemoving, setIsRemoving] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const { data: attempts = [], isLoading: isAttemptsLoading } =
      useTaskAttempts(task.id, {
        enabled: Boolean(!attempt),
        refetchInterval: false,
      });

    const eligibleAttempts = useMemo(() => {
      const list = attempt
        ? attempt.container_ref
          ? [attempt]
          : []
        : attempts.filter((item) => item.container_ref);
      return [...list].sort(
        (a, b) =>
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
    }, [attempt, attempts]);

    useEffect(() => {
      if (attempt) {
        setSelectedAttemptId(attempt.id);
        return;
      }
      if (!eligibleAttempts.length) {
        setSelectedAttemptId(null);
        return;
      }
      if (
        !selectedAttemptId ||
        !eligibleAttempts.some((item) => item.id === selectedAttemptId)
      ) {
        setSelectedAttemptId(eligibleAttempts[0].id);
      }
    }, [attempt, eligibleAttempts, selectedAttemptId]);

    const selectedAttempt = useMemo(() => {
      if (!eligibleAttempts.length) return null;
      if (!selectedAttemptId) return eligibleAttempts[0] ?? null;
      return (
        eligibleAttempts.find((item) => item.id === selectedAttemptId) ??
        eligibleAttempts[0] ??
        null
      );
    }, [eligibleAttempts, selectedAttemptId]);

    const { executionProcesses } = useExecutionProcesses(selectedAttempt?.id, {
      showSoftDeleted: true,
    });
    const hasRunningProcesses = executionProcesses.some(
      (process) => process.status === 'running'
    );

    const showAttemptSelect =
      !attempt && eligibleAttempts.length > 1 && !isAttemptsLoading;

    const handleConfirm = async () => {
      if (!selectedAttempt || hasRunningProcesses) return;
      setIsRemoving(true);
      setError(null);

      try {
        await attemptsApi.removeWorktree(selectedAttempt.id);
        await Promise.all([
          queryClient.invalidateQueries({ queryKey: taskAttemptKeys.all }),
          queryClient.invalidateQueries({
            queryKey: taskAttemptKeys.attempt(selectedAttempt.id),
          }),
          queryClient.invalidateQueries({
            queryKey: taskAttemptKeys.attemptWithSession(selectedAttempt.id),
          }),
          queryClient.invalidateQueries({
            queryKey: taskAttemptKeys.byTaskWithSessions(task.id),
          }),
        ]);
        modal.resolve();
        modal.hide();
      } catch (err: unknown) {
        const message =
          err instanceof Error ? err.message : t('removeWorktreeDialog.error');
        setError(message);
      } finally {
        setIsRemoving(false);
      }
    };

    const handleCancel = () => {
      modal.reject();
      modal.hide();
    };

    const attemptLabel = selectedAttempt
      ? `${selectedAttempt.branch || t('removeWorktreeDialog.unknownBranch')} - ${formatTimeAgo(
          selectedAttempt.created_at
        )}`
      : null;

    const confirmDisabled =
      isRemoving ||
      !selectedAttempt ||
      hasRunningProcesses ||
      isAttemptsLoading;

    return (
      <Dialog
        open={modal.visible}
        onOpenChange={(open) => !open && handleCancel()}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t('removeWorktreeDialog.title')}</DialogTitle>
            <DialogDescription>
              {t('removeWorktreeDialog.description')}
            </DialogDescription>
          </DialogHeader>

          <Alert variant="destructive" className="mb-4">
            <AlertDescription>
              {t('removeWorktreeDialog.warning')}
            </AlertDescription>
          </Alert>

          {isAttemptsLoading && !attempt && (
            <div className="text-sm text-muted-foreground">
              {t('removeWorktreeDialog.loadingAttempts')}
            </div>
          )}

          {!isAttemptsLoading && !attempt && !eligibleAttempts.length && (
            <Alert variant="destructive" className="mb-4">
              <AlertDescription>
                {t('removeWorktreeDialog.noEligible')}
              </AlertDescription>
            </Alert>
          )}

          {showAttemptSelect && (
            <div className="mb-4 space-y-2">
              <Label htmlFor="remove-worktree-attempt">
                {t('removeWorktreeDialog.attemptLabel')}
              </Label>
              <Select
                value={selectedAttemptId ?? undefined}
                onValueChange={(value) => setSelectedAttemptId(value)}
              >
                <SelectTrigger id="remove-worktree-attempt">
                  <SelectValue
                    placeholder={t('removeWorktreeDialog.selectAttempt')}
                  />
                </SelectTrigger>
                <SelectContent>
                  {eligibleAttempts.map((item) => (
                    <SelectItem key={item.id} value={item.id}>
                      {item.branch || t('removeWorktreeDialog.unknownBranch')} -{' '}
                      {formatTimeAgo(item.created_at)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          )}

          {!showAttemptSelect && attemptLabel && (
            <div className="mb-4 text-sm text-muted-foreground">
              {t('removeWorktreeDialog.attemptLabel')}: {attemptLabel}
            </div>
          )}

          {hasRunningProcesses && (
            <Alert variant="destructive" className="mb-4">
              <AlertDescription>
                {t('removeWorktreeDialog.runningWarning')}
              </AlertDescription>
            </Alert>
          )}

          {error && (
            <Alert variant="destructive" className="mb-4">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}

          <DialogFooter>
            <Button
              variant="outline"
              onClick={handleCancel}
              disabled={isRemoving}
            >
              {t('common:buttons.cancel')}
            </Button>
            <Button
              variant="destructive"
              onClick={handleConfirm}
              disabled={confirmDisabled}
            >
              {isRemoving
                ? t('removeWorktreeDialog.removing')
                : t('removeWorktreeDialog.confirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const RemoveWorktreeDialog = defineModal<
  RemoveWorktreeDialogProps,
  void
>(RemoveWorktreeDialogImpl);
