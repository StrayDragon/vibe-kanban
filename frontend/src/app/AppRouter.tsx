import { useEffect } from 'react';
import { Navigate, Route, Routes, useParams } from 'react-router-dom';

import { ThemeProvider } from '@/components/ThemeProvider';
import { useUserSystem } from '@/components/ConfigProvider';
import { Loader } from '@/components/ui/loader';
import { NormalLayout } from '@/components/layout/NormalLayout';
import { ProjectTasks } from '@/pages/ProjectTasks';
import { TaskGroupWorkflow } from '@/pages/TaskGroupWorkflow';
import { TasksOverview } from '@/pages/TasksOverview';
import { FullAttemptLogsPage } from '@/pages/FullAttemptLogs';
import { usePreviousPath } from '@/hooks/utils/usePreviousPath';
import { SearchProvider } from '@/contexts/SearchContext';
import { ThemeMode } from 'shared/types';

import { DisclaimerDialog } from '@/components/dialogs/global/DisclaimerDialog';
import { OnboardingDialog } from '@/components/dialogs/global/OnboardingDialog';
import { ReleaseNotesDialog } from '@/components/dialogs/global/ReleaseNotesDialog';
import {
  AgentSettings,
  GeneralSettings,
  McpSettings,
  ProjectSettings,
  SettingsLayout,
} from '@/pages/settings/';

export function AppRouter() {
  const { config, updateAndSaveConfig, loading } = useUserSystem();

  usePreviousPath();

  useEffect(() => {
    if (!config) return;
    let cancelled = false;

    const showNextStep = async () => {
      if (!config.disclaimer_acknowledged) {
        await DisclaimerDialog.show();
        if (!cancelled) {
          await updateAndSaveConfig({ disclaimer_acknowledged: true });
        }
        DisclaimerDialog.hide();
        return;
      }

      if (!config.onboarding_acknowledged) {
        const result = await OnboardingDialog.show();
        if (!cancelled) {
          await updateAndSaveConfig({
            onboarding_acknowledged: true,
            executor_profile: result.profile,
            editor: result.editor,
          });
        }
        OnboardingDialog.hide();
        return;
      }

      if (config.show_release_notes) {
        await ReleaseNotesDialog.show();
        if (!cancelled) {
          await updateAndSaveConfig({ show_release_notes: false });
        }
        ReleaseNotesDialog.hide();
        return;
      }
    };

    showNextStep();

    return () => {
      cancelled = true;
    };
  }, [config, updateAndSaveConfig]);

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
              element={<FullAttemptLogsPage />}
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
              <Route path="/tasks" element={<TasksOverview />} />
              <Route
                path="/tasks/:projectId/:taskId"
                element={<TasksOverview />}
              />
              <Route
                path="/tasks/:projectId/:taskId/attempts/:attemptId"
                element={<TasksOverview />}
              />
              <Route
                path="/projects/:projectId/tasks"
                element={<ProjectTasks />}
              />
              <Route path="/settings/*" element={<SettingsLayout />}>
                <Route index element={<Navigate to="general" replace />} />
                <Route path="general" element={<GeneralSettings />} />
                <Route path="projects" element={<ProjectSettings />} />
                <Route path="agents" element={<AgentSettings />} />
                <Route path="mcp" element={<McpSettings />} />
              </Route>
              <Route
                path="/mcp-servers"
                element={<Navigate to="/settings/mcp" replace />}
              />
              <Route
                path="/projects/:projectId/tasks/:taskId"
                element={<ProjectTasks />}
              />
              <Route
                path="/projects/:projectId/tasks/:taskId/attempts/:attemptId"
                element={<ProjectTasks />}
              />
              <Route
                path="/projects/:projectId/task-groups/:taskGroupId"
                element={<TaskGroupWorkflow />}
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
