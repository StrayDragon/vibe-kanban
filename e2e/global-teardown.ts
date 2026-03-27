import fs from 'node:fs/promises';

export default async function globalTeardown(): Promise<void> {
  const runDir = process.env.VK_E2E_RUN_DIR;
  if (!runDir) return;
  await fs.rm(runDir, { recursive: true, force: true });
}

