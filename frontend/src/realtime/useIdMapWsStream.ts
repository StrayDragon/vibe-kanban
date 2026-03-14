import { useCallback } from 'react';
import type { Operation } from 'rfc6902';
import { useJsonPatchWsStream } from '@/hooks/useJsonPatchWsStream';
import { normalizeIdMapPatches } from '@/hooks/jsonPatchUtils';

type IdMapState<K extends string, T> = {
  [P in K]: Record<string, T>;
};

/**
 * Realtime primitive for the common “id-map over JSON-Patch WebSocket” pattern.
 * Server sends an initial snapshot (replace) and subsequent add/replace/remove
 * operations under the given `patchPrefix`.
 */
export function useIdMapWsStream<K extends string, T extends object>(
  endpoint: string | undefined,
  enabled: boolean,
  mapKey: K,
  patchPrefix: string
) {
  const initialData = useCallback(
    (): IdMapState<K, T> => ({ [mapKey]: {} } as IdMapState<K, T>),
    [mapKey]
  );

  const deduplicatePatches = useCallback(
    (patches: Operation[], current: IdMapState<K, T> | undefined) =>
      normalizeIdMapPatches(patches, current?.[mapKey], patchPrefix),
    [mapKey, patchPrefix]
  );

  return useJsonPatchWsStream<IdMapState<K, T>>(
    endpoint,
    enabled,
    initialData,
    { deduplicatePatches }
  );
}

