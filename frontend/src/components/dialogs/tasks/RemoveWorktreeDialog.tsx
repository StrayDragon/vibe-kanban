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
import { useTaskAttempts, taskAttemptKeys } from '@/hooks/useTaskAttempts';
import { useExecutionProcesses } from '@/hooks/useExecutionProcesses';

export interface RemoveWorktreeDialogProps {
  task: TaskWithAttemptStatus;
  attempt?: Workspace | null;
}

const formatTimeAgo = (iso: string) => {
  const date = new Date(iso);
  const diffMs = Date.now() - date.getTime();
  const absSec = Math.round(Math.abs(diffMs) / 1000);

  const rtf =
    typeof Intl !== 'undefined' && typeof Intl.RelativeTimeFormat === 'function'
      ? new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' })
      : null;

  const to = (value: number, unit: Intl.RelativeTimeFormatUnit) =>
    rtf ? rtf.format(-value, unit) : `${value} ${unit} ago`;

  if (absSec < 60) return to(Math.round(absSec), 'second');
  const mins = Math.round(absSec / 60);
  if (mins < 60) return to(mins, 'minute');
  const hours = Math.round(mins / 60);
  if (hours < 24) return to(hours, 'hour');
  const days = Math.round(hours / 24);
  if (days < 30) return to(days, 'day');
  const months = Math.round(days / 30);
  if (months < 12) return to(months, 'month');
  const years = Math.round(months / 12);
  return to(years, 'year');
};

const RemoveWorktreeDialogImpl =
  NiceModal.create<RemoveWorktreeDialogProps>(({ task, attempt }) => {
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
            queryKey: ['taskAttempt', selectedAttempt.id],
          }),
          queryClient.invalidateQueries({
            queryKey: ['taskAttemptWithSession', selectedAttempt.id],
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
  });

export const RemoveWorktreeDialog = defineModal<
  RemoveWorktreeDialogProps,
  void
>(RemoveWorktreeDialogImpl);
