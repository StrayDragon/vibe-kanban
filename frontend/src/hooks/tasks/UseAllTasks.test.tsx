import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { TaskWithAttemptStatus } from 'shared/types';
import { useOptimisticTasksStore } from '@/stores/useOptimisticTasksStore';

import { useAllTasks } from './useAllTasks';

let streamTasks: Record<string, TaskWithAttemptStatus> = {};
let onInvalidateCb:
  | ((invalidate: unknown, meta: { seq: number | null }) => void)
  | null = null;

vi.mock('../useJsonPatchWsStream', () => ({
  useJsonPatchWsStream: (
    _endpoint: string | undefined,
    _enabled: boolean,
    _initialData: () => unknown,
    options?: {
      onInvalidate?: (
        invalidate: unknown,
        meta: { seq: number | null }
      ) => void;
    }
  ) => {
    onInvalidateCb = options?.onInvalidate ?? null;
    return {
      data: { tasks: streamTasks },
      isConnected: true,
      isResyncing: false,
      error: null,
      resync: vi.fn(),
    };
  },
}));

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

const emitInvalidate = (taskIds: string[]) => {
  onInvalidateCb?.({ taskIds }, { seq: 1 });
};

describe('useAllTasks optimistic overrides', () => {
  beforeEach(() => {
    useOptimisticTasksStore.getState().reset();
    streamTasks = {};
    onInvalidateCb = null;
  });

  it('clears an optimistic status override when the stream progresses beyond it', async () => {
    const taskId = 'task-1';

    useOptimisticTasksStore
      .getState()
      .setOverride(taskId, { status: 'inprogress' }, { baseUpdatedAtMs: 0 });

    streamTasks = {
      [taskId]: makeTask({
        id: taskId,
        status: 'inreview',
        updated_at: new Date(1_000).toISOString(),
      }),
    };

    const { result } = renderHook(() => useAllTasks());

    await act(async () => {});

    expect(
      useOptimisticTasksStore.getState().overrides[taskId]
    ).toBeUndefined();
    expect(result.current.tasksById[taskId].status).toBe('inreview');
  });

  it('keeps an optimistic status override when the stream is behind it', async () => {
    const taskId = 'task-1';

    useOptimisticTasksStore
      .getState()
      .setOverride(taskId, { status: 'inprogress' }, { baseUpdatedAtMs: 0 });

    streamTasks = {
      [taskId]: makeTask({
        id: taskId,
        status: 'todo',
        updated_at: new Date(1_000).toISOString(),
      }),
    };

    const { result } = renderHook(() => useAllTasks());

    await act(async () => {});

    expect(useOptimisticTasksStore.getState().overrides[taskId]).toBeDefined();
    expect(result.current.tasksById[taskId].status).toBe('inprogress');
  });
});

describe('useAllTasks derivation cache', () => {
  beforeEach(() => {
    useOptimisticTasksStore.getState().reset();
    streamTasks = {};
    onInvalidateCb = null;
  });

  it('keeps per-status array references stable for unaffected statuses', async () => {
    streamTasks = {
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

    const { result, rerender } = renderHook(() => useAllTasks());
    await act(async () => {});

    const prevTodo = result.current.tasksByStatus.todo;
    const prevDone = result.current.tasksByStatus.done;

    streamTasks = {
      ...streamTasks,
      a: makeTask({
        id: 'a',
        status: 'todo',
        title: 'Updated',
        created_at: streamTasks.a.created_at,
      }),
    };
    emitInvalidate(['a']);
    rerender();
    await act(async () => {});

    expect(result.current.tasksByStatus.todo).not.toBe(prevTodo);
    expect(result.current.tasksByStatus.done).toBe(prevDone);

    const baseline = deriveBaseline(streamTasks);
    expect(result.current.tasks.map((t) => t.id)).toEqual(
      baseline.tasks.map((t) => t.id)
    );
    expect(result.current.tasksByStatus.todo.map((t) => t.id)).toEqual(
      baseline.tasksByStatus.todo.map((t) => t.id)
    );
    expect(result.current.tasksByStatus.done.map((t) => t.id)).toEqual(
      baseline.tasksByStatus.done.map((t) => t.id)
    );
  });

  it('matches baseline ordering across add/move/remove updates', async () => {
    streamTasks = {
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

    const { result, rerender } = renderHook(() => useAllTasks());
    await act(async () => {});

    // Add a new task (most recent).
    streamTasks = {
      ...streamTasks,
      d: makeTask({
        id: 'd',
        status: 'todo',
        created_at: new Date(4).toISOString(),
      }),
    };
    emitInvalidate(['d']);
    rerender();
    await act(async () => {});

    // Move an existing task across statuses.
    streamTasks = {
      ...streamTasks,
      a: makeTask({
        id: 'a',
        status: 'inprogress',
        created_at: streamTasks.a.created_at,
      }),
    };
    emitInvalidate(['a']);
    rerender();
    await act(async () => {});

    // Remove a task.
    const { b: removed, ...rest } = streamTasks;
    void removed;
    streamTasks = rest;
    emitInvalidate(['b']);
    rerender();
    await act(async () => {});

    const baseline = deriveBaseline(streamTasks);
    expect(result.current.tasks.map((t) => t.id)).toEqual(
      baseline.tasks.map((t) => t.id)
    );
    expect(result.current.tasksByStatus.todo.map((t) => t.id)).toEqual(
      baseline.tasksByStatus.todo.map((t) => t.id)
    );
    expect(result.current.tasksByStatus.inprogress.map((t) => t.id)).toEqual(
      baseline.tasksByStatus.inprogress.map((t) => t.id)
    );
    expect(result.current.tasksByStatus.inreview.map((t) => t.id)).toEqual(
      baseline.tasksByStatus.inreview.map((t) => t.id)
    );
    expect(result.current.tasksByStatus.done.map((t) => t.id)).toEqual(
      baseline.tasksByStatus.done.map((t) => t.id)
    );
  });
});
