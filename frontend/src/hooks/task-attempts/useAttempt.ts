import { useQuery } from '@tanstack/react-query';
import { attemptsApi } from '@/lib/api';
import type { Workspace } from 'shared/types';
import { taskAttemptKeys } from '@/query-keys/taskAttemptKeys';

export const attemptKeys = {
  byId: (attemptId: string | undefined) => taskAttemptKeys.attempt(attemptId),
};

type Options = {
  enabled?: boolean;
};

export function useAttempt(attemptId?: string, opts?: Options) {
  const enabled = (opts?.enabled ?? true) && !!attemptId;

  return useQuery<Workspace>({
    queryKey: attemptKeys.byId(attemptId),
    queryFn: () => attemptsApi.get(attemptId!),
    enabled,
  });
}
