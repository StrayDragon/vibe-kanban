export const projectKeys = {
  all: ['projects'] as const,
  byId: (projectId: string | undefined) => ['project', projectId] as const,
  repositories: (projectId: string | undefined) =>
    ['projectRepositories', projectId] as const,
  repoScripts: (projectId: string | undefined, repoIds: string[]) =>
    [
      'projectRepoScripts',
      projectId,
      [...repoIds].slice().sort((left, right) => left.localeCompare(right)),
    ] as const,
  latestLifecycleHookOutcome: (projectId: string | undefined) =>
    ['projectLatestLifecycleHook', projectId] as const,
};
