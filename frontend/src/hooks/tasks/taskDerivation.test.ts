import { describe, expect, it } from 'vitest';

import type { TaskWithAttemptStatus } from 'shared/types';

import {
  applyTaskDerivationChanges,
  buildTaskDerivationCache,
} from './taskDerivation';

function makeTask(overrides: Partial<TaskWithAttemptStatus>) {
  const projectId = overrides.project_id ?? 'project';
  const taskId = overrides.id ?? 'task';
  const createdAt = overrides.created_at ?? new Date(0).toISOString();
  const updatedAt = overrides.updated_at ?? createdAt;
  return {
    has_in_progress_attempt: false,
    last_attempt_failed: false,
    executor: '',
    dispatch_state: null,
    orchestration: null,
    id: taskId,
    project_id: projectId,
    title: 'Test',
    description: null,
    status: 'todo',
    task_kind: 'default',
    milestone_id: null,
    milestone_node_id: null,
    parent_workspace_id: null,
    origin_task_id: null,
    created_by_kind: 'human_ui',
    continuation_turns_override: null,
    shared_task_id: null,
    archived_kanban_id: null,
    created_at: createdAt,
    updated_at: updatedAt,
    ...overrides,
  } satisfies TaskWithAttemptStatus;
}

const deriveBaseline = (tasksById: Record<string, TaskWithAttemptStatus>) => {
  const tasks = Object.values(tasksById).sort((a, b) => {
    const msDiff =
      new Date(b.created_at as string).getTime() -
      new Date(a.created_at as string).getTime();
    if (msDiff !== 0) return msDiff;
    return a.id.localeCompare(b.id);
  });
  const tasksByStatus = {
    todo: [] as TaskWithAttemptStatus[],
    inprogress: [] as TaskWithAttemptStatus[],
    inreview: [] as TaskWithAttemptStatus[],
    done: [] as TaskWithAttemptStatus[],
    cancelled: [] as TaskWithAttemptStatus[],
  };
  tasks.forEach((task) => {
    tasksByStatus[task.status]?.push(task);
  });
  return { tasks, tasksByStatus };
};

describe('taskDerivation', () => {
  it('builds stable sorted lists and status buckets', () => {
    const tasksById = {
      b: makeTask({
        id: 'b',
        status: 'done',
        created_at: new Date(1).toISOString(),
      }),
      a: makeTask({
        id: 'a',
        status: 'todo',
        created_at: new Date(1).toISOString(),
      }),
      c: makeTask({
        id: 'c',
        status: 'inprogress',
        created_at: new Date(2).toISOString(),
      }),
    };

    const cache = buildTaskDerivationCache(Object.values(tasksById));
    const baseline = deriveBaseline(tasksById);

    expect(cache.tasks.map((t) => t.id)).toEqual(
      baseline.tasks.map((t) => t.id)
    );
    expect(cache.tasksByStatus.todo.map((t) => t.id)).toEqual(
      baseline.tasksByStatus.todo.map((t) => t.id)
    );
    expect(cache.tasksByStatus.done.map((t) => t.id)).toEqual(
      baseline.tasksByStatus.done.map((t) => t.id)
    );
  });

  it('keeps unaffected status list references stable for same-status updates', () => {
    const tasksById: Record<string, TaskWithAttemptStatus> = {
      a: makeTask({
        id: 'a',
        status: 'todo',
        created_at: new Date(1).toISOString(),
      }),
      b: makeTask({
        id: 'b',
        status: 'done',
        created_at: new Date(2).toISOString(),
      }),
      c: makeTask({
        id: 'c',
        status: 'inprogress',
        created_at: new Date(3).toISOString(),
      }),
    };

    const cache = buildTaskDerivationCache(Object.values(tasksById));
    const prevTodo = cache.tasksByStatus.todo;
    const prevDone = cache.tasksByStatus.done;

    tasksById.a = makeTask({
      ...tasksById.a,
      title: 'Updated',
      created_at: tasksById.a.created_at,
    });

    const applied = applyTaskDerivationChanges(
      cache,
      ['a'],
      (id) => tasksById[id] ?? null
    );
    expect(applied).toBe(true);
    expect(cache.tasksByStatus.todo).not.toBe(prevTodo);
    expect(cache.tasksByStatus.done).toBe(prevDone);
  });

  it('matches baseline ordering across add/move/remove updates', () => {
    const tasksById: Record<string, TaskWithAttemptStatus> = {
      a: makeTask({
        id: 'a',
        status: 'todo',
        created_at: new Date(1).toISOString(),
      }),
      b: makeTask({
        id: 'b',
        status: 'inreview',
        created_at: new Date(2).toISOString(),
      }),
      c: makeTask({
        id: 'c',
        status: 'done',
        created_at: new Date(3).toISOString(),
      }),
    };

    const cache = buildTaskDerivationCache(Object.values(tasksById));

    // Add a new most-recent task.
    tasksById.d = makeTask({
      id: 'd',
      status: 'todo',
      created_at: new Date(4).toISOString(),
    });
    expect(
      applyTaskDerivationChanges(cache, ['d'], (id) => tasksById[id] ?? null)
    ).toBe(true);

    // Move an existing task across statuses (created_at is stable).
    tasksById.a = makeTask({
      ...tasksById.a,
      status: 'inprogress',
      created_at: tasksById.a.created_at,
    });
    expect(
      applyTaskDerivationChanges(cache, ['a'], (id) => tasksById[id] ?? null)
    ).toBe(true);

    // Remove a task.
    delete tasksById.b;
    expect(
      applyTaskDerivationChanges(cache, ['b'], (id) => tasksById[id] ?? null)
    ).toBe(true);

    const baseline = deriveBaseline(tasksById);
    expect(cache.tasks.map((t) => t.id)).toEqual(
      baseline.tasks.map((t) => t.id)
    );
    expect(cache.tasksByStatus.todo.map((t) => t.id)).toEqual(
      baseline.tasksByStatus.todo.map((t) => t.id)
    );
    expect(cache.tasksByStatus.inprogress.map((t) => t.id)).toEqual(
      baseline.tasksByStatus.inprogress.map((t) => t.id)
    );
    expect(cache.tasksByStatus.inreview.map((t) => t.id)).toEqual(
      baseline.tasksByStatus.inreview.map((t) => t.id)
    );
    expect(cache.tasksByStatus.done.map((t) => t.id)).toEqual(
      baseline.tasksByStatus.done.map((t) => t.id)
    );
  });
});
