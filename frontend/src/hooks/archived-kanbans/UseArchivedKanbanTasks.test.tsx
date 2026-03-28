import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { TaskWithAttemptStatus } from 'shared/types';

import { useArchivedKanbanTasks } from './useArchivedKanbanTasks';

let streamTasks: Record<string, TaskWithAttemptStatus> = {};
let onInvalidateCb:
  | ((invalidate: unknown, meta: { seq: number | null }) => void)
  | null = null;

vi.mock('@/hooks/useJsonPatchWsStream', () => ({
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
    archived_kanban_id: 'archive',
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

describe('useArchivedKanbanTasks derivation cache', () => {
  beforeEach(() => {
    streamTasks = {};
    onInvalidateCb = null;
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
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

    const { result, rerender } = renderHook(() =>
      useArchivedKanbanTasks('archive')
    );
    await act(async () => {
      vi.runAllTimers();
    });

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
    await act(async () => {
      vi.runAllTimers();
    });

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
});
