export function getSeed(): number {
  const raw = process.env.VK_E2E_SEED;
  if (!raw) return 42;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) return 42;
  return Math.floor(parsed);
}

export function getReposDir(): string {
  return process.env.VK_E2E_REPOS_DIR ?? '.e2e/repos';
}

