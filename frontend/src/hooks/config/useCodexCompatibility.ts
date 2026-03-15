import { useCallback, useEffect, useState } from 'react';

import type { BaseCodingAgent, CodexProtocolCompatibility } from 'shared/types';
import { configApi } from '@/lib/api';

export function useCodexCompatibility(
  variant: string | null | undefined,
  enabled: boolean = true
): {
  compatibility: CodexProtocolCompatibility | null;
  isChecking: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  check: (refresh: boolean) => Promise<void>;
} {
  const [compatibility, setCompatibility] =
    useState<CodexProtocolCompatibility | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const check = useCallback(
    async (refresh: boolean) => {
      if (!enabled) {
        return;
      }
      setIsChecking(true);
      setError(null);
      try {
        const compat = await configApi.checkAgentCompatibility(
          'CODEX' as BaseCodingAgent,
          variant ?? null,
          refresh
        );
        setCompatibility(compat);
      } catch (err) {
        console.error('Failed to check Codex compatibility:', err);
        setCompatibility(null);
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setIsChecking(false);
      }
    },
    [variant, enabled]
  );

  useEffect(() => {
    if (!enabled) {
      setCompatibility(null);
      setError(null);
      setIsChecking(false);
      return;
    }

    check(false);
  }, [check, enabled]);

  return {
    compatibility,
    isChecking,
    error,
    refresh: () => check(true),
    check,
  };
}
