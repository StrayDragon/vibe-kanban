import type { ProjectExecutionMode, TaskWithAttemptStatus } from 'shared/types';
import { TaskOrchestrationSummaryStrip } from './TaskOrchestrationSummaryStrip';
import type { OrchestrationFilter } from '@/utils/automation';

interface TaskOrchestrationOverviewProps {
  tasks: TaskWithAttemptStatus[];
  activeFilter: OrchestrationFilter;
  onFilterChange: (filter: OrchestrationFilter) => void;
  projectExecutionMode?: ProjectExecutionMode;
  className?: string;
}

export function TaskOrchestrationOverview({
  tasks,
  activeFilter,
  onFilterChange,
  projectExecutionMode,
  className,
}: TaskOrchestrationOverviewProps) {
  return (
    <TaskOrchestrationSummaryStrip
      tasks={tasks}
      filter={activeFilter}
      onFilterChange={onFilterChange}
      title="Orchestration"
      subtitle="Separate manual work, auto-managed runs, review handoffs, and blocked tasks."
      projectExecutionMode={projectExecutionMode}
      className={className}
    />
  );
}
