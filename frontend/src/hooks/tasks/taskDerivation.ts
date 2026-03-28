import type { TaskStatus, TaskWithAttemptStatus } from 'shared/types';

export const TASK_STATUSES: TaskStatus[] = [
  'todo',
  'inprogress',
  'inreview',
  'done',
  'cancelled',
];

export const EMPTY_TASKS_BY_STATUS: Record<
  TaskStatus,
  TaskWithAttemptStatus[]
> = {
  todo: [],
  inprogress: [],
  inreview: [],
  done: [],
  cancelled: [],
};

export type TaskDerivationCache = {
  tasks: TaskWithAttemptStatus[];
  tasksByStatus: Record<TaskStatus, TaskWithAttemptStatus[]>;
  indexById: Map<string, number>;
  indexByIdByStatus: Record<TaskStatus, Map<string, number>>;
  statusById: Map<string, TaskStatus>;
  createdAtMsById: Map<string, number>;
};

const createdAtMsForTask = (
  task: TaskWithAttemptStatus,
  createdAtMsById: Map<string, number>
) => {
  const cached = createdAtMsById.get(task.id);
  if (typeof cached === 'number') return cached;
  const parsed = Date.parse(task.created_at as unknown as string);
  const value = Number.isFinite(parsed) ? parsed : 0;
  createdAtMsById.set(task.id, value);
  return value;
};

const compareTaskCreatedDesc = (
  a: TaskWithAttemptStatus,
  b: TaskWithAttemptStatus,
  createdAtMsById: Map<string, number>
) => {
  const msDiff =
    createdAtMsForTask(b, createdAtMsById) -
    createdAtMsForTask(a, createdAtMsById);
  if (msDiff !== 0) return msDiff;
  return a.id.localeCompare(b.id);
};

const findInsertIndexByCreatedAtDesc = (
  list: TaskWithAttemptStatus[],
  task: TaskWithAttemptStatus,
  createdAtMsById: Map<string, number>
) => {
  const taskMs = createdAtMsForTask(task, createdAtMsById);
  const taskId = task.id;
  let lo = 0;
  let hi = list.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    const midTask = list[mid];
    const midMs = createdAtMsForTask(midTask, createdAtMsById);
    if (midMs > taskMs) {
      lo = mid + 1;
      continue;
    }
    if (midMs < taskMs) {
      hi = mid;
      continue;
    }
    if (midTask.id.localeCompare(taskId) <= 0) {
      lo = mid + 1;
    } else {
      hi = mid;
    }
  }
  return lo;
};

export const buildTaskDerivationCache = (
  tasks: TaskWithAttemptStatus[]
): TaskDerivationCache => {
  const createdAtMsById = new Map<string, number>();

  const sorted = tasks
    .slice()
    .sort((a, b) => compareTaskCreatedDesc(a, b, createdAtMsById));

  const tasksByStatus: Record<TaskStatus, TaskWithAttemptStatus[]> = {
    todo: [],
    inprogress: [],
    inreview: [],
    done: [],
    cancelled: [],
  };

  sorted.forEach((task) => {
    tasksByStatus[task.status]?.push(task);
    createdAtMsForTask(task, createdAtMsById);
  });

  const indexById = new Map<string, number>();
  sorted.forEach((task, idx) => indexById.set(task.id, idx));

  const indexByIdByStatus: Record<TaskStatus, Map<string, number>> = {
    todo: new Map(),
    inprogress: new Map(),
    inreview: new Map(),
    done: new Map(),
    cancelled: new Map(),
  };

  TASK_STATUSES.forEach((status) => {
    tasksByStatus[status].forEach((task, idx) => {
      indexByIdByStatus[status].set(task.id, idx);
    });
  });

  const statusById = new Map<string, TaskStatus>();
  sorted.forEach((task) => statusById.set(task.id, task.status));

  return {
    tasks: sorted,
    tasksByStatus,
    indexById,
    indexByIdByStatus,
    statusById,
    createdAtMsById,
  };
};

export const applyTaskDerivationChanges = (
  cache: TaskDerivationCache,
  changedIds: Iterable<string>,
  getTask: (id: string) => TaskWithAttemptStatus | null
): boolean => {
  const sameStatusUpdates: {
    id: string;
    task: TaskWithAttemptStatus;
    taskIndex: number;
    status: TaskStatus;
    statusIndex: number;
  }[] = [];
  const moves: {
    id: string;
    task: TaskWithAttemptStatus;
    taskIndex: number;
    prevStatus: TaskStatus;
    prevStatusIndex: number;
    nextStatus: TaskStatus;
  }[] = [];
  const deletions: {
    id: string;
    taskIndex: number;
    status: TaskStatus;
    statusIndex: number;
  }[] = [];
  const insertions: { id: string; task: TaskWithAttemptStatus }[] = [];

  for (const id of changedIds) {
    const nextTask = getTask(id);
    const prevTaskIndex = cache.indexById.get(id);

    if (typeof prevTaskIndex !== 'number') {
      if (!nextTask) continue;
      insertions.push({ id, task: nextTask });
      continue;
    }

    if (!nextTask) {
      const prevStatus = cache.statusById.get(id);
      if (!prevStatus) return false;
      const statusIndex = cache.indexByIdByStatus[prevStatus].get(id);
      if (typeof statusIndex !== 'number') return false;
      deletions.push({
        id,
        taskIndex: prevTaskIndex,
        status: prevStatus,
        statusIndex,
      });
      continue;
    }

    // prev exists + next exists
    const createdAtMs = cache.createdAtMsById.get(id);
    if (typeof createdAtMs === 'number') {
      const parsed = Date.parse(nextTask.created_at as unknown as string);
      if (Number.isFinite(parsed) && parsed !== createdAtMs) {
        return false;
      }
    }

    const prevStatus = cache.statusById.get(id);
    if (!prevStatus) return false;
    const prevStatusIndex = cache.indexByIdByStatus[prevStatus].get(id);
    if (typeof prevStatusIndex !== 'number') return false;
    const nextStatus = nextTask.status;

    if (prevStatus === nextStatus) {
      sameStatusUpdates.push({
        id,
        task: nextTask,
        taskIndex: prevTaskIndex,
        status: prevStatus,
        statusIndex: prevStatusIndex,
      });
      continue;
    }

    moves.push({
      id,
      task: nextTask,
      taskIndex: prevTaskIndex,
      prevStatus,
      prevStatusIndex,
      nextStatus,
    });
  }

  const nextTasks = cache.tasks.slice();
  const nextTasksByStatus: Record<TaskStatus, TaskWithAttemptStatus[]> = {
    ...cache.tasksByStatus,
  };
  const createdAtMsById = cache.createdAtMsById;
  const statusesNeedingReindex = new Set<TaskStatus>();

  const ensureStatusArrayClone = (status: TaskStatus) => {
    const current = nextTasksByStatus[status];
    if (current !== cache.tasksByStatus[status]) return current;
    const clone = current.slice();
    nextTasksByStatus[status] = clone;
    return clone;
  };

  // Replace task objects in the global tasks list.
  sameStatusUpdates.forEach(({ taskIndex, task }) => {
    nextTasks[taskIndex] = task;
  });
  moves.forEach(({ taskIndex, task }) => {
    nextTasks[taskIndex] = task;
  });

  // Replace task objects in the same-status arrays (indices remain stable).
  sameStatusUpdates.forEach(({ status, statusIndex, task }) => {
    const list = ensureStatusArrayClone(status);
    list[statusIndex] = task;
  });

  // Apply removals to per-status lists (descending index order per status).
  const removalsByStatus = new Map<TaskStatus, number[]>();
  deletions.forEach(({ status, statusIndex }) => {
    const list = removalsByStatus.get(status) ?? [];
    list.push(statusIndex);
    removalsByStatus.set(status, list);
  });
  moves.forEach(({ prevStatus, prevStatusIndex }) => {
    const list = removalsByStatus.get(prevStatus) ?? [];
    list.push(prevStatusIndex);
    removalsByStatus.set(prevStatus, list);
  });

  removalsByStatus.forEach((indices, status) => {
    indices.sort((a, b) => b - a);
    const list = ensureStatusArrayClone(status);
    indices.forEach((index) => {
      list.splice(index, 1);
    });
    statusesNeedingReindex.add(status);
  });

  // Apply insertions (moves + new tasks) to per-status lists.
  moves.forEach(({ task, nextStatus }) => {
    const list = ensureStatusArrayClone(nextStatus);
    const insertAt = findInsertIndexByCreatedAtDesc(
      list,
      task,
      createdAtMsById
    );
    list.splice(insertAt, 0, task);
    statusesNeedingReindex.add(nextStatus);
  });

  insertions.forEach(({ task }) => {
    const nextStatus = task.status;
    const list = ensureStatusArrayClone(nextStatus);
    const insertAt = findInsertIndexByCreatedAtDesc(
      list,
      task,
      createdAtMsById
    );
    list.splice(insertAt, 0, task);
    statusesNeedingReindex.add(nextStatus);
  });

  // Apply deletions/insertions to the global tasks list (descending removals, then insertions).
  let tasksNeedReindex = false;
  deletions
    .slice()
    .sort((a, b) => b.taskIndex - a.taskIndex)
    .forEach(({ taskIndex }) => {
      nextTasks.splice(taskIndex, 1);
      tasksNeedReindex = true;
    });

  insertions.forEach(({ task }) => {
    const insertAt = findInsertIndexByCreatedAtDesc(
      nextTasks,
      task,
      createdAtMsById
    );
    nextTasks.splice(insertAt, 0, task);
    tasksNeedReindex = true;
  });

  // Mutate internal cache maps last, after we know we're committing to this update.
  moves.forEach(({ id, nextStatus }) => {
    cache.statusById.set(id, nextStatus);
  });
  deletions.forEach(({ id }) => {
    cache.statusById.delete(id);
    createdAtMsById.delete(id);
  });
  insertions.forEach(({ task }) => {
    cache.statusById.set(task.id, task.status);
    createdAtMsForTask(task, createdAtMsById);
  });

  if (tasksNeedReindex) {
    const indexById = new Map<string, number>();
    nextTasks.forEach((task, idx) => indexById.set(task.id, idx));
    cache.indexById = indexById;
  }

  statusesNeedingReindex.forEach((status) => {
    const map = new Map<string, number>();
    nextTasksByStatus[status].forEach((task, idx) => map.set(task.id, idx));
    cache.indexByIdByStatus[status] = map;
  });

  cache.tasks = nextTasks;
  cache.tasksByStatus = nextTasksByStatus;

  return true;
};
