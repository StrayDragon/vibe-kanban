import os from 'node:os';
import path from 'node:path';

export function getSeed(): number {
  const raw = process.env.VK_E2E_SEED;
  if (!raw) return 42;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) return 42;
  return Math.floor(parsed);
}

function getRunDir(): string {
  return process.env.VK_E2E_RUN_DIR ?? path.join(os.tmpdir(), 'vk-e2e');
}

export function getReposDir(): string {
  return process.env.VK_E2E_REPOS_DIR ?? path.join(getRunDir(), 'repos');
}

export function getConfigDir(): string {
  return process.env.VK_E2E_CONFIG_DIR ?? path.join(getRunDir(), 'config');
}
