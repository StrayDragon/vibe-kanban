import { useCallback, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';

import { archivedKanbansApi } from '@/lib/api';
import { useProject } from '@/contexts/ProjectContext';
import { paths } from '@/lib/paths';
import { useArchivedKanbanTasks } from '@/hooks/archived-kanbans/useArchivedKanbanTasks';
import { archivedKanbanKeys } from '@/query-keys/archivedKanbanKeys';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Loader } from '@/components/ui/loader';
import TaskKanbanBoard from '@/components/tasks/TaskKanbanBoard';
import {
  ArchivedTaskDetailsDialog,
  DeleteArchivedKanbanDialog,
  RestoreArchivedKanbanDialog,
} from '@/components/dialogs';

import type { TaskWithAttemptStatus } from 'shared/types';

export function ProjectArchiveDetail() {
  const { t } = useTranslation(['tasks', 'common']);
  const navigate = useNavigate();
  const { archiveId } = useParams<{ archiveId: string }>();
  const { projectId } = useProject();

  const [selectedTaskId, setSelectedTaskId] = useState<string | undefined>(
    undefined
  );

  const {
    data: archiveData,
    isLoading,
    error,
  } = useQuery({
    queryKey: archivedKanbanKeys.byId(archiveId),
    queryFn: () => archivedKanbansApi.getById(archiveId!),
    enabled: Boolean(archiveId),
    staleTime: 15_000,
  });

  const archive = archiveData?.archived_kanban ?? null;

  const {
    tasksByStatus,
    tasks,
    isLoading: tasksLoading,
    error: tasksError,
  } = useArchivedKanbanTasks(archiveId ?? '');

  const handleViewTaskDetails = useCallback(
    async (task: TaskWithAttemptStatus) => {
      setSelectedTaskId(task.id);
      try {
        await ArchivedTaskDetailsDialog.show({ task });
      } finally {
        ArchivedTaskDetailsDialog.hide();
        setSelectedTaskId(undefined);
      }
    },
    []
  );

  const handleRestore = useCallback(async () => {
    if (!archiveId) return;
    try {
      await RestoreArchivedKanbanDialog.show({ archiveId });
      navigate(paths.projectTasks(projectId!));
    } finally {
      RestoreArchivedKanbanDialog.hide();
    }
  }, [archiveId, navigate, projectId]);

  const handleDelete = useCallback(async () => {
    if (!archiveId || !archive) return;
    try {
      await DeleteArchivedKanbanDialog.show({
        archiveId,
        archiveTitle: archive.title,
        tasksCount: Number(archive.tasks_count),
      });
      navigate(paths.projectArchives(projectId!));
    } finally {
      DeleteArchivedKanbanDialog.hide();
    }
  }, [archiveId, archive, navigate, projectId]);

  const kanban = useMemo(() => {
    return (
      <div className="w-full h-full overflow-x-auto overflow-y-auto overscroll-x-contain">
        <TaskKanbanBoard
          columns={tasksByStatus}
          onDragEnd={() => {}}
          onViewTaskDetails={handleViewTaskDetails}
          selectedTaskId={selectedTaskId}
          projectId={projectId ?? ''}
          readOnly
        />
      </div>
    );
  }, [handleViewTaskDetails, projectId, selectedTaskId, tasksByStatus]);

  if (!archiveId || !projectId) {
    return (
      <div className="max-w-5xl mx-auto mt-8 px-6">
        <Alert variant="destructive">
          <AlertTitle>{t('common:error')}</AlertTitle>
          <AlertDescription>{t('archives.archiveMissing')}</AlertDescription>
        </Alert>
      </div>
    );
  }

  if (isLoading) {
    return <Loader message={t('loading')} size={32} className="py-8" />;
  }

  if (error || !archive) {
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
    <div className="h-full w-full flex flex-col overflow-hidden">
      <div className="shrink-0 border-b bg-background">
        <div className="max-w-7xl mx-auto px-6 py-4 flex flex-wrap items-center justify-between gap-3">
          <div className="min-w-0">
            <div className="text-sm text-muted-foreground">
              {t('archives.detailLabel')}
            </div>
            <div className="text-xl font-semibold truncate">
              {archive.title}
            </div>
            <div className="text-xs text-muted-foreground mt-1">
              {t('archives.tasksCount', { count: tasks.length })} ·{' '}
              {new Date(archive.created_at).toLocaleString()}
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              onClick={() => navigate(paths.projectArchives(projectId))}
            >
              {t('archives.backToArchives')}
            </Button>
            <Button variant="outline" onClick={handleRestore}>
              {t('archives.restoreButton')}
            </Button>
            <Button variant="destructive" onClick={handleDelete}>
              {t('archives.deleteButton')}
            </Button>
          </div>
        </div>
      </div>

      {tasksError ? (
        <div className="max-w-5xl mx-auto mt-8 px-6">
          <Alert variant="destructive">
            <AlertTitle>{t('common:error')}</AlertTitle>
            <AlertDescription>{tasksError}</AlertDescription>
          </Alert>
        </div>
      ) : tasksLoading ? (
        <Loader message={t('loading')} size={32} className="py-8" />
      ) : (
        <div className="flex-1 min-h-0">{kanban}</div>
      )}
    </div>
  );
}
