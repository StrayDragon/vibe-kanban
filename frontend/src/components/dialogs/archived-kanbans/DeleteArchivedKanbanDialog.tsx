import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import NiceModal, { useModal } from '@ebay/nice-modal-react';

import { defineModal } from '@/lib/modals';
import { archivedKanbansApi } from '@/lib/api';

import { Alert } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';

export interface DeleteArchivedKanbanDialogProps {
  archiveId: string;
  archiveTitle: string;
  tasksCount?: number;
}

export interface DeleteArchivedKanbanDialogResult {
  deletedTaskCount: number;
}

const DeleteArchivedKanbanDialogImpl =
  NiceModal.create<DeleteArchivedKanbanDialogProps>(
    ({ archiveId, archiveTitle, tasksCount }) => {
      const { t } = useTranslation('tasks');
      const modal = useModal();

      const [confirmation, setConfirmation] = useState('');
      const [isSubmitting, setIsSubmitting] = useState(false);
      const [error, setError] = useState<string | null>(null);

      const matches = useMemo(
        () => confirmation.trim() === archiveTitle.trim(),
        [confirmation, archiveTitle]
      );

      const handleCancel = () => {
        modal.reject();
        modal.hide();
      };

      const handleConfirm = async () => {
        setError(null);
        setIsSubmitting(true);
        try {
          const response = await archivedKanbansApi.delete(archiveId);
          modal.resolve({
            deletedTaskCount: Number(response.deleted_task_count),
          });
          modal.hide();
        } catch (err: unknown) {
          const message =
            err instanceof Error ? err.message : t('archives.deleteDialog.error');
          setError(message);
        } finally {
          setIsSubmitting(false);
        }
      };

      return (
        <Dialog open={modal.visible} onOpenChange={(open) => !open && handleCancel()}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{t('archives.deleteDialog.title')}</DialogTitle>
              <DialogDescription>
                {t('archives.deleteDialog.description')}
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-4">
              <Alert variant="destructive">
                {t('archives.deleteDialog.warning', {
                  count: typeof tasksCount === 'number' ? tasksCount : undefined,
                })}
              </Alert>

              <div className="space-y-2">
                <div className="text-sm text-muted-foreground">
                  {t('archives.deleteDialog.typeToConfirm', { title: archiveTitle })}
                </div>
                <Input
                  value={confirmation}
                  onChange={(e) => setConfirmation(e.target.value)}
                  placeholder={archiveTitle}
                  disabled={isSubmitting}
                />
              </div>

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
              <Button
                variant="destructive"
                onClick={handleConfirm}
                disabled={!matches || isSubmitting}
              >
                {isSubmitting
                  ? t('archives.deleteDialog.submitting')
                  : t('archives.deleteDialog.confirm')}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      );
    }
  );

export const DeleteArchivedKanbanDialog = defineModal<
  DeleteArchivedKanbanDialogProps,
  DeleteArchivedKanbanDialogResult
>(DeleteArchivedKanbanDialogImpl);
