import type { TaskWithAttemptStatus } from 'shared/types';
import type { TaskWithGroup } from '@/types/task-group';

export const isTaskGroupEntry = (task: TaskWithAttemptStatus): boolean => {
  const typed = task as TaskWithGroup;
  const kind = typed.task_kind ?? typed.taskKind;
  if (kind) return kind === 'group';
  return Boolean(typed.task_group_id ?? typed.taskGroupId);
};

export const getTaskGroupId = (task: TaskWithAttemptStatus): string | null => {
  const typed = task as TaskWithGroup;
  return typed.task_group_id ?? typed.taskGroupId ?? null;
};

export const isTaskGroupSubtask = (task: TaskWithAttemptStatus): boolean => {
  const typed = task as TaskWithGroup;
  const groupId = typed.task_group_id ?? typed.taskGroupId;
  if (!groupId) return false;
  const kind = typed.task_kind ?? typed.taskKind;
  if (!kind) return true;
  return kind !== 'group';
};
