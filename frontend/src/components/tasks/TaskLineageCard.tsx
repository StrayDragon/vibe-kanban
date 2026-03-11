import { GitBranchPlus, Link2, Loader2, Sparkles } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useNavigateWithSearch } from '@/hooks';
import { useTaskLineage } from '@/hooks/tasks/useTaskLineage';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import type { TaskWithAttemptStatus } from 'shared/types';

type BuildTaskPath = (projectId: string, taskId: string) => string;

interface TaskLineageCardProps {
  task: TaskWithAttemptStatus;
  buildTaskPath?: BuildTaskPath;
}

function sourceLabel(kind: TaskWithAttemptStatus['created_by_kind']) {
  switch (kind) {
    case 'agent_followup':
      return 'Agent Follow-up';
    case 'milestone_planner':
      return 'Milestone Planner';
    case 'mcp':
      return 'MCP';
    case 'scheduler':
      return 'Scheduler';
    case 'human_ui':
    default:
      return 'Human UI';
  }
}

export function TaskLineageCard({ task, buildTaskPath }: TaskLineageCardProps) {
  const { t } = useTranslation('tasks');
  const navigate = useNavigateWithSearch();
  const { data, isLoading } = useTaskLineage(task.id);

  const originTask = data?.origin_task ?? null;
  const followUps = data?.follow_up_tasks ?? [];
  const showSource = task.created_by_kind !== 'human_ui';

  if (
    !showSource &&
    !task.origin_task_id &&
    followUps.length === 0 &&
    !isLoading
  ) {
    return null;
  }

  const toTaskPath = (projectId: string, taskId: string) =>
    buildTaskPath
      ? buildTaskPath(projectId, taskId)
      : `/projects/${projectId}/tasks/${taskId}`;

  return (
    <div className="rounded-xl border border-border/70 bg-background/80 p-4 space-y-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 text-sm font-semibold">
          <Link2 className="h-4 w-4 text-muted-foreground" />
          {t('taskPanel.lineage.title')}
        </div>
        {showSource && (
          <Badge variant="outline" className="gap-1.5">
            <Sparkles className="h-3.5 w-3.5" />
            {sourceLabel(task.created_by_kind)}
          </Badge>
        )}
      </div>

      {task.origin_task_id && (
        <div className="space-y-1">
          <div className="text-xs uppercase tracking-[0.12em] text-muted-foreground">
            {t('taskPanel.lineage.createdFrom')}
          </div>
          {isLoading && !originTask ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              {t('taskPanel.lineage.loading')}
            </div>
          ) : originTask ? (
            <Button
              type="button"
              variant="outline"
              className="h-auto min-h-10 w-full justify-between gap-3 px-3 py-2 text-left"
              onClick={() =>
                navigate(toTaskPath(originTask.project_id, originTask.id))
              }
            >
              <div className="min-w-0">
                <div className="text-sm font-medium line-clamp-1">
                  {originTask.title || t('taskPanel.lineage.untitled')}
                </div>
                <div className="text-xs text-muted-foreground">
                  {originTask.status}
                </div>
              </div>
              <Link2 className="h-4 w-4 shrink-0 text-muted-foreground" />
            </Button>
          ) : (
            <div className="text-sm text-muted-foreground">
              {t('taskPanel.lineage.originUnavailable')}
            </div>
          )}
        </div>
      )}

      {followUps.length > 0 && (
        <div className="space-y-2">
          <div className="text-xs uppercase tracking-[0.12em] text-muted-foreground">
            {t('taskPanel.lineage.followUps', { count: followUps.length })}
          </div>
          <div className="grid gap-2">
            {followUps.map((followUp) => (
              <Button
                key={followUp.id}
                type="button"
                variant="outline"
                className="h-auto min-h-10 w-full justify-between gap-3 px-3 py-2 text-left"
                onClick={() =>
                  navigate(toTaskPath(followUp.project_id, followUp.id))
                }
              >
                <div className="min-w-0">
                  <div className="text-sm font-medium line-clamp-1">
                    {followUp.title || t('taskPanel.lineage.untitled')}
                  </div>
                  <div className="text-xs text-muted-foreground">
                    {followUp.status}
                  </div>
                </div>
                <GitBranchPlus className="h-4 w-4 shrink-0 text-muted-foreground" />
              </Button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
