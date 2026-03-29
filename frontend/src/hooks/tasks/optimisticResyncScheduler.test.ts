import { describe, expect, it } from 'vitest';
import { nextOptimisticResyncEligibleAtMs } from './optimisticResyncScheduler';

describe('nextOptimisticResyncEligibleAtMs', () => {
  it('returns null when there are no metas', () => {
    const next = nextOptimisticResyncEligibleAtMs([], {
      now: 1000,
      resyncAfterMs: 1200,
      minResyncGapMs: 800,
      maxAttempts: 2,
    });
    expect(next).toBeNull();
  });

  it('skips metas that already reached maxAttempts', () => {
    const next = nextOptimisticResyncEligibleAtMs(
      [
        {
          setAt: 0,
          resyncAttempts: 2,
          lastResyncAt: null,
        },
      ],
      {
        now: 0,
        resyncAfterMs: 1200,
        minResyncGapMs: 800,
        maxAttempts: 2,
      }
    );
    expect(next).toBeNull();
  });

  it('returns setAt + resyncAfterMs when no lastResyncAt exists', () => {
    const next = nextOptimisticResyncEligibleAtMs(
      [
        {
          setAt: 10,
          resyncAttempts: 0,
          lastResyncAt: null,
        },
      ],
      {
        now: 0,
        resyncAfterMs: 1200,
        minResyncGapMs: 800,
        maxAttempts: 2,
      }
    );
    expect(next).toBe(1210);
  });

  it('respects minResyncGapMs when lastResyncAt is more recent', () => {
    const next = nextOptimisticResyncEligibleAtMs(
      [
        {
          setAt: 0,
          resyncAttempts: 0,
          lastResyncAt: 1000,
        },
      ],
      {
        now: 0,
        resyncAfterMs: 1200,
        minResyncGapMs: 800,
        maxAttempts: 2,
      }
    );
    expect(next).toBe(1800);
  });

  it('returns the earliest eligible time across metas', () => {
    const next = nextOptimisticResyncEligibleAtMs(
      [
        {
          setAt: 0,
          resyncAttempts: 0,
          lastResyncAt: null,
        },
        {
          setAt: 500,
          resyncAttempts: 0,
          lastResyncAt: 600,
        },
      ],
      {
        now: 0,
        resyncAfterMs: 1200,
        minResyncGapMs: 800,
        maxAttempts: 2,
      }
    );
    // first eligibleAt = 1200; second eligibleAt = max(1700, 1400) = 1700
    expect(next).toBe(1200);
  });

  it('clamps a past eligible time to now', () => {
    const next = nextOptimisticResyncEligibleAtMs(
      [
        {
          setAt: 0,
          resyncAttempts: 0,
          lastResyncAt: null,
        },
      ],
      {
        now: 5000,
        resyncAfterMs: 1200,
        minResyncGapMs: 800,
        maxAttempts: 2,
      }
    );
    expect(next).toBe(5000);
  });
});
