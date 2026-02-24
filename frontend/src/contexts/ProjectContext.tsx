import {
  createContext,
  useContext,
  ReactNode,
  useMemo,
  useEffect,
} from 'react';
import { useLocation } from 'react-router-dom';
import type { Project } from 'shared/types';
import { useProjects } from '@/hooks/projects/useProjects';

interface ProjectContextValue {
  projectId: string | undefined;
  project: Project | undefined;
  projects: Project[];
  projectsById: Record<string, Project>;
  isLoading: boolean;
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

  const { projects, projectsById, isLoading, error } = useProjects();
  const project = projectId ? projectsById[projectId] : undefined;

  const value = useMemo(
    () => ({
      projectId,
      project,
      projects,
      projectsById,
      isLoading,
      error,
      isError: !!error,
    }),
    [projectId, project, projects, projectsById, isLoading, error]
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
