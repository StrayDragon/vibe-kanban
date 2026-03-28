import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Operation } from 'rfc6902';
import { createInvalidationBatcher } from './eventStreamInvalidationBatcher';
import { branchStatusKeys } from '@/hooks/task-attempts/useBranchStatus';
import { taskAttemptKeys } from '@/query-keys/taskAttemptKeys';

describe('createInvalidationBatcher', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('batches and deduplicates invalidate hints within a tick', async () => {
    vi.useFakeTimers();

    const invalidateQueries = vi.fn();
    const queryClient = { invalidateQueries } as const;
    const batcher = createInvalidationBatcher(queryClient);

    batcher.enqueueHints({
      taskIds: ['task-1'],
      workspaceIds: ['workspace-1'],
      hasExecutionProcess: false,
    });
    batcher.enqueueHints({
      taskIds: ['task-1', 'task-2'],
      workspaceIds: ['workspace-1', 'workspace-2'],
      hasExecutionProcess: true,
    });

    expect(invalidateQueries).not.toHaveBeenCalled();

    await vi.runAllTimersAsync();

    expect(invalidateQueries).toHaveBeenCalledTimes(11);

    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: taskAttemptKeys.byTask('task-1'),
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: taskAttemptKeys.byTaskWithSessions('task-1'),
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: taskAttemptKeys.byTask('task-2'),
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: taskAttemptKeys.byTaskWithSessions('task-2'),
    });

    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: branchStatusKeys.byAttempt('workspace-1'),
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: taskAttemptKeys.attempt('workspace-1'),
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: taskAttemptKeys.attemptWithSession('workspace-1'),
    });

    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: branchStatusKeys.byAttempt('workspace-2'),
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: taskAttemptKeys.attempt('workspace-2'),
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: taskAttemptKeys.attemptWithSession('workspace-2'),
    });

    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: branchStatusKeys.all,
    });
  });

  it('cancels pending invalidations when reset is called', async () => {
    vi.useFakeTimers();

    const invalidateQueries = vi.fn();
    const queryClient = { invalidateQueries } as const;
    const batcher = createInvalidationBatcher(queryClient);

    batcher.enqueueHints({ taskIds: ['task-1'] });
    batcher.reset();

    await vi.runAllTimersAsync();

    expect(invalidateQueries).not.toHaveBeenCalled();
  });

  it('accepts json_patch fallback and enqueues derived invalidations', async () => {
    vi.useFakeTimers();

    const invalidateQueries = vi.fn();
    const queryClient = { invalidateQueries } as const;
    const batcher = createInvalidationBatcher(queryClient);

    const patch: Operation[] = [
      {
        op: 'replace',
        path: '/workspaces/workspace-1',
        value: { task_id: 'task-1' },
      },
    ];

    batcher.enqueueJsonPatch(patch);
    await vi.runAllTimersAsync();

    expect(invalidateQueries).toHaveBeenCalledTimes(5);
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: taskAttemptKeys.byTask('task-1'),
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: taskAttemptKeys.byTaskWithSessions('task-1'),
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: branchStatusKeys.byAttempt('workspace-1'),
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: taskAttemptKeys.attempt('workspace-1'),
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: taskAttemptKeys.attemptWithSession('workspace-1'),
    });
  });
});
