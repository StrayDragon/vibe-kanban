export const taskAttemptKeys = {
  all: ['taskAttempts'] as const,
  byTask: (taskId: string | undefined) => ['taskAttempts', taskId] as const,
  byTaskWithSessions: (taskId: string | undefined) =>
    ['taskAttemptsWithSessions', taskId] as const,

  attempt: (attemptId: string | undefined) =>
    ['taskAttempt', attemptId] as const,
  attemptWithSession: (attemptId: string | undefined) =>
    ['taskAttemptWithSession', attemptId] as const,

  repo: (attemptId: string | undefined) => ['attemptRepo', attemptId] as const,
  repoSelection: (attemptId: string | undefined) =>
    ['attemptRepoSelection', attemptId] as const,

  branch: (attemptId: string | undefined) =>
    ['attemptBranch', attemptId] as const,
};

