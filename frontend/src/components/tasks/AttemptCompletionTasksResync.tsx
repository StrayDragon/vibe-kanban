import { useEffect, useMemo, useRef } from 'react';

import type { TaskStatus } from 'shared/types';
import { useAttemptExecution } from '@/hooks';

export function AttemptCompletionTasksResync({
  attemptId,
  taskId,
  taskStatus,
  resyncTasks,
}: {
  attemptId: string | undefined;
  taskId: string | undefined;
  taskStatus: TaskStatus | undefined;
  resyncTasks: () => void;
}) {
  const { processes } = useAttemptExecution(attemptId, taskId);
  const prevProcessRef = useRef<{ id: string | null; status: string | null }>({
    id: null,
    status: null,
  });
  const resyncedForProcessRef = useRef<string | null>(null);

  const latestCodingAgent = useMemo(() => {
    const codingAgentProcesses = processes.filter(
      (process) => process.run_reason === 'codingagent'
    );
    if (codingAgentProcesses.length === 0) return null;
    return [...codingAgentProcesses].sort(
      (a, b) => Date.parse(b.created_at) - Date.parse(a.created_at)
    )[0];
  }, [processes]);

  useEffect(() => {
    prevProcessRef.current = { id: null, status: null };
    resyncedForProcessRef.current = null;
  }, [attemptId]);

  useEffect(() => {
    if (!latestCodingAgent?.id || !latestCodingAgent?.status) return;

    const { id, status } = latestCodingAgent;
    const prev = prevProcessRef.current;
    prevProcessRef.current = { id, status };

    if (taskStatus !== 'inprogress') return;
    if (status === 'running') return;
    if (resyncedForProcessRef.current === id) return;

    const transitionedFromRunning = prev.id === id && prev.status === 'running';
    const shouldResync = transitionedFromRunning || prev.id !== id;
    if (!shouldResync) return;

    resyncedForProcessRef.current = id;
    const timer = window.setTimeout(() => resyncTasks(), 250);
    return () => window.clearTimeout(timer);
  }, [latestCodingAgent, resyncTasks, taskStatus]);

  return null;
}
