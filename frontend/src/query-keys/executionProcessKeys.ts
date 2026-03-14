export const executionProcessKeys = {
  byAttempt: (attemptId: string | undefined) =>
    ['executionProcesses', attemptId] as const,
  details: (processId: string | undefined) =>
    ['processDetails', processId] as const,
};

