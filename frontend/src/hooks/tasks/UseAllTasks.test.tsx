import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { TaskWithAttemptStatus } from 'shared/types';
import { useOptimisticTasksStore } from '@/stores/useOptimisticTasksStore';

import { useAllTasks } from './useAllTasks';

let streamTasks: Record<string, TaskWithAttemptStatus> = {};

vi.mock('../useJsonPatchWsStream', () => ({
  useJsonPatchWsStream: () => ({
    data: { tasks: streamTasks },
    isConnected: true,
    isResyncing: false,
    error: null,
    resync: vi.fn(),
  }),
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

describe('useAllTasks optimistic overrides', () => {
  beforeEach(() => {
    useOptimisticTasksStore.getState().reset();
    streamTasks = {};
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
