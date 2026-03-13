import { useCallback } from 'react';
import type { SetURLSearchParams } from 'react-router-dom';
import type { LayoutMode } from '@/components/layout/TasksLayout';

export function useLayoutMode(
  searchParams: URLSearchParams,
  setSearchParams: SetURLSearchParams
) {
  const rawMode = searchParams.get('view');
  const mode: LayoutMode =
    rawMode === 'preview' || rawMode === 'diffs' ? rawMode : null;

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
