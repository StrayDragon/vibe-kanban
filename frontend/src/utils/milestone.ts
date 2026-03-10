import type { TaskWithAttemptStatus } from 'shared/types';

export const isMilestoneEntry = (task: TaskWithAttemptStatus): boolean =>
  task.task_kind === 'milestone';

export const getMilestoneId = (task: TaskWithAttemptStatus): string | null =>
  task.milestone_id ?? null;

export const isMilestoneSubtask = (task: TaskWithAttemptStatus): boolean =>
  Boolean(task.milestone_id) && task.task_kind !== 'milestone';

