import type { Operation } from 'rfc6902';

export type PatchInvalidations = {
  taskIds: string[];
  workspaceIds: string[];
  hasExecutionProcess: boolean;
};

const decodePointerSegment = (segment: string) =>
  segment.replace(/~1/g, '/').replace(/~0/g, '~');

const splitPath = (path: string): string[] =>
  path.split('/').filter(Boolean).map(decodePointerSegment);

const extractTaskId = (value: unknown): string | undefined => {
  if (!value || typeof value !== 'object') return undefined;
  const record = value as Record<string, unknown>;
  const taskId = record.task_id;
  return typeof taskId === 'string' ? taskId : undefined;
};

export const collectInvalidations = (
  patches: Operation[]
): PatchInvalidations => {
  const taskIds = new Set<string>();
  const workspaceIds = new Set<string>();
  let hasExecutionProcess = false;

  for (const op of patches) {
    const path = typeof op.path === 'string' ? op.path : '';
    if (!path) continue;

    const segments = splitPath(path);
    if (segments.length === 0) continue;

    if (segments[0] === 'workspaces' && segments[1]) {
      workspaceIds.add(segments[1]);
      if (op.op === 'add' || op.op === 'replace') {
        const taskId = extractTaskId((op as { value?: unknown }).value);
        if (taskId) {
          taskIds.add(taskId);
        }
      }
      continue;
    }

    if (segments[0] === 'execution_processes') {
      hasExecutionProcess = true;
    }
  }

  return {
    taskIds: Array.from(taskIds),
    workspaceIds: Array.from(workspaceIds),
    hasExecutionProcess,
  };
};
