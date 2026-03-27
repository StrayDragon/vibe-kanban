import { useCallback, useEffect, useState } from 'react';
import { useJsonPatchWsStream } from '../useJsonPatchWsStream';
import { scratchApi } from '@/lib/api';
import { ScratchType, type Scratch, type UpdateScratch } from 'shared/types';

type ScratchState = {
  scratch: Scratch | null;
};

export interface UseScratchResult {
  scratch: Scratch | null;
  isLoading: boolean;
  isConnected: boolean;
  error: string | null;
  updateScratch: (update: UpdateScratch) => Promise<void>;
  deleteScratch: () => Promise<void>;
}

/**
 * Stream a single scratch item via WebSocket (JSON Patch).
 * Server sends the scratch object directly at /scratch.
 */
export const useScratch = (
  scratchType: ScratchType,
  id?: string | null
): UseScratchResult => {
  const enabled = Boolean(id);
  const [connectEnabled, setConnectEnabled] = useState(false);
  const endpoint =
    enabled && id ? scratchApi.getStreamUrl(scratchType, id) : undefined;

  const initialData = useCallback((): ScratchState => ({ scratch: null }), []);

  useEffect(() => {
    setConnectEnabled(false);
    if (!enabled) return;
    const timer = window.setTimeout(() => setConnectEnabled(true), 200);
    return () => window.clearTimeout(timer);
  }, [enabled, id]);

  const { data, isConnected, error } = useJsonPatchWsStream<ScratchState>(
    endpoint,
    connectEnabled,
    initialData
  );

  // Treat deleted scratches as null
  const rawScratch = data?.scratch as (Scratch & { deleted?: boolean }) | null;
  const scratch = rawScratch?.deleted ? null : (rawScratch ?? null);

  const updateScratch = useCallback(
    async (update: UpdateScratch) => {
      if (!id) return;
      await scratchApi.update(scratchType, id, update);
    },
    [scratchType, id]
  );

  const deleteScratch = useCallback(async () => {
    if (!id) return;
    await scratchApi.delete(scratchType, id);
  }, [scratchType, id]);

  // Treat scratch as "loading" until we receive the first snapshot/patch payload.
  // `useJsonPatchWsStream` marks `isConnected` on socket open, which can happen
  // before any patches arrive. Consumers generally need the initial data to
  // avoid UI races (e.g. follow-up drafts being wiped during connect).
  const isLoading = enabled && !data && !error;

  return {
    scratch,
    isLoading,
    isConnected,
    error,
    updateScratch,
    deleteScratch,
  };
};
