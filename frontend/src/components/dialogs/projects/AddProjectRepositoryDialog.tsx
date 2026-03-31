import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { useQueryClient } from '@tanstack/react-query';
import { defineModal, getErrorMessage } from '@/lib/modals';
import {
  projectsApi,
  type AddProjectRepositoryByPathResponse,
} from '@/lib/api';
import { projectKeys } from '@/query-keys/projectKeys';
import { toast } from '@/components/ui/toast';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Alert } from '@/components/ui/alert';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { FolderPickerDialog } from '@/components/dialogs/shared/FolderPickerDialog';

import { FolderOpen, Loader2 } from 'lucide-react';

export interface AddProjectRepositoryDialogProps {
  projectId: string;
  projectName?: string;
}

export type AddProjectRepositoryDialogResult =
  | AddProjectRepositoryByPathResponse
  | 'canceled';

function basename(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, '');
  const parts = trimmed.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? '';
}

const AddProjectRepositoryDialogImpl =
  NiceModal.create<AddProjectRepositoryDialogProps>(
    ({ projectId, projectName }) => {
      const { t } = useTranslation(['projects', 'common']);
      const modal = useModal();
      const queryClient = useQueryClient();

      const [repoPath, setRepoPath] = useState('');
      const [displayName, setDisplayName] = useState('');
      const [isSubmitting, setIsSubmitting] = useState(false);
      const [error, setError] = useState<string | null>(null);

      const normalizedProjectName = useMemo(
        () => projectName?.trim() || projectId.slice(0, 8),
        [projectId, projectName]
      );

      useEffect(() => {
        if (!modal.visible) return;
        setRepoPath('');
        setDisplayName('');
        setIsSubmitting(false);
        setError(null);
      }, [modal.visible]);

      const handleCancel = useCallback(() => {
        if (isSubmitting) return;
        modal.resolve('canceled' as AddProjectRepositoryDialogResult);
        modal.hide();
      }, [isSubmitting, modal]);

      const handleBrowse = useCallback(async () => {
        if (isSubmitting) return;
        setError(null);

        const picked = await FolderPickerDialog.show({
          title: t('addRepositoryDialog.picker.title'),
          description: t('addRepositoryDialog.picker.description', {
            projectName: normalizedProjectName,
          }),
          value: repoPath,
        });
        if (!picked) return;

        setRepoPath(picked);
        if (!displayName.trim()) {
          setDisplayName(basename(picked));
        }
      }, [
        displayName,
        isSubmitting,
        normalizedProjectName,
        repoPath,
        t,
      ]);

      const handleSubmit = useCallback(async () => {
        if (isSubmitting) return;
        setError(null);

        const trimmedPath = repoPath.trim();
        if (!trimmedPath) {
          setError(t('addRepositoryDialog.errors.pathRequired'));
          return;
        }

        setIsSubmitting(true);
        try {
          const result = await projectsApi.addRepositoryByPath(projectId, {
            path: trimmedPath,
            display_name: displayName.trim() || undefined,
            reload: true,
          });

          queryClient.invalidateQueries({
            queryKey: projectKeys.repositories(projectId),
          });

          toast({
            title: result.was_added
              ? t('addRepositoryDialog.toast.added')
              : t('addRepositoryDialog.toast.exists'),
            description: result.display_name,
          });

          modal.resolve(result as AddProjectRepositoryDialogResult);
          modal.hide();
        } catch (err) {
          setError(getErrorMessage(err));
        } finally {
          setIsSubmitting(false);
        }
      }, [
        displayName,
        isSubmitting,
        modal,
        projectId,
        queryClient,
        repoPath,
        t,
      ]);

      return (
        <Dialog open={modal.visible} onOpenChange={(open) => !open && handleCancel()}>
          <DialogContent className="sm:max-w-[520px]">
            <DialogHeader>
              <DialogTitle>{t('addRepositoryDialog.title')}</DialogTitle>
              <DialogDescription>
                {t('addRepositoryDialog.description', {
                  projectName: normalizedProjectName,
                })}
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="add-repo-path">
                  {t('addRepositoryDialog.path.label')}
                </Label>
                <div className="flex items-center gap-2">
                  <Input
                    id="add-repo-path"
                    value={repoPath}
                    onChange={(e) => setRepoPath(e.target.value)}
                    placeholder={t('addRepositoryDialog.path.placeholder')}
                    disabled={isSubmitting}
                    className="flex-1"
                  />
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => void handleBrowse()}
                    disabled={isSubmitting}
                  >
                    <FolderOpen className="mr-2 h-4 w-4" />
                    {t('addRepositoryDialog.browse')}
                  </Button>
                </div>
              </div>

              <div className="space-y-2">
                <Label htmlFor="add-repo-display-name">
                  {t('addRepositoryDialog.displayName.label')}
                </Label>
                <Input
                  id="add-repo-display-name"
                  value={displayName}
                  onChange={(e) => setDisplayName(e.target.value)}
                  placeholder={t('addRepositoryDialog.displayName.placeholder')}
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
              >
                {t('common:buttons.cancel')}
              </Button>
              <Button onClick={() => void handleSubmit()} disabled={isSubmitting}>
                {isSubmitting ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    {t('addRepositoryDialog.submitting')}
                  </>
                ) : (
                  t('addRepositoryDialog.submit')
                )}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      );
    }
  );

export const AddProjectRepositoryDialog = defineModal<
  AddProjectRepositoryDialogProps,
  AddProjectRepositoryDialogResult
>(AddProjectRepositoryDialogImpl);

