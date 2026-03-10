import { useTranslation } from 'react-i18next';
import NiceModal, { useModal } from '@ebay/nice-modal-react';

import { defineModal } from '@/lib/modals';
import { statusLabels } from '@/utils/statusLabels';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';

import type { TaskWithAttemptStatus } from 'shared/types';

export interface ArchivedTaskDetailsDialogProps {
  task: TaskWithAttemptStatus;
}

const ArchivedTaskDetailsDialogImpl =
  NiceModal.create<ArchivedTaskDetailsDialogProps>(({ task }) => {
    const { t } = useTranslation('tasks');
    const modal = useModal();

    const handleClose = () => {
      modal.resolve();
      modal.hide();
    };

    return (
      <Dialog
        open={modal.visible}
        onOpenChange={(open) => !open && handleClose()}
      >
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>
              {task.title || t('archives.taskDetails.untitled')}
            </DialogTitle>
            <DialogDescription>
              {t('archives.taskDetails.status')}: {statusLabels[task.status]}
            </DialogDescription>
          </DialogHeader>

          {task.description ? (
            <div className="whitespace-pre-wrap text-sm text-foreground break-words">
              {task.description}
            </div>
          ) : (
            <div className="text-sm text-muted-foreground">
              {t('archives.taskDetails.noDescription')}
            </div>
          )}

          <DialogFooter>
            <Button variant="outline" onClick={handleClose} autoFocus>
              {t('common:buttons.close')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  });

export const ArchivedTaskDetailsDialog = defineModal<
  ArchivedTaskDetailsDialogProps,
  void
>(ArchivedTaskDetailsDialogImpl);
