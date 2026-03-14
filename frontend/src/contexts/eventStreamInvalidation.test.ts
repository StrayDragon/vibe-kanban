import { describe, expect, it, vi } from 'vitest';
import type { Operation } from 'rfc6902';
import { invalidateQueriesFromJsonPatch } from './eventStreamInvalidation';
import { branchStatusKeys } from '@/hooks/task-attempts/useBranchStatus';
import { taskAttemptKeys } from '@/query-keys/taskAttemptKeys';

describe('invalidateQueriesFromJsonPatch', () => {
  it('invalidates attempt and branch status queries for workspace updates', () => {
    const invalidateQueries = vi.fn();
    const queryClient = { invalidateQueries } as const;

    const patches: Operation[] = [
      {
        op: 'replace',
        path: '/workspaces/workspace-1',
        value: { task_id: 'task-1' },
      },
      {
        op: 'add',
        path: '/execution_processes/process-1',
        value: { id: 'process-1' },
      },
    ];

    invalidateQueriesFromJsonPatch(queryClient, patches);

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
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: branchStatusKeys.all,
    });
  });
});
