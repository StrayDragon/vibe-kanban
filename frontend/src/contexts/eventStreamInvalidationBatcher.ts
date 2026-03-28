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

const MAX_UNIQUE_IDS_PER_BATCH = 512;

type FlushHandle =
  | { kind: 'timeout'; id: ReturnType<typeof setTimeout> }
  | { kind: 'raf'; id: number };

export function createInvalidationBatcher(
  queryClient: InvalidatableQueryClient
): InvalidationBatcher {
  const taskIds = new Set<string>();
  const workspaceIds = new Set<string>();
  let hasExecutionProcess = false;
  let flushHandle: FlushHandle | null = null;

  const flush = () => {
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
    if (flushHandle !== null) return;

    const canUseRaf =
      typeof document !== 'undefined' &&
      document.visibilityState === 'visible' &&
      typeof requestAnimationFrame === 'function';

    if (canUseRaf) {
      const id = requestAnimationFrame(() => {
        flushHandle = null;
        flush();
      });
      flushHandle = { kind: 'raf', id };
      return;
    }

    const id = setTimeout(() => {
      flushHandle = null;
      flush();
    }, 0);
    flushHandle = { kind: 'timeout', id };
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

    if (taskIds.size + workspaceIds.size > MAX_UNIQUE_IDS_PER_BATCH) {
      reset();
      queryClient.invalidateQueries();
      return;
    }

    if (changed) {
      scheduleFlush();
    }
  };

  const enqueueJsonPatch = (patch: Operation[]) => {
    enqueueHints(collectInvalidations(patch));
  };

  const reset = () => {
    if (flushHandle?.kind === 'timeout') {
      clearTimeout(flushHandle.id);
      flushHandle = null;
    } else if (flushHandle?.kind === 'raf') {
      if (typeof cancelAnimationFrame === 'function') {
        cancelAnimationFrame(flushHandle.id);
      }
      flushHandle = null;
    }
    taskIds.clear();
    workspaceIds.clear();
    hasExecutionProcess = false;
  };

  return { enqueueHints, enqueueJsonPatch, reset };
}
