import { describe, expect, it } from 'vitest';
import type { Operation } from 'rfc6902';
import { collectInvalidations } from './eventInvalidation';

describe('collectInvalidations', () => {
  it('collects task and workspace invalidations from workspace patches', () => {
    const patches: Operation[] = [
      {
        op: 'replace',
        path: '/workspaces/workspace-1',
        value: { task_id: 'task-1' },
      },
      {
        op: 'remove',
        path: '/workspaces/workspace-2',
      },
    ];

    const result = collectInvalidations(patches);
    expect(result.taskIds).toEqual(['task-1']);
    expect(result.workspaceIds.sort()).toEqual(['workspace-1', 'workspace-2']);
    expect(result.hasExecutionProcess).toBe(false);
  });

  it('flags execution process invalidations', () => {
    const patches: Operation[] = [
      {
        op: 'add',
        path: '/execution_processes/process-1',
        value: { id: 'process-1' },
      },
    ];

    const result = collectInvalidations(patches);
    expect(result.hasExecutionProcess).toBe(true);
  });
});
