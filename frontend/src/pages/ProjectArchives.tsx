import { useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';

import { archivedKanbansApi } from '@/lib/api';
import { useProject } from '@/contexts/ProjectContext';
import { paths } from '@/lib/paths';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Loader } from '@/components/ui/loader';
import { ArchiveKanbanDialog } from '@/components/dialogs';

export function ProjectArchives() {
  const { t } = useTranslation(['tasks', 'common']);
  const navigate = useNavigate();
  const { projectId, project } = useProject();

  const {
    data: archives = [],
    isLoading,
    error,
    refetch,
  } = useQuery({
    queryKey: ['archived-kanbans', projectId],
    queryFn: () => archivedKanbansApi.listByProject(projectId!),
    enabled: Boolean(projectId),
    staleTime: 15_000,
  });

  const handleArchive = useCallback(async () => {
    if (!projectId) return;
    try {
      const result = await ArchiveKanbanDialog.show({ projectId });
      if (result?.archiveId) {
        navigate(paths.projectArchive(projectId, result.archiveId));
      }
    } finally {
      ArchiveKanbanDialog.hide();
      void refetch();
    }
  }, [navigate, projectId, refetch]);

  if (!projectId) {
    return (
      <div className="max-w-5xl mx-auto mt-8 px-6">
        <Alert variant="destructive">
          <AlertTitle>{t('common:error')}</AlertTitle>
          <AlertDescription>{t('archives.projectMissing')}</AlertDescription>
        </Alert>
      </div>
    );
  }

  if (isLoading) {
    return <Loader message={t('loading')} size={32} className="py-8" />;
  }

  if (error) {
    return (
      <div className="max-w-5xl mx-auto mt-8 px-6">
        <Alert variant="destructive">
          <AlertTitle>{t('common:error')}</AlertTitle>
          <AlertDescription>
            {error instanceof Error ? error.message : t('archives.loadError')}
          </AlertDescription>
        </Alert>
      </div>
    );
  }

  return (
    <div className="h-full w-full overflow-y-auto">
      <div className="max-w-5xl mx-auto px-6 py-6 space-y-6">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="space-y-1">
            <h1 className="text-2xl font-semibold tracking-tight">
              {t('archives.title')}
            </h1>
            <p className="text-sm text-muted-foreground">
              {project?.name
                ? t('archives.subtitleWithProject', { name: project.name })
                : t('archives.subtitle')}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              onClick={() => navigate(paths.projectTasks(projectId))}
            >
              {t('archives.backToKanban')}
            </Button>
            <Button onClick={handleArchive}>
              {t('archives.archiveButton')}
            </Button>
          </div>
        </div>

        {archives.length === 0 ? (
          <Card>
            <CardContent className="text-center py-10">
              <p className="text-muted-foreground">{t('archives.empty')}</p>
            </CardContent>
          </Card>
        ) : (
          <div className="space-y-3">
            {archives.map((entry) => (
              <Card
                key={entry.id}
                className="cursor-pointer hover:bg-muted/30 transition-colors"
                onClick={() =>
                  navigate(paths.projectArchive(projectId, entry.id))
                }
              >
                <CardContent className="py-4 flex items-start justify-between gap-4">
                  <div className="min-w-0">
                    <div className="font-medium truncate">{entry.title}</div>
                    <div className="text-xs text-muted-foreground mt-1">
                      {new Date(entry.created_at).toLocaleString()}
                    </div>
                  </div>
                  <div className="text-sm text-muted-foreground shrink-0">
                    {t('archives.tasksCount', {
                      count: Number(entry.tasks_count),
                    })}
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
