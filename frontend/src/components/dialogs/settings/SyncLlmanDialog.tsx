import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { profilesApi } from '@/lib/api';
import { defineModal } from '@/lib/modals';

export type SyncLlmanDialogProps = Record<string, never>;

export type SyncLlmanDialogResult = {
  action: 'sync' | 'canceled';
  path: string;
};

const SyncLlmanDialogImpl = NiceModal.create<SyncLlmanDialogProps>(() => {
  const modal = useModal();
  const { t } = useTranslation(['settings', 'common']);
  const [path, setPath] = useState('');
  const userEditedRef = useRef(false);

  useEffect(() => {
    if (!modal.visible) return;
    let active = true;
    userEditedRef.current = false;
    setPath('');

    const resolvePath = async () => {
      try {
        const resolved = await profilesApi.resolveLlmanPath();
        if (!active || userEditedRef.current) return;
        setPath(resolved.path ?? '');
      } catch (error) {
        console.error('Failed to resolve LLMAN path:', error);
      }
    };

    void resolvePath();

    return () => {
      active = false;
    };
  }, [modal.visible]);

  const handleSync = () => {
    modal.resolve({ action: 'sync', path } as SyncLlmanDialogResult);
    modal.hide();
  };

  const handleCancel = () => {
    modal.resolve({ action: 'canceled' } as SyncLlmanDialogResult);
    modal.hide();
  };

  const handleOpenChange = (open: boolean) => {
    if (!open) {
      handleCancel();
    }
  };

  return (
    <Dialog open={modal.visible} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t('settings.agents.llman.syncTitle')}</DialogTitle>
          <DialogDescription>
            {t('settings.agents.llman.syncDescription')}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="llman-sync-path">
              {t('settings.agents.llman.pathLabel')}
            </Label>
            <Input
              id="llman-sync-path"
              value={path}
              onChange={(event) => {
                userEditedRef.current = true;
                setPath(event.target.value);
              }}
              placeholder={t('settings.agents.llman.pathPlaceholder')}
              autoFocus
            />
            <p className="text-sm text-muted-foreground">
              {t('settings.agents.llman.pathHelper')}
            </p>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={handleCancel}>
            {t('common:buttons.cancel')}
          </Button>
          <Button onClick={handleSync}>
            {t('settings.agents.llman.syncButton')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});

export const SyncLlmanDialog = defineModal<
  SyncLlmanDialogProps,
  SyncLlmanDialogResult
>(SyncLlmanDialogImpl);
