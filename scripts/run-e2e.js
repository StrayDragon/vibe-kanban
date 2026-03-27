#!/usr/bin/env node
/* eslint-disable no-console */

const { spawn } = require('child_process');
const fs = require('fs');
const net = require('net');
const path = require('path');

function parsePositiveInt(raw, fallback) {
  if (raw === undefined || raw === null || String(raw).trim() === '') {
    return fallback;
  }
  const value = Number(raw);
  if (!Number.isFinite(value) || value <= 0) return fallback;
  return Math.floor(value);
}

function rmDirIfExists(dirPath) {
  try {
    fs.rmSync(dirPath, { recursive: true, force: true });
  } catch {
    // ignore
  }
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function writeConfigYaml(filePath, value) {
  // `serde_yaml` accepts JSON as valid YAML, which keeps E2E config generation simple.
  writeJson(filePath, value);
}

function spawnLogged(command, args, options) {
  const child = spawn(command, args, options);
  child.on('exit', (code, signal) => {
    if (signal) return;
    if (typeof code === 'number' && code !== 0) {
      console.error(`[e2e] ${command} exited with code ${code}`);
    }
  });
  return child;
}

function waitForExit(child) {
  return new Promise((resolve) => {
    child.on('exit', (code, signal) => resolve({ code, signal }));
  });
}

function isPortAvailable(port) {
  return new Promise((resolve) => {
    const socket = net.createConnection({ port, host: '127.0.0.1' });
    socket.on('connect', () => {
      socket.destroy();
      resolve(false);
    });
    socket.on('error', () => resolve(true));
  });
}

async function findFreePort(startPort) {
  let port = startPort;
  while (!(await isPortAvailable(port))) {
    port += 1;
    if (port > 65535) {
      throw new Error('No available ports found');
    }
  }
  return port;
}

async function waitForHttpOk(url, { timeoutMs, intervalMs }) {
  const deadline = Date.now() + timeoutMs;
  // eslint-disable-next-line no-constant-condition
  while (true) {
    try {
      const res = await fetch(url, { method: 'GET' });
      if (res.ok) return;
    } catch {
      // ignore
    }

    if (Date.now() > deadline) {
      throw new Error(`Timed out waiting for ${url}`);
    }
    await new Promise((r) => setTimeout(r, intervalMs));
  }
}

async function runCommand(command, args, options) {
  const child = spawnLogged(command, args, options);
  const { code, signal } = await waitForExit(child);
  if (signal) {
    throw new Error(`${command} terminated by signal ${signal}`);
  }
  if (code !== 0) {
    throw new Error(`${command} exited with code ${code}`);
  }
}

async function main() {
  const seed = parsePositiveInt(process.env.VK_E2E_SEED, 42);
  const frontendPortOverride = parsePositiveInt(
    process.env.VK_E2E_FRONTEND_PORT,
    0
  );
  const backendPortOverride = parsePositiveInt(
    process.env.VK_E2E_BACKEND_PORT,
    0
  );

  const repoRoot = process.cwd();
  const e2eRoot = path.join(repoRoot, '.e2e');
  const assetDir = path.join(e2eRoot, 'assets');
  const configDir = path.join(e2eRoot, 'config');
  const reposDir = path.join(e2eRoot, 'repos');

  fs.mkdirSync(e2eRoot, { recursive: true });
  rmDirIfExists(assetDir);
  rmDirIfExists(configDir);
  rmDirIfExists(reposDir);
  fs.mkdirSync(assetDir, { recursive: true });
  fs.mkdirSync(reposDir, { recursive: true });
  // Some UI flows expect user-provided parent directories to already exist.
  fs.mkdirSync(path.join(reposDir, 'worktrees'), { recursive: true });

  // Deterministic, non-interactive config for E2E.
  writeConfigYaml(path.join(configDir, 'config.yaml'), {
    config_version: 'v11',
    theme: 'LIGHT',
    language: 'EN',
    executor_profile: { executor: 'FAKE_AGENT' },
    executor_profiles: {
      executors: {
        FAKE_AGENT: {
          DEFAULT: {
            FAKE_AGENT: {
              seed,
              cadence_ms: 0,
              message_chunk_min: 4,
              message_chunk_max: 8,
              tool_events: {
                exec_command: true,
                apply_patch: true,
                mcp: true,
                web_search: false,
                approvals: false,
                errors: false,
              },
              write_fake_files: false,
              include_reasoning: false,
            },
          },
        },
      },
    },
    disclaimer_acknowledged: true,
    onboarding_acknowledged: true,
    show_release_notes: false,
    editor: { editor_type: 'NONE' },
    notifications: {
      sound_enabled: false,
      push_enabled: false,
      sound_file: 'ABSTRACT_SOUND4',
    },
    projects: [],
  });

  const frontendPort =
    frontendPortOverride || (await findFreePort(parsePositiveInt(process.env.FRONTEND_PORT, 3000)));
  const backendPort =
    backendPortOverride || (await findFreePort(Math.max(frontendPort + 1, 3001)));
  const baseUrl = `http://127.0.0.1:${frontendPort}`;

  console.log(`[e2e] seed=${seed}`);
  console.log(`[e2e] asset_dir=${assetDir}`);
  console.log(`[e2e] config_dir=${configDir}`);
  console.log(`[e2e] repos_dir=${reposDir}`);
  console.log(`[e2e] backend=http://127.0.0.1:${backendPort}`);
  console.log(`[e2e] frontend=${baseUrl}`);

  const envCommon = {
    ...process.env,
    VK_E2E_SEED: String(seed),
    VK_E2E_REPOS_DIR: reposDir,
    VK_E2E_CONFIG_DIR: configDir,
    VK_E2E_BASE_URL: baseUrl,
  };

  let backendProc;
  let frontendProc;
  const stopAll = async () => {
    const procs = [frontendProc, backendProc].filter(Boolean);
    for (const proc of procs) {
      try {
        proc.kill('SIGTERM');
      } catch {
        // ignore
      }
    }
  };

  const onSignal = (signal) => {
    console.log(`[e2e] received ${signal}, stopping...`);
    void stopAll().finally(() => process.exit(130));
  };
  process.on('SIGINT', onSignal);
  process.on('SIGTERM', onSignal);

  try {
    // Ensure fake-agent binary exists next to the dev server binary (target/debug).
    await runCommand(
      'cargo',
      ['build', '-p', 'executors', '--bin', 'fake-agent', '--features', 'fake-agent'],
      { stdio: 'inherit' }
    );

    backendProc = spawnLogged(
      'cargo',
      ['run', '-p', 'server', '--bin', 'server', '--features', 'executors/fake-agent'],
      {
        stdio: 'inherit',
        env: {
          ...envCommon,
          VIBE_ASSET_DIR: assetDir,
          VK_CONFIG_DIR: configDir,
          HOST: '127.0.0.1',
          BACKEND_PORT: String(backendPort),
          PORT: String(backendPort),
          VK_OPEN_BROWSER_STARTUP: 'false',
        },
      }
    );

    await waitForHttpOk(`http://127.0.0.1:${backendPort}/health`, {
      timeoutMs: 120_000,
      intervalMs: 250,
    });

    frontendProc = spawnLogged(
      'pnpm',
      [
        'exec',
        'vite',
        '--host',
        '127.0.0.1',
        '--port',
        String(frontendPort),
        '--strictPort',
      ],
      {
      stdio: 'inherit',
      // Vite defaults to binding to all interfaces (and on some machines that
      // means IPv6), which can lead to "port in use" bumps that desync from
      // VK_E2E_BASE_URL. For E2E we always bind to 127.0.0.1 and require the
      // selected port.
      //
      // We intentionally run the frontend dev script directly from the
      // frontend package to avoid inheriting root `frontend:dev` flags.
      cwd: path.join(repoRoot, 'frontend'),
      env: {
        ...envCommon,
        FRONTEND_PORT: String(frontendPort),
        BACKEND_HOST: '127.0.0.1',
        BACKEND_PORT: String(backendPort),
        VITE_OPEN: 'false',
      },
      }
    );

    await waitForHttpOk(`${baseUrl}/`, {
      timeoutMs: 120_000,
      intervalMs: 250,
    });

    const headed = process.env.VK_E2E_HEADED === '1';
    const testScript = headed ? 'e2e:test:headed' : 'e2e:test';
    const testProc = spawnLogged('pnpm', ['run', testScript], {
      stdio: 'inherit',
      env: envCommon,
    });

    const { code, signal } = await waitForExit(testProc);
    if (signal) {
      throw new Error(`Playwright terminated by signal ${signal}`);
    }
    process.exitCode = code ?? 1;
  } finally {
    await stopAll();
  }
}

void main().catch((err) => {
  console.error('[e2e] failed:', err);
  process.exit(1);
});
