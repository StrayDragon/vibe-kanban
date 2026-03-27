import { useMemo } from 'react';
import { useIdMapWsStream } from '@/realtime';
import type { ProjectPublic } from 'shared/types';

export interface UseProjectsResult {
  projects: ProjectPublic[];
  projectsById: Record<string, ProjectPublic>;
  isLoading: boolean;
  isConnected: boolean;
  error: Error | null;
}

export function useProjects(): UseProjectsResult {
  const endpoint = '/api/projects/stream/ws';

  const { data, isConnected, error } = useIdMapWsStream<
    'projects',
    ProjectPublic
  >(endpoint, true, 'projects', '/projects/');

  const projectsById = useMemo(() => data?.projects ?? {}, [data]);

  const projects = useMemo(() => {
    return Object.values(projectsById).sort((a, b) => {
      const byName = a.name.localeCompare(b.name);
      if (byName !== 0) return byName;
      return a.id.localeCompare(b.id);
    });
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
