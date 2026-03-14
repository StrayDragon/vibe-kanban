export const projectKeys = {
  all: ['projects'] as const,
  byId: (projectId: string | undefined) => ['project', projectId] as const,
  repositories: (projectId: string | undefined) =>
    ['projectRepositories', projectId] as const,
};
