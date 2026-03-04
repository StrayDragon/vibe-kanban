export const paths = {
  projects: () => '/projects',
  overview: () => '/tasks',
  projectTasks: (projectId: string) => `/projects/${projectId}/tasks`,
  projectArchives: (projectId: string) => `/projects/${projectId}/archives`,
  projectArchive: (projectId: string, archiveId: string) =>
    `/projects/${projectId}/archives/${archiveId}`,
  task: (projectId: string, taskId: string) =>
    `/projects/${projectId}/tasks/${taskId}`,
  attempt: (projectId: string, taskId: string, attemptId: string) =>
    `/projects/${projectId}/tasks/${taskId}/attempts/${attemptId}`,
  taskGroupWorkflow: (projectId: string, taskGroupId: string) =>
    `/projects/${projectId}/task-groups/${taskGroupId}`,
  attemptFull: (projectId: string, taskId: string, attemptId: string) =>
    `/projects/${projectId}/tasks/${taskId}/attempts/${attemptId}/full`,
  overviewTask: (projectId: string, taskId: string) =>
    `/tasks/${projectId}/${taskId}`,
  overviewAttempt: (projectId: string, taskId: string, attemptId: string) =>
    `/tasks/${projectId}/${taskId}/attempts/${attemptId}`,
};
