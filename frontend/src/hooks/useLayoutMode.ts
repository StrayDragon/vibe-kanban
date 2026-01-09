import { useCallback, useEffect } from 'react';
import type { SetURLSearchParams } from 'react-router-dom';
import type { LayoutMode } from '@/components/layout/TasksLayout';

export function useLayoutMode(
  searchParams: URLSearchParams,
  setSearchParams: SetURLSearchParams
) {
  const rawMode = searchParams.get('view');
  const mode: LayoutMode =
    rawMode === 'preview' || rawMode === 'diffs' ? rawMode : null;

  // TODO: Remove this redirect after v0.1.0 (legacy URL support for bookmarked links)
  // Migrates old `view=logs` to `view=diffs`
  useEffect(() => {
    if (rawMode !== 'logs') return;
    const params = new URLSearchParams(searchParams);
    params.set('view', 'diffs');
    setSearchParams(params, { replace: true });
  }, [rawMode, searchParams, setSearchParams]);

  const setMode = useCallback(
    (newMode: LayoutMode) => {
      const params = new URLSearchParams(searchParams);
      if (newMode === null) {
        params.delete('view');
      } else {
        params.set('view', newMode);
      }
      setSearchParams(params, { replace: true });
    },
    [searchParams, setSearchParams]
  );

  return { mode, setMode };
}
