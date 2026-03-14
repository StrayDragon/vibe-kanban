import { useMemo } from 'react';
import { useIdMapWsStream } from '@/realtime';
import type { Project } from 'shared/types';

export interface UseProjectsResult {
  projects: Project[];
  projectsById: Record<string, Project>;
  isLoading: boolean;
  isConnected: boolean;
  error: Error | null;
}

export function useProjects(): UseProjectsResult {
  const endpoint = '/api/projects/stream/ws';

  const { data, isConnected, error } = useIdMapWsStream<'projects', Project>(
    endpoint,
    true,
    'projects',
    '/projects/'
  );

  const projectsById = useMemo(() => data?.projects ?? {}, [data]);

  const projects = useMemo(() => {
    return Object.values(projectsById).sort(
      (a, b) =>
        new Date(b.created_at as unknown as string).getTime() -
        new Date(a.created_at as unknown as string).getTime()
    );
  }, [projectsById]);

  const projectsData = data ? projects : undefined;
  const errorObj = useMemo(() => (error ? new Error(error) : null), [error]);

  return {
    projects: projectsData ?? [],
    projectsById,
    isLoading: !data && !error,
    isConnected,
    error: errorObj,
  };
}
