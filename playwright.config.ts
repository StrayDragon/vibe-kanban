import { randomUUID } from 'node:crypto';
import os from 'node:os';
import path from 'node:path';
import { defineConfig, devices } from '@playwright/test';

function parseWorkers(raw: string | undefined): number | undefined {
  if (!raw) return undefined;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) return undefined;
  return Math.floor(parsed);
}

function parsePositiveInt(raw: string | undefined, fallback: number): number {
  if (!raw || raw.trim() === '') return fallback;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) return fallback;
  return Math.floor(parsed);
}

function portSeedFromRunId(id: string): number {
  const compact = id.replaceAll('-', '');
  const head = compact.slice(0, 8);
  const parsed = Number.parseInt(head, 16);
  if (Number.isFinite(parsed)) return parsed;
  return Math.floor(Math.random() * 10_000);
}

type E2EMode = 'dev' | 'just-run';

function getE2EMode(): E2EMode {
  const raw = process.env.VK_E2E_MODE?.trim();
  if (!raw || raw === 'dev') return 'dev';
  if (raw === 'just-run') return 'just-run';
  throw new Error(`Invalid VK_E2E_MODE: ${raw} (expected 'dev' or 'just-run')`);
}

const e2eMode = getE2EMode();
const runId = process.env.VK_E2E_RUN_ID ?? randomUUID();
const runDir =
  process.env.VK_E2E_RUN_DIR ?? path.join(os.tmpdir(), `vk-e2e-${runId}`);

const assetDir = path.join(runDir, 'assets');
const configDir = path.join(runDir, 'config');
const reposDir = path.join(runDir, 'repos');
const databaseUrl = `sqlite://${path.join(assetDir, 'db.sqlite')}?mode=rwc`;

const portSeed = portSeedFromRunId(runId);
const portBase = 31_000;
const portSpan = 10_000;
const defaultFrontendPort = portBase + (portSeed % portSpan);
const defaultBackendPort = portBase + ((portSeed + 1) % portSpan);
const defaultServerPort = portBase + ((portSeed + 2) % portSpan);

const frontendPort = parsePositiveInt(
  process.env.VK_E2E_FRONTEND_PORT ?? process.env.FRONTEND_PORT,
  defaultFrontendPort
);
const backendPort = parsePositiveInt(
  process.env.VK_E2E_BACKEND_PORT ?? process.env.BACKEND_PORT,
  defaultBackendPort
);
const serverPort = parsePositiveInt(
  process.env.VK_E2E_SERVER_PORT ?? process.env.PORT,
  defaultServerPort
);

const baseUrl =
  e2eMode === 'just-run'
    ? `http://127.0.0.1:${serverPort}`
    : `http://127.0.0.1:${frontendPort}`;
const backendBaseUrl =
  e2eMode === 'just-run'
    ? baseUrl
    : `http://127.0.0.1:${backendPort}`;

process.env.VK_E2E_MODE = e2eMode;
process.env.VK_E2E_RUN_ID = runId;
process.env.VK_E2E_RUN_DIR = runDir;
process.env.VK_E2E_ASSET_DIR = assetDir;
process.env.VK_E2E_CONFIG_DIR = configDir;
process.env.VK_E2E_REPOS_DIR = reposDir;
process.env.VK_E2E_BASE_URL = baseUrl;
process.env.VK_E2E_BACKEND_BASE_URL = backendBaseUrl;
process.env.VK_E2E_SEED ??= '42';

const defaultWebServerTimeout = e2eMode === 'just-run' ? 600_000 : 240_000;
const webServerTimeout = parsePositiveInt(
  process.env.VK_E2E_SERVER_TIMEOUT,
  defaultWebServerTimeout
);

export default defineConfig({
  testDir: './e2e',
  testMatch: '**/*.spec.ts',
  globalSetup: './e2e/global-setup',
  globalTeardown: './e2e/global-teardown',
  webServer:
    e2eMode === 'just-run'
      ? {
          name: 'Server',
          command: `just run 127.0.0.1 ${serverPort} false`,
          url: `${baseUrl}/health`,
          timeout: webServerTimeout,
          reuseExistingServer: false,
          gracefulShutdown: { signal: 'SIGTERM', timeout: 5_000 },
          env: {
            ...process.env,
            VK_ENABLE_FAKE_AGENT: '1',
            VIBE_ASSET_DIR: assetDir,
            VK_CONFIG_DIR: configDir,
            DATABASE_URL: databaseUrl,
            HOST: '127.0.0.1',
            PORT: String(serverPort),
          },
        }
      : [
          {
            name: 'Backend',
            command:
              'cargo build -p executors --bin fake-agent --features fake-agent && ' +
              'cargo run -p server --bin server --features executors/fake-agent',
            url: `${backendBaseUrl}/health`,
            timeout: webServerTimeout,
            reuseExistingServer: false,
            gracefulShutdown: { signal: 'SIGTERM', timeout: 5_000 },
            env: {
              ...process.env,
              VIBE_ASSET_DIR: assetDir,
              VK_CONFIG_DIR: configDir,
              DATABASE_URL: databaseUrl,
              HOST: '127.0.0.1',
              BACKEND_PORT: String(backendPort),
              PORT: String(backendPort),
              VK_OPEN_BROWSER_STARTUP: 'false',
            },
          },
          {
            name: 'Frontend',
            cwd: 'frontend',
            command:
              `pnpm exec vite --host 127.0.0.1 --port ${frontendPort} --strictPort`,
            url: `${baseUrl}/`,
            timeout: webServerTimeout,
            reuseExistingServer: false,
            gracefulShutdown: { signal: 'SIGTERM', timeout: 5_000 },
            env: {
              ...process.env,
              FRONTEND_PORT: String(frontendPort),
              BACKEND_HOST: '127.0.0.1',
              BACKEND_PORT: String(backendPort),
              VITE_OPEN: 'false',
            },
          },
        ],
  forbidOnly: !!process.env.CI,
  retries: 0,
  // E2E suite shares a single dev server + sqlite db by default, so parallelism can
  // introduce flakiness unless tests are written to fully isolate state.
  workers: parseWorkers(process.env.VK_E2E_WORKERS) ?? (process.env.CI ? 1 : 1),
  reporter: process.env.CI
    ? [['list'], ['html', { open: 'never' }]]
    : [['list']],
  use: {
    baseURL: baseUrl,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
