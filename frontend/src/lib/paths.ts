export const paths = {
  projects: () => '/projects',
  overview: () => '/tasks',
  projectTasks: (projectId: string) => `/projects/${projectId}/tasks`,
  task: (projectId: string, taskId: string) =>
    `/projects/${projectId}/tasks/${taskId}`,
  attempt: (projectId: string, taskId: string, attemptId: string) =>
    `/projects/${projectId}/tasks/${taskId}/attempts/${attemptId}`,
  attemptFull: (projectId: string, taskId: string, attemptId: string) =>
    `/projects/${projectId}/tasks/${taskId}/attempts/${attemptId}/full`,
  overviewTask: (projectId: string, taskId: string) =>
    `/tasks/${projectId}/${taskId}`,
  overviewAttempt: (projectId: string, taskId: string, attemptId: string) =>
    `/tasks/${projectId}/${taskId}/attempts/${attemptId}`,
};
