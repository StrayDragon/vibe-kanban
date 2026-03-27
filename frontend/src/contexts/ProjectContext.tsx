import {
  createContext,
  useContext,
  ReactNode,
  useMemo,
  useEffect,
} from 'react';
import { useLocation } from 'react-router-dom';
import type { ProjectPublic } from 'shared/types';
import { useProjects } from '@/hooks/projects/useProjects';

interface ProjectContextValue {
  projectId: string | undefined;
  project: ProjectPublic | undefined;
  projects: ProjectPublic[];
  projectsById: Record<string, ProjectPublic>;
  isLoading: boolean;
  isConnected: boolean;
  error: Error | null;
  isError: boolean;
}

const ProjectContext = createContext<ProjectContextValue | null>(null);

interface ProjectProviderProps {
  children: ReactNode;
}

export function ProjectProvider({ children }: ProjectProviderProps) {
  const location = useLocation();

  // Extract projectId from current route path
  const projectId = useMemo(() => {
    const match = location.pathname.match(/^\/(projects|tasks)\/([^/]+)/);
    return match ? match[2] : undefined;
  }, [location.pathname]);

  const { projects, projectsById, isLoading, isConnected, error } =
    useProjects();
  const project = projectId ? projectsById[projectId] : undefined;

  const value = useMemo(
    () => ({
      projectId,
      project,
      projects,
      projectsById,
      isLoading,
      isConnected,
      error,
      isError: !!error,
    }),
    [projectId, project, projects, projectsById, isLoading, isConnected, error]
  );

  // Centralized page title management
  useEffect(() => {
    if (project) {
      document.title = `${project.name} | vibe-kanban`;
    } else {
      document.title = 'vibe-kanban';
    }
  }, [project]);

  return (
    <ProjectContext.Provider value={value}>{children}</ProjectContext.Provider>
  );
}

export function useProject(): ProjectContextValue {
  const context = useContext(ProjectContext);
  if (!context) {
    throw new Error('useProject must be used within a ProjectProvider');
  }
  return context;
}
