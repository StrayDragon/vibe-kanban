import { useTranslation } from 'react-i18next';
import { CheckSquare, Eye, FileDiff, X } from 'lucide-react';
import { useLocation } from 'react-router-dom';
import { Button } from '../ui/button';
import { ToggleGroup, ToggleGroupItem } from '../ui/toggle-group';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '../ui/tooltip';
import { useNavigateWithSearch } from '@/hooks';
import { paths } from '@/lib/paths';
import type { LayoutMode } from '../layout/TasksLayout';
import type { TaskWithAttemptStatus } from 'shared/types';
import type { Workspace } from 'shared/types';
import { ActionsDropdown } from '../ui/actions-dropdown';

interface AttemptHeaderActionsProps {
  onClose: () => void;
  mode?: LayoutMode;
  onModeChange?: (mode: LayoutMode) => void;
  task: TaskWithAttemptStatus;
  attempt?: Workspace | null;
}

export const AttemptHeaderActions = ({
  onClose,
  mode,
  onModeChange,
  task,
  attempt,
}: AttemptHeaderActionsProps) => {
  const { t } = useTranslation('tasks');
  const navigate = useNavigateWithSearch();
  const location = useLocation();
  const showModeToggle = typeof mode !== 'undefined' && onModeChange;
  const isOverviewRoute = location.pathname.startsWith('/tasks');
  const taskViewLabel = t('attemptHeaderActions.task');

  const handleTaskView = () => {
    const taskPath = isOverviewRoute
      ? paths.overviewTask(task.project_id, task.id)
      : paths.task(task.project_id, task.id);
    navigate(taskPath);
  };

  return (
    <>
      <TooltipProvider>
        <div className="inline-flex items-center gap-4">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="icon"
                aria-label={taskViewLabel}
                onClick={handleTaskView}
              >
                <CheckSquare className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom">{taskViewLabel}</TooltipContent>
          </Tooltip>

          {showModeToggle && (
            <ToggleGroup
              type="single"
              value={mode ?? ''}
              onValueChange={(v) => {
                const newMode = (v as LayoutMode) || null;
                onModeChange(newMode);
              }}
              className="inline-flex gap-4"
              aria-label="Layout mode"
            >
              <Tooltip>
                <TooltipTrigger asChild>
                  <ToggleGroupItem
                    value="preview"
                    aria-label="Preview"
                    active={mode === 'preview'}
                  >
                    <Eye className="h-4 w-4" />
                  </ToggleGroupItem>
                </TooltipTrigger>
                <TooltipContent side="bottom">
                  {t('attemptHeaderActions.preview')}
                </TooltipContent>
              </Tooltip>

              <Tooltip>
                <TooltipTrigger asChild>
                  <ToggleGroupItem
                    value="diffs"
                    aria-label="Diffs"
                    active={mode === 'diffs'}
                  >
                    <FileDiff className="h-4 w-4" />
                  </ToggleGroupItem>
                </TooltipTrigger>
                <TooltipContent side="bottom">
                  {t('attemptHeaderActions.diffs')}
                </TooltipContent>
              </Tooltip>
            </ToggleGroup>
          )}
        </div>
      </TooltipProvider>
      {showModeToggle && (
        <div className="h-4 w-px bg-border" />
      )}
      <ActionsDropdown task={task} attempt={attempt} />
      <Button variant="icon" aria-label="Close" onClick={onClose}>
        <X size={16} />
      </Button>
    </>
  );
};
