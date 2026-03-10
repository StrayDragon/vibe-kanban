import { useQuery } from '@tanstack/react-query';
import type { Milestone } from 'shared/types';
import { milestonesApi } from '@/lib/api';

export const milestoneKeys = {
  byId: (milestoneId: string | undefined) => ['milestones', milestoneId],
};

type Options = {
  enabled?: boolean;
};

export function useMilestone(milestoneId?: string, opts?: Options) {
  const enabled = (opts?.enabled ?? true) && !!milestoneId;

  return useQuery<Milestone>({
    queryKey: milestoneKeys.byId(milestoneId),
    queryFn: () => milestonesApi.getById(milestoneId!),
    enabled,
  });
}
