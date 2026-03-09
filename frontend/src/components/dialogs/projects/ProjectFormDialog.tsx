import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import NiceModal, { useModal } from '@ebay/nice-modal-react';

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
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { RepoPickerDialog } from '@/components/dialogs/shared/RepoPickerDialog';
import { useProjectMutations } from '@/hooks/projects/useProjectMutations';
import { defineModal } from '@/lib/modals';
import { uiIds } from '@/lib/uiIds';
import { getUnsafeRepoPathWarnings } from '@/utils/repoPathSafety';

import { AlertCircle, ArrowLeft, FolderGit, Loader2 } from 'lucide-react';
import type { CreateProject, Project, Repo } from 'shared/types';

export interface ProjectFormDialogProps {}

export type ProjectFormDialogResult = Project | 'canceled';

type Step = 'repo' | 'details';

const ProjectFormDialogImpl = NiceModal.create<ProjectFormDialogProps>(() => {
  const modal = useModal();
  const { t } = useTranslation(['projects', 'common']);

  const [step, setStep] = useState<Step>('repo');
  const [selectedRepo, setSelectedRepo] = useState<Repo | null>(null);
  const [projectName, setProjectName] = useState('');
  const [unsafeAck, setUnsafeAck] = useState(false);
  const nameInputRef = useRef<HTMLInputElement | null>(null);

  const { createProject } = useProjectMutations({
    onCreateSuccess: (project: Project) => {
      modal.resolve(project as ProjectFormDialogResult);
      modal.hide();
    },
  });
  const resetCreateProject = createProject.reset;

  useEffect(() => {
    if (!modal.visible) return;
    setStep('repo');
    setSelectedRepo(null);
    setProjectName('');
    setUnsafeAck(false);
    resetCreateProject();
  }, [modal.visible, resetCreateProject]);

  const unsafeWarnings = useMemo(() => {
    if (!selectedRepo) return [];
    return getUnsafeRepoPathWarnings(selectedRepo.path);
  }, [selectedRepo]);
  const requiresUnsafeAck = unsafeWarnings.length > 0;

  useEffect(() => {
    if (!modal.visible) return;
    if (step !== 'details') return;
    requestAnimationFrame(() => nameInputRef.current?.focus());
  }, [modal.visible, step]);

  const handleCancel = useCallback(() => {
    if (createProject.isPending) return;
    modal.resolve('canceled' as ProjectFormDialogResult);
    modal.hide();
  }, [createProject.isPending, modal]);

  const handlePickRepo = useCallback(async () => {
    if (createProject.isPending) return;
    createProject.reset();

    const repo = await RepoPickerDialog.show({
      title: t('projects:createWizard.repoPicker.title'),
      description: t('projects:createWizard.repoPicker.description'),
    });

    if (!repo) return;

    setSelectedRepo(repo);
    setProjectName(repo.display_name || repo.name);
    setUnsafeAck(false);
    setStep('details');
  }, [createProject, t]);

  const handleBack = useCallback(() => {
    if (createProject.isPending) return;
    createProject.reset();
    setStep('repo');
  }, [createProject]);

  const handleContinue = useCallback(() => {
    if (createProject.isPending) return;
    if (!selectedRepo) return;
    setStep('details');
  }, [createProject.isPending, selectedRepo]);

  const handleCreate = useCallback(() => {
    if (createProject.isPending) return;
    if (!selectedRepo) return;

    const trimmedName = projectName.trim();
    if (!trimmedName) return;
    if (requiresUnsafeAck && !unsafeAck) return;

    const createData: CreateProject = {
      name: trimmedName,
      repositories: [
        {
          display_name: selectedRepo.display_name || selectedRepo.name,
          git_repo_path: selectedRepo.path,
        },
      ],
    };

    createProject.mutate(createData);
  }, [
    createProject,
    projectName,
    requiresUnsafeAck,
    selectedRepo,
    unsafeAck,
  ]);

  const canCreate = Boolean(
    selectedRepo &&
      projectName.trim().length > 0 &&
      (!requiresUnsafeAck || unsafeAck) &&
      !createProject.isPending
  );

  const repoSummary = selectedRepo ? (
    <div className="rounded-lg border bg-card px-4 py-3">
      <div className="flex items-start gap-3">
        <FolderGit className="mt-0.5 h-5 w-5 flex-shrink-0 text-muted-foreground" />
        <div className="min-w-0 flex-1">
          <div className="font-medium text-foreground">
            {selectedRepo.display_name || selectedRepo.name}
          </div>
          <div className="mt-1 truncate text-xs font-mono text-muted-foreground">
            {selectedRepo.path}
          </div>
        </div>
        {step === 'details' ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={handlePickRepo}
            disabled={createProject.isPending}
          >
            {t('common:buttons.change')}
          </Button>
        ) : null}
      </div>
    </div>
  ) : null;

  const handleOpenChange = (open: boolean) => {
    if (!open) {
      handleCancel();
    }
  };

  return (
    <Dialog
      open={modal.visible}
      onOpenChange={handleOpenChange}
      uncloseable={createProject.isPending}
    >
      <DialogContent className="sm:max-w-[520px]" data-testid="project-wizard">
        <DialogHeader>
          <DialogTitle>{t('projects:createWizard.title')}</DialogTitle>
          <DialogDescription>
            {step === 'repo'
              ? t('projects:createWizard.repoStep.description')
              : t('projects:createWizard.detailsStep.description')}
          </DialogDescription>
        </DialogHeader>

        {step === 'repo' ? (
          <div className="space-y-4">
            {repoSummary}
            <Button
              type="button"
              variant="outline"
              className="w-full justify-start gap-3"
              onClick={handlePickRepo}
              disabled={createProject.isPending}
              id={uiIds.projectWizardPickRepo}
              data-testid={uiIds.projectWizardPickRepo}
            >
              <FolderGit className="h-4 w-4" />
              {selectedRepo
                ? t('projects:createWizard.repoStep.changeRepo')
                : t('projects:createWizard.repoStep.pickRepo')}
            </Button>
          </div>
        ) : (
          <div className="space-y-4">
            {repoSummary}

            <div className="space-y-2">
              <Label htmlFor={uiIds.projectWizardName}>
                {t('projects:createWizard.detailsStep.projectName.label')}
              </Label>
              <Input
                id={uiIds.projectWizardName}
                data-testid={uiIds.projectWizardName}
                ref={nameInputRef}
                value={projectName}
                onChange={(e) => setProjectName(e.target.value)}
                placeholder={t(
                  'projects:createWizard.detailsStep.projectName.placeholder'
                )}
                disabled={createProject.isPending}
              />
            </div>

            {requiresUnsafeAck && selectedRepo && (
              <Alert variant="destructive">
                <AlertDescription className="space-y-3">
                  <div className="font-medium">
                    {t('projects:createWizard.unsafePath.title')}
                  </div>
                  <div className="text-sm">
                    {t('projects:createWizard.unsafePath.description')}
                  </div>
                  <div className="rounded-md bg-background/40 px-3 py-2 text-xs font-mono break-all">
                    {selectedRepo.path}
                  </div>
                  <ul className="list-disc pl-5 text-sm">
                    {unsafeWarnings.includes('temp_dir') && (
                      <li>
                        {t('projects:createWizard.unsafePath.warnings.tempDir')}
                      </li>
                    )}
                    {unsafeWarnings.includes('git_worktree') && (
                      <li>
                        {t(
                          'projects:createWizard.unsafePath.warnings.gitWorktree'
                        )}
                      </li>
                    )}
                  </ul>
                  <label className="flex items-start gap-2 text-sm">
                    <input
                      data-testid="project-wizard-unsafe-ack"
                      id={uiIds.projectWizardUnsafeAck}
                      type="checkbox"
                      className="mt-1"
                      checked={unsafeAck}
                      onChange={(e) => setUnsafeAck(e.target.checked)}
                      disabled={createProject.isPending}
                    />
                    <span>
                      {t('projects:createWizard.unsafePath.ackLabel')}
                    </span>
                  </label>
                </AlertDescription>
              </Alert>
            )}
          </div>
        )}

        {createProject.isError && (
          <Alert variant="destructive">
            <AlertCircle className="h-4 w-4" />
            <AlertDescription>
              {createProject.error instanceof Error
                ? createProject.error.message
                : t('projects:createWizard.errors.createFailed')}
            </AlertDescription>
          </Alert>
        )}

        <DialogFooter className="gap-2">
          {step === 'details' ? (
            <Button
              type="button"
              variant="outline"
              onClick={handleBack}
              disabled={createProject.isPending}
            >
              <ArrowLeft className="mr-2 h-4 w-4" />
              {t('common:buttons.back')}
            </Button>
          ) : selectedRepo ? (
            <Button
              type="button"
              variant="outline"
              onClick={handleContinue}
              disabled={createProject.isPending}
            >
              {t('common:buttons.continue')}
            </Button>
          ) : null}

          <Button
            type="button"
            variant="outline"
            onClick={handleCancel}
            disabled={createProject.isPending}
          >
            {t('common:buttons.cancel')}
          </Button>

          {step === 'details' ? (
            <Button
              type="button"
              onClick={handleCreate}
              disabled={!canCreate}
              id={uiIds.projectWizardSubmitCreate}
              data-testid="project-wizard-create"
            >
              {createProject.isPending ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  {t('projects:createWizard.detailsStep.creating')}
                </>
              ) : (
                t('projects:createWizard.detailsStep.create')
              )}
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});

export const ProjectFormDialog = defineModal<
  ProjectFormDialogProps,
  ProjectFormDialogResult
>(ProjectFormDialogImpl);
