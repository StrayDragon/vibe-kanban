export type OptimisticResyncMeta = {
  setAt: number;
  resyncAttempts: number;
  lastResyncAt: number | null;
};

export type OptimisticResyncScheduleOptions = {
  now: number;
  resyncAfterMs: number;
  minResyncGapMs: number;
  maxAttempts: number;
};

export const nextOptimisticResyncEligibleAtMs = (
  metas: OptimisticResyncMeta[],
  options: OptimisticResyncScheduleOptions
): number | null => {
  let next: number | null = null;
  for (const meta of metas) {
    if (meta.resyncAttempts >= options.maxAttempts) continue;

    let eligibleAt = meta.setAt + options.resyncAfterMs;
    if (meta.lastResyncAt !== null) {
      eligibleAt = Math.max(
        eligibleAt,
        meta.lastResyncAt + options.minResyncGapMs
      );
    }

    if (next === null || eligibleAt < next) {
      next = eligibleAt;
    }
  }

  if (next === null) return null;
  return Math.max(next, options.now);
};
