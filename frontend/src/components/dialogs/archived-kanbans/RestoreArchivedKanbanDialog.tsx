import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import NiceModal, { useModal } from '@ebay/nice-modal-react';

import { defineModal } from '@/lib/modals';
import { archivedKanbansApi } from '@/lib/api';
import { statusLabels } from '@/utils/statusLabels';

import { Alert } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';

import type { TaskStatus } from 'shared/types';

export interface RestoreArchivedKanbanDialogProps {
  archiveId: string;
}

export interface RestoreArchivedKanbanDialogResult {
  restoredTaskCount: number;
}

const STATUS_OPTIONS: TaskStatus[] = [
  'todo',
  'inprogress',
  'inreview',
  'done',
  'cancelled',
];

const DEFAULT_STATUSES: TaskStatus[] = ['done', 'cancelled'];

const RestoreArchivedKanbanDialogImpl =
  NiceModal.create<RestoreArchivedKanbanDialogProps>(({ archiveId }) => {
    const { t } = useTranslation('tasks');
    const modal = useModal();

    const [restoreAll, setRestoreAll] = useState(true);
    const [selected, setSelected] = useState<Set<TaskStatus>>(
      () => new Set(DEFAULT_STATUSES)
    );
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const selectedStatuses = useMemo(
      () => STATUS_OPTIONS.filter((status) => selected.has(status)),
      [selected]
    );

    const handleCancel = () => {
      modal.reject();
      modal.hide();
    };

    const handleConfirm = async () => {
      setError(null);

      if (!restoreAll && selectedStatuses.length === 0) {
        setError(t('archives.restoreDialog.statusRequired'));
        return;
      }

      setIsSubmitting(true);
      try {
        const response = await archivedKanbansApi.restore(archiveId, {
          restore_all: restoreAll,
          statuses: restoreAll ? null : selectedStatuses,
        });

        modal.resolve({ restoredTaskCount: Number(response.restored_task_count) });
        modal.hide();
      } catch (err: unknown) {
        const message =
          err instanceof Error ? err.message : t('archives.restoreDialog.error');
        setError(message);
      } finally {
        setIsSubmitting(false);
      }
    };

    return (
      <Dialog open={modal.visible} onOpenChange={(open) => !open && handleCancel()}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t('archives.restoreDialog.title')}</DialogTitle>
            <DialogDescription>
              {t('archives.restoreDialog.description')}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <Alert>{t('archives.restoreDialog.warning')}</Alert>

            <label className="flex items-center gap-2 text-sm select-none">
              <Checkbox
                checked={restoreAll}
                onCheckedChange={(checked) => setRestoreAll(checked)}
                disabled={isSubmitting}
              />
              <span>{t('archives.restoreDialog.restoreAll')}</span>
            </label>

            {!restoreAll && (
              <div className="space-y-2">
                <div className="text-sm font-medium">
                  {t('archives.restoreDialog.statusesLabel')}
                </div>
                <div className="grid grid-cols-2 gap-2">
                  {STATUS_OPTIONS.map((status) => {
                    const checked = selected.has(status);
                    return (
                      <label
                        key={status}
                        className="flex items-center gap-2 text-sm text-muted-foreground cursor-pointer select-none"
                      >
                        <Checkbox
                          checked={checked}
                          onCheckedChange={(next) => {
                            setSelected((prev) => {
                              const updated = new Set(prev);
                              if (next) {
                                updated.add(status);
                              } else {
                                updated.delete(status);
                              }
                              return updated;
                            });
                          }}
                          disabled={isSubmitting}
                        />
                        <span className="text-foreground">
                          {statusLabels[status]}
                        </span>
                      </label>
                    );
                  })}
                </div>
              </div>
            )}

            {error && <Alert variant="destructive">{error}</Alert>}
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={handleCancel}
              disabled={isSubmitting}
              autoFocus
            >
              {t('common:buttons.cancel')}
            </Button>
            <Button onClick={handleConfirm} disabled={isSubmitting}>
              {isSubmitting
                ? t('archives.restoreDialog.submitting')
                : t('archives.restoreDialog.confirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  });

export const RestoreArchivedKanbanDialog = defineModal<
  RestoreArchivedKanbanDialogProps,
  RestoreArchivedKanbanDialogResult
>(RestoreArchivedKanbanDialogImpl);
