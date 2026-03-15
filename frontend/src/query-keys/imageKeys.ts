export const imageKeys = {
  taskImages: (taskId: string | undefined) => ['taskImages', taskId] as const,
  metadata: (params: {
    taskAttemptId: string | undefined;
    taskId: string | undefined;
    src: string;
  }) =>
    ['imageMetadata', params.taskAttemptId, params.taskId, params.src] as const,
};
