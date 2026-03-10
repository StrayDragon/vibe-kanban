import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useTranslation } from 'react-i18next';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Alert, AlertDescription } from '@/components/ui/alert';
import {
  AlertCircle,
  ArrowLeft,
  Folder,
  FolderGit,
  FolderPlus,
  Loader2,
  Search,
} from 'lucide-react';
import { fileSystemApi, repoApi } from '@/lib/api';
import { uiIds } from '@/lib/uiIds';
import { DirectoryEntry, Repo } from 'shared/types';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/lib/modals';
import { FolderPickerDialog } from './FolderPickerDialog';

export interface RepoPickerDialogProps {
  value?: string;
  title?: string;
  description?: string;
}

type Stage = 'options' | 'existing' | 'new';

const RepoPickerDialogImpl = NiceModal.create<RepoPickerDialogProps>(
  ({ title, description }) => {
    const modal = useModal();
    const { t } = useTranslation('common');
    const resolvedTitle = title ?? t('repoPicker.title');
    const resolvedDescription = description ?? t('repoPicker.description');
    const [stage, setStage] = useState<Stage>('options');
    const [error, setError] = useState('');
    const [isWorking, setIsWorking] = useState(false);

    // Stage: existing
    const [allRepos, setAllRepos] = useState<DirectoryEntry[]>([]);
    const [reposLoading, setReposLoading] = useState(false);
    const [showMoreRepos, setShowMoreRepos] = useState(false);

    // Stage: new
    const [repoName, setRepoName] = useState('');
    const [parentPath, setParentPath] = useState('');

    useEffect(() => {
      if (modal.visible) {
        setStage('options');
        setError('');
        setAllRepos([]);
        setShowMoreRepos(false);
        setRepoName('');
        setParentPath('');
      }
    }, [modal.visible]);

    const loadRecentRepos = useCallback(async () => {
      setReposLoading(true);
      setError('');
      try {
        const repos = await fileSystemApi.listGitRepos();
        setAllRepos(repos);
      } catch (err) {
        setError(t('repoPicker.errors.loadFailed'));
        console.error('Failed to load repos:', err);
      } finally {
        setReposLoading(false);
      }
    }, [t]);

    useEffect(() => {
      if (stage === 'existing' && allRepos.length === 0 && !reposLoading) {
        loadRecentRepos();
      }
    }, [stage, allRepos.length, reposLoading, loadRecentRepos]);

    const registerAndReturn = async (path: string) => {
      setIsWorking(true);
      setError('');
      try {
        const repo = await repoApi.register({ path });
        modal.resolve(repo);
        modal.hide();
      } catch (err) {
        setError(
          err instanceof Error
            ? err.message
            : t('repoPicker.errors.registerFailed')
        );
      } finally {
        setIsWorking(false);
      }
    };

    const handleSelectRepo = (repo: DirectoryEntry) => {
      registerAndReturn(repo.path);
    };

    const handleBrowseForRepo = async () => {
      setError('');
      const selectedPath = await FolderPickerDialog.show({
        title: t('repoPicker.browseExisting.title'),
        description: t('repoPicker.browseExisting.description'),
      });
      if (selectedPath) {
        registerAndReturn(selectedPath);
      }
    };

    const handleCreateRepo = async () => {
      if (!repoName.trim()) {
        setError(t('repoPicker.new.nameRequired'));
        return;
      }

      setIsWorking(true);
      setError('');
      try {
        const repo = await repoApi.init({
          parent_path: parentPath.trim() || '.',
          folder_name: repoName.trim(),
        });
        modal.resolve(repo);
        modal.hide();
      } catch (err) {
        setError(
          err instanceof Error
            ? err.message
            : t('repoPicker.errors.createFailed')
        );
      } finally {
        setIsWorking(false);
      }
    };

    const handleCancel = () => {
      modal.resolve(null);
      modal.hide();
    };

    const handleOpenChange = (open: boolean) => {
      if (!open && !isWorking) {
        handleCancel();
      }
    };

    const goBack = () => {
      setStage('options');
      setError('');
    };

    return (
      <div className="fixed inset-0 z-[10000] pointer-events-none [&>*]:pointer-events-auto">
        <Dialog open={modal.visible} onOpenChange={handleOpenChange}>
          <DialogContent className="max-w-[500px] w-full">
            <DialogHeader>
              <DialogTitle>{resolvedTitle}</DialogTitle>
              <DialogDescription>{resolvedDescription}</DialogDescription>
            </DialogHeader>

            <div className="space-y-4">
              {/* Stage: Options */}
              {stage === 'options' && (
                <>
                  <button
                    type="button"
                    className="w-full rounded-lg border bg-card p-4 text-left transition-shadow hover:shadow-md"
                    onClick={() => setStage('existing')}
                    id={uiIds.repoPickerOptionExisting}
                  >
                    <div className="flex items-start gap-3">
                      <FolderGit className="h-5 w-5 mt-0.5 flex-shrink-0 text-muted-foreground" />
                      <div className="min-w-0 flex-1">
                        <div className="font-medium text-foreground">
                          {t('repoPicker.options.existing.title')}
                        </div>
                        <div className="text-xs text-muted-foreground mt-1">
                          {t('repoPicker.options.existing.description')}
                        </div>
                      </div>
                    </div>
                  </button>

                  <button
                    type="button"
                    className="w-full rounded-lg border bg-card p-4 text-left transition-shadow hover:shadow-md"
                    onClick={() => setStage('new')}
                    id={uiIds.repoPickerOptionNew}
                  >
                    <div className="flex items-start gap-3">
                      <FolderPlus className="h-5 w-5 mt-0.5 flex-shrink-0 text-muted-foreground" />
                      <div className="min-w-0 flex-1">
                        <div className="font-medium text-foreground">
                          {t('repoPicker.options.new.title')}
                        </div>
                        <div className="text-xs text-muted-foreground mt-1">
                          {t('repoPicker.options.new.description')}
                        </div>
                      </div>
                    </div>
                  </button>
                </>
              )}

              {/* Stage: Existing */}
              {stage === 'existing' && (
                <>
                  <button
                    type="button"
                    className="text-sm text-muted-foreground hover:text-foreground flex items-center gap-1"
                    onClick={goBack}
                    disabled={isWorking}
                  >
                    <ArrowLeft className="h-3 w-3" />
                    {t('repoPicker.backToOptions')}
                  </button>

                  {reposLoading && (
                    <div className="p-4 border rounded-lg bg-card">
                      <div className="flex items-center gap-3">
                        <div className="animate-spin h-5 w-5 border-2 border-muted-foreground border-t-transparent rounded-full" />
                        <div className="text-sm text-muted-foreground">
                          {t('repoPicker.existing.loading')}
                        </div>
                      </div>
                    </div>
                  )}

                  {!reposLoading && allRepos.length > 0 && (
                    <div className="space-y-2">
                      {allRepos
                        .slice(0, showMoreRepos ? allRepos.length : 3)
                        .map((repo) => (
                          <button
                            key={repo.path}
                            type="button"
                            className="w-full rounded-lg border bg-card p-4 text-left transition-shadow hover:shadow-md"
                            onClick={() => !isWorking && handleSelectRepo(repo)}
                            disabled={isWorking}
                          >
                            <div className="flex items-start gap-3">
                              <FolderGit className="h-5 w-5 mt-0.5 flex-shrink-0 text-muted-foreground" />
                              <div className="min-w-0 flex-1">
                                <div className="font-medium text-foreground">
                                  {repo.name}
                                </div>
                                <div className="text-xs text-muted-foreground truncate mt-1">
                                  {repo.path}
                                </div>
                              </div>
                            </div>
                          </button>
                        ))}

                      {!showMoreRepos && allRepos.length > 3 && (
                        <button
                          type="button"
                          className="text-sm text-muted-foreground hover:text-foreground transition-colors text-left"
                          onClick={() => setShowMoreRepos(true)}
                        >
                          {t('repoPicker.existing.showMore', {
                            count: allRepos.length - 3,
                          })}
                        </button>
                      )}
                      {showMoreRepos && allRepos.length > 3 && (
                        <button
                          type="button"
                          className="text-sm text-muted-foreground hover:text-foreground transition-colors text-left"
                          onClick={() => setShowMoreRepos(false)}
                        >
                          {t('repoPicker.existing.showLess')}
                        </button>
                      )}
                    </div>
                  )}

                  <button
                    type="button"
                    className="w-full rounded-lg border border-dashed bg-card p-4 text-left transition-shadow hover:shadow-md"
                    onClick={() => !isWorking && handleBrowseForRepo()}
                    disabled={isWorking}
                    id={uiIds.repoPickerBrowse}
                  >
                    <div className="flex items-start gap-3">
                      <Search className="h-5 w-5 mt-0.5 flex-shrink-0 text-muted-foreground" />
                      <div className="min-w-0 flex-1">
                        <div className="font-medium text-foreground">
                          {t('repoPicker.existing.browse.title')}
                        </div>
                        <div className="text-xs text-muted-foreground mt-1">
                          {t('repoPicker.existing.browse.description')}
                        </div>
                      </div>
                    </div>
                  </button>
                </>
              )}

              {/* Stage: New */}
              {stage === 'new' && (
                <>
                  <button
                    type="button"
                    className="text-sm text-muted-foreground hover:text-foreground flex items-center gap-1"
                    onClick={goBack}
                    disabled={isWorking}
                  >
                    <ArrowLeft className="h-3 w-3" />
                    {t('repoPicker.backToOptions')}
                  </button>

                  <div className="space-y-4">
                    <div className="space-y-2">
                      <Label htmlFor={uiIds.repoPickerName}>
                        {t('repoPicker.new.nameLabel')}{' '}
                        <span className="text-red-500">*</span>
                      </Label>
                      <Input
                        id={uiIds.repoPickerName}
                        type="text"
                        value={repoName}
                        onChange={(e) => setRepoName(e.target.value)}
                        placeholder={t('repoPicker.new.namePlaceholder')}
                        disabled={isWorking}
                      />
                      <p className="text-xs text-muted-foreground">
                        {t('repoPicker.new.nameHelper')}
                      </p>
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor={uiIds.repoPickerParentPath}>
                        {t('repoPicker.new.parentLabel')}
                      </Label>
                      <div className="flex space-x-2">
                        <Input
                          id={uiIds.repoPickerParentPath}
                          type="text"
                          value={parentPath}
                          onChange={(e) => setParentPath(e.target.value)}
                          placeholder={t('repoPicker.new.parentPlaceholder')}
                          className="flex-1"
                          disabled={isWorking}
                        />
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          disabled={isWorking}
                          aria-label={t('repoPicker.new.browseParentAria')}
                          title={t('repoPicker.new.browseParentAria')}
                          onClick={async () => {
                            const selectedPath = await FolderPickerDialog.show({
                              title: t('repoPicker.new.browseParent.title'),
                              description: t(
                                'repoPicker.new.browseParent.description'
                              ),
                              value: parentPath,
                            });
                            if (selectedPath) {
                              setParentPath(selectedPath);
                            }
                          }}
                        >
                          <Folder className="h-4 w-4" />
                        </Button>
                      </div>
                      <p className="text-xs text-muted-foreground">
                        {t('repoPicker.new.parentHelper')}
                      </p>
                    </div>

                    <Button
                      onClick={handleCreateRepo}
                      disabled={isWorking || !repoName.trim()}
                      className="w-full"
                      id={uiIds.repoPickerSubmitCreate}
                    >
                      {isWorking ? (
                        <>
                          <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                          {t('repoPicker.new.creating')}
                        </>
                      ) : (
                        t('repoPicker.new.create')
                      )}
                    </Button>
                  </div>
                </>
              )}

              {error && (
                <Alert variant="destructive">
                  <AlertCircle className="h-4 w-4" />
                  <AlertDescription>{error}</AlertDescription>
                </Alert>
              )}

              {isWorking && stage === 'existing' && (
                <div className="flex items-center justify-center gap-2 text-sm text-muted-foreground">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  {t('repoPicker.existing.registering')}
                </div>
              )}
            </div>
          </DialogContent>
        </Dialog>
      </div>
    );
  }
);

export const RepoPickerDialog = defineModal<RepoPickerDialogProps, Repo | null>(
  RepoPickerDialogImpl
);
