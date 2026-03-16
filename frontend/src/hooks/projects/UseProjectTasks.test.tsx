import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { TaskWithAttemptStatus } from 'shared/types';
import { useOptimisticTasksStore } from '@/stores/useOptimisticTasksStore';

import { useProjectTasks } from './useProjectTasks';

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
    title: '测试',
    description: "直接输出一个'hi'",
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

describe('useProjectTasks optimistic overrides', () => {
  beforeEach(() => {
    useOptimisticTasksStore.getState().reset();
    streamTasks = {};
  });

  it('clears an optimistic status override when the stream progresses beyond it', async () => {
    const projectId = 'project-1';
    const taskId = 'task-1';

    useOptimisticTasksStore.getState().setOverride(taskId, {
      status: 'inprogress',
    });
    const setAt =
      useOptimisticTasksStore.getState().overrides[taskId].meta.setAt;

    streamTasks = {
      [taskId]: makeTask({
        id: taskId,
        project_id: projectId,
        status: 'inreview',
        updated_at: new Date(setAt + 1_000).toISOString(),
      }),
    };

    const { result } = renderHook(() => useProjectTasks(projectId));

    await act(async () => {});

    expect(
      useOptimisticTasksStore.getState().overrides[taskId]
    ).toBeUndefined();
    expect(result.current.tasksById[taskId].status).toBe('inreview');
  });

  it('keeps an optimistic status override when the stream is behind it', async () => {
    const projectId = 'project-1';
    const taskId = 'task-1';

    useOptimisticTasksStore.getState().setOverride(taskId, {
      status: 'inprogress',
    });
    const setAt =
      useOptimisticTasksStore.getState().overrides[taskId].meta.setAt;

    streamTasks = {
      [taskId]: makeTask({
        id: taskId,
        project_id: projectId,
        status: 'todo',
        updated_at: new Date(setAt + 1_000).toISOString(),
      }),
    };

    const { result } = renderHook(() => useProjectTasks(projectId));

    await act(async () => {});

    expect(useOptimisticTasksStore.getState().overrides[taskId]).toBeDefined();
    expect(result.current.tasksById[taskId].status).toBe('inprogress');
  });

  it('keeps an optimistic backward status override until the server updates', async () => {
    const projectId = 'project-1';
    const taskId = 'task-1';

    useOptimisticTasksStore.getState().setOverride(taskId, {
      status: 'todo',
    });
    const setAt =
      useOptimisticTasksStore.getState().overrides[taskId].meta.setAt;

    streamTasks = {
      [taskId]: makeTask({
        id: taskId,
        project_id: projectId,
        status: 'inprogress',
        updated_at: new Date(setAt - 60_000).toISOString(),
      }),
    };

    const { result } = renderHook(() => useProjectTasks(projectId));

    await act(async () => {});

    expect(useOptimisticTasksStore.getState().overrides[taskId]).toBeDefined();
    expect(result.current.tasksById[taskId].status).toBe('todo');
  });
});
