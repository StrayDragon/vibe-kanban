import { lazy, Suspense, useEffect, type ReactNode } from 'react';
import { Navigate, Route, Routes, useParams } from 'react-router-dom';

import { ThemeProvider } from '@/components/ThemeProvider';
import { useUserSystem } from '@/components/ConfigProvider';
import { Loader } from '@/components/ui/loader';
import { NormalLayout } from '@/components/layout/NormalLayout';
import { usePreviousPath } from '@/hooks/utils/usePreviousPath';
import { SearchProvider } from '@/contexts/SearchContext';
import { ThemeMode } from 'shared/types';

import { DisclaimerDialog } from '@/components/dialogs/global/DisclaimerDialog';

const TasksOverview = lazy(() =>
  import('@/pages/TasksOverview').then((mod) => ({
    default: mod.TasksOverview,
  }))
);
const ProjectTasks = lazy(() =>
  import('@/pages/ProjectTasks').then((mod) => ({ default: mod.ProjectTasks }))
);
const ProjectArchives = lazy(() =>
  import('@/pages/ProjectArchives').then((mod) => ({
    default: mod.ProjectArchives,
  }))
);
const ProjectArchiveDetail = lazy(() =>
  import('@/pages/ProjectArchiveDetail').then((mod) => ({
    default: mod.ProjectArchiveDetail,
  }))
);
const MilestoneWorkflow = lazy(() =>
  import('@/pages/MilestoneWorkflow').then((mod) => ({
    default: mod.MilestoneWorkflow,
  }))
);
const FullAttemptLogsPage = lazy(() =>
  import('@/pages/FullAttemptLogs').then((mod) => ({
    default: mod.FullAttemptLogsPage,
  }))
);
const NotFoundPage = lazy(() =>
  import('@/pages/NotFoundPage').then((mod) => ({ default: mod.NotFoundPage }))
);

const SettingsLayout = lazy(() =>
  import('@/pages/settings/SettingsLayout').then((mod) => ({
    default: mod.SettingsLayout,
  }))
);
const GeneralSettings = lazy(() =>
  import('@/pages/settings/GeneralSettings').then((mod) => ({
    default: mod.GeneralSettings,
  }))
);
const ProjectSettings = lazy(() =>
  import('@/pages/settings/ProjectSettings').then((mod) => ({
    default: mod.ProjectSettings,
  }))
);
const AgentSettings = lazy(() =>
  import('@/pages/settings/AgentSettings').then((mod) => ({
    default: mod.AgentSettings,
  }))
);
const McpSettings = lazy(() =>
  import('@/pages/settings/McpSettings').then((mod) => ({
    default: mod.McpSettings,
  }))
);

function RouteSuspense({ children }: { children: ReactNode }) {
  return (
    <Suspense
      fallback={
        <div className="min-h-[60vh] flex items-center justify-center">
          <Loader message="Loading..." size={32} />
        </div>
      }
    >
      {children}
    </Suspense>
  );
}

export function AppRouter() {
  const { config, loading } = useUserSystem();

  usePreviousPath();

  useEffect(() => {
    if (!config) return;
    let cancelled = false;

    const showDisclaimer = async () => {
      const storageKey = 'vk.disclaimer_acknowledged';
      const acknowledged = localStorage.getItem(storageKey) === '1';
      if (acknowledged) return;

      await DisclaimerDialog.show();
      if (!cancelled) {
        localStorage.setItem(storageKey, '1');
      }
      DisclaimerDialog.hide();
    };

    void showDisclaimer();

    return () => {
      cancelled = true;
    };
  }, [config]);

  if (loading) {
    return (
      <div className="min-h-screen bg-background flex items-center justify-center">
        <Loader message="Loading..." size={32} />
      </div>
    );
  }

  return (
    <ThemeProvider initialTheme={config?.theme || ThemeMode.SYSTEM}>
      <SearchProvider>
        <div className="h-screen flex flex-col bg-background">
          <Routes>
            <Route
              path="/projects/:projectId/tasks/:taskId/attempts/:attemptId/full"
              element={
                <RouteSuspense>
                  <FullAttemptLogsPage />
                </RouteSuspense>
              }
            />

            <Route element={<NormalLayout />}>
              <Route path="/" element={<Navigate to="/tasks" replace />} />
              <Route
                path="/projects"
                element={<Navigate to="/tasks" replace />}
              />
              <Route
                path="/projects/:projectId"
                element={<ProjectRedirect />}
              />
              <Route
                path="/tasks"
                element={
                  <RouteSuspense>
                    <TasksOverview />
                  </RouteSuspense>
                }
              />
              <Route
                path="/tasks/:projectId/:taskId"
                element={
                  <RouteSuspense>
                    <TasksOverview />
                  </RouteSuspense>
                }
              />
              <Route
                path="/tasks/:projectId/:taskId/attempts/:attemptId"
                element={
                  <RouteSuspense>
                    <TasksOverview />
                  </RouteSuspense>
                }
              />
              <Route
                path="/projects/:projectId/tasks"
                element={
                  <RouteSuspense>
                    <ProjectTasks />
                  </RouteSuspense>
                }
              />
              <Route
                path="/projects/:projectId/archives"
                element={
                  <RouteSuspense>
                    <ProjectArchives />
                  </RouteSuspense>
                }
              />
              <Route
                path="/projects/:projectId/archives/:archiveId"
                element={
                  <RouteSuspense>
                    <ProjectArchiveDetail />
                  </RouteSuspense>
                }
              />
              <Route
                path="/settings/*"
                element={
                  <RouteSuspense>
                    <SettingsLayout />
                  </RouteSuspense>
                }
              >
                <Route index element={<Navigate to="general" replace />} />
                <Route
                  path="general"
                  element={
                    <RouteSuspense>
                      <GeneralSettings />
                    </RouteSuspense>
                  }
                />
                <Route
                  path="projects"
                  element={
                    <RouteSuspense>
                      <ProjectSettings />
                    </RouteSuspense>
                  }
                />
                <Route
                  path="agents"
                  element={
                    <RouteSuspense>
                      <AgentSettings />
                    </RouteSuspense>
                  }
                />
                <Route
                  path="mcp"
                  element={
                    <RouteSuspense>
                      <McpSettings />
                    </RouteSuspense>
                  }
                />
              </Route>
              <Route
                path="/mcp-servers"
                element={<Navigate to="/settings/mcp" replace />}
              />
              <Route
                path="/projects/:projectId/tasks/:taskId"
                element={
                  <RouteSuspense>
                    <ProjectTasks />
                  </RouteSuspense>
                }
              />
              <Route
                path="/projects/:projectId/tasks/:taskId/attempts/:attemptId"
                element={
                  <RouteSuspense>
                    <ProjectTasks />
                  </RouteSuspense>
                }
              />
              <Route
                path="/projects/:projectId/milestones/:milestoneId"
                element={
                  <RouteSuspense>
                    <MilestoneWorkflow />
                  </RouteSuspense>
                }
              />
              <Route
                path="*"
                element={
                  <RouteSuspense>
                    <NotFoundPage />
                  </RouteSuspense>
                }
              />
            </Route>
          </Routes>
        </div>
      </SearchProvider>
    </ThemeProvider>
  );
}

function ProjectRedirect() {
  const { projectId } = useParams<{ projectId?: string }>();

  if (!projectId) {
    return <Navigate to="/tasks" replace />;
  }

  return <Navigate to={`/projects/${projectId}/tasks`} replace />;
}
