import type { QueryClient } from '@tanstack/react-query';
import type { Operation } from 'rfc6902';
import { collectInvalidations } from '@/utils/eventInvalidation';
import {
  invalidateQueriesFromHints,
  type InvalidationHints,
} from '@/contexts/eventStreamInvalidation';

type InvalidatableQueryClient = Pick<QueryClient, 'invalidateQueries'>;

export type InvalidationBatcher = {
  enqueueHints: (hints: InvalidationHints) => void;
  enqueueJsonPatch: (patch: Operation[]) => void;
  reset: () => void;
};

export function createInvalidationBatcher(
  queryClient: InvalidatableQueryClient
): InvalidationBatcher {
  const taskIds = new Set<string>();
  const workspaceIds = new Set<string>();
  let hasExecutionProcess = false;
  let flushTimer: ReturnType<typeof setTimeout> | null = null;

  const flush = () => {
    flushTimer = null;

    if (taskIds.size === 0 && workspaceIds.size === 0 && !hasExecutionProcess) {
      return;
    }

    const hints: InvalidationHints = {
      taskIds: Array.from(taskIds),
      workspaceIds: Array.from(workspaceIds),
      hasExecutionProcess,
    };

    taskIds.clear();
    workspaceIds.clear();
    hasExecutionProcess = false;

    invalidateQueriesFromHints(queryClient, hints);
  };

  const scheduleFlush = () => {
    if (flushTimer !== null) return;
    flushTimer = setTimeout(flush, 0);
  };

  const enqueueHints = (hints: InvalidationHints) => {
    let changed = false;

    if (Array.isArray(hints.taskIds)) {
      for (const taskId of hints.taskIds) {
        if (typeof taskId === 'string' && taskId) {
          const before = taskIds.size;
          taskIds.add(taskId);
          if (taskIds.size !== before) {
            changed = true;
          }
        }
      }
    }

    if (Array.isArray(hints.workspaceIds)) {
      for (const workspaceId of hints.workspaceIds) {
        if (typeof workspaceId === 'string' && workspaceId) {
          const before = workspaceIds.size;
          workspaceIds.add(workspaceId);
          if (workspaceIds.size !== before) {
            changed = true;
          }
        }
      }
    }

    if (hints.hasExecutionProcess && !hasExecutionProcess) {
      hasExecutionProcess = true;
      changed = true;
    }

    if (changed) {
      scheduleFlush();
    }
  };

  const enqueueJsonPatch = (patch: Operation[]) => {
    enqueueHints(collectInvalidations(patch));
  };

  const reset = () => {
    if (flushTimer !== null) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
    taskIds.clear();
    workspaceIds.clear();
    hasExecutionProcess = false;
  };

  return { enqueueHints, enqueueJsonPatch, reset };
}
