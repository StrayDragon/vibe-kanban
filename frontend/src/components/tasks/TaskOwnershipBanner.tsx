import { Bot, Play, UserRound } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { TaskWithAttemptStatus } from 'shared/types';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  getProjectExecutionModeLabel,
  getResumeAutomationMode,
  getTaskAutomationModeLabel,
  getTaskAutomationOwnershipPresentation,
  getTaskAutomationOwnerKey,
} from '@/utils/automation';

interface TaskOwnershipBannerProps {
  task: TaskWithAttemptStatus;
  onStartAttempt: () => void;
  onEnableAuto: () => void;
  onTakeOverManually: () => void;
  isMutating?: boolean;
}

export function TaskOwnershipBanner({
  task,
  onStartAttempt,
  onEnableAuto,
  onTakeOverManually,
  isMutating = false,
}: TaskOwnershipBannerProps) {
  const { t } = useTranslation('tasks');
  const ownership = getTaskAutomationOwnershipPresentation(task);
  const ownerKey = getTaskAutomationOwnerKey(task);
  const canResumeAuto =
    task.automation_mode === 'manual' || ownerKey === 'manual';
  const enableAutoLabel =
    getResumeAutomationMode(task) === 'inherit'
      ? t('taskPanel.resumeProjectAuto', {
          defaultValue: 'Resume project auto',
        })
      : t('taskPanel.enableAuto', { defaultValue: 'Enable auto' });

  return (
    <div className="rounded-xl border border-border/70 bg-background/80 p-4 space-y-4">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div className="space-y-2">
          <div className="text-xs font-semibold uppercase tracking-[0.14em] text-muted-foreground">
            {t('taskPanel.ownershipTitle')}
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant={ownership.variant}>{ownership.label}</Badge>
            {task.project_execution_mode === 'auto' && (
              <Badge variant="outline">
                {t('taskPanel.projectMode')}:{' '}
                {getProjectExecutionModeLabel(task.project_execution_mode)}
              </Badge>
            )}
            <Badge variant="outline">
              {t('taskPanel.taskMode')}: {getTaskAutomationModeLabel(task)}
            </Badge>
            <Badge variant="outline">
              {t('taskPanel.effectiveMode')}: {task.effective_automation_mode}
            </Badge>
          </div>
          <div className="text-sm text-muted-foreground max-w-2xl">
            {ownership.detail}
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {ownerKey === 'manual' && (
            <>
              <Button size="sm" onClick={onStartAttempt}>
                <Play className="mr-2 h-4 w-4" />
                {t('toolbar.startAttempt')}
              </Button>
              {canResumeAuto && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={onEnableAuto}
                  disabled={isMutating}
                >
                  <Bot className="mr-2 h-4 w-4" />
                  {enableAutoLabel}
                </Button>
              )}
            </>
          )}

          {(ownerKey === 'managed' || ownerKey === 'blocked') && (
            <Button
              size="sm"
              variant="outline"
              onClick={onTakeOverManually}
              disabled={isMutating}
            >
              <UserRound className="mr-2 h-4 w-4" />
              {t('taskPanel.takeOverManual', {
                defaultValue: 'Take over manually',
              })}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
