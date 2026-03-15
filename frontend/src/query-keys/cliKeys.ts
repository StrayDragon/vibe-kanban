export const cliKeys = {
  dependencyPreflight: (agent: string | null | undefined) =>
    ['cli-preflight', agent] as const,
};
