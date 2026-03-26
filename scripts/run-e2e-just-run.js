#!/usr/bin/env node
/* eslint-disable no-console */

// E2E runner that exercises the same production-ish "just run" path:
// - builds frontend + backend via `just run`
// - runs the release server (serves frontend + API on one port)
// - executes Playwright suite against the running server

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

function shouldCopySeedAsset(srcPath) {
  const stat = fs.statSync(srcPath);
  if (stat.isDirectory()) {
    return true;
  }

  const lowerName = path.basename(srcPath).toLowerCase();
  return !(
    lowerName.endsWith('.db') ||
    lowerName.endsWith('.sqlite') ||
    lowerName.endsWith('.sqlite3')
  );
}

function spawnLogged(command, args, options) {
  const child = spawn(command, args, options);
  child.on('exit', (code, signal) => {
    if (signal) return;
    if (typeof code === 'number' && code !== 0) {
      console.error(`[e2e:just-run] ${command} exited with code ${code}`);
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

async function main() {
  const seed = parsePositiveInt(process.env.VK_E2E_SEED, 42);
  const serverPortOverride = parsePositiveInt(
    process.env.VK_E2E_SERVER_PORT ?? process.env.VK_E2E_BACKEND_PORT,
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
  fs.mkdirSync(reposDir, { recursive: true });
  fs.mkdirSync(path.join(reposDir, 'worktrees'), { recursive: true });

  const seedAssetsDir = path.join(repoRoot, 'dev_assets_seed');
  if (fs.existsSync(seedAssetsDir)) {
    fs.cpSync(seedAssetsDir, assetDir, {
      recursive: true,
      filter: shouldCopySeedAsset,
    });
  } else {
    fs.mkdirSync(assetDir, { recursive: true });
  }

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

  const serverPort =
    serverPortOverride || (await findFreePort(parsePositiveInt(process.env.PORT, 3001)));
  const baseUrl = `http://127.0.0.1:${serverPort}`;

  console.log(`[e2e:just-run] seed=${seed}`);
  console.log(`[e2e:just-run] asset_dir=${assetDir}`);
  console.log(`[e2e:just-run] config_dir=${configDir}`);
  console.log(`[e2e:just-run] repos_dir=${reposDir}`);
  console.log(`[e2e:just-run] server=${baseUrl}`);

  const envCommon = {
    ...process.env,
    VK_E2E_SEED: String(seed),
    VK_E2E_REPOS_DIR: reposDir,
    VK_E2E_CONFIG_DIR: configDir,
    VK_E2E_BASE_URL: baseUrl,
  };

  let serverProc;
  const stopAll = async () => {
    if (!serverProc) return;
    try {
      serverProc.kill('SIGTERM');
    } catch {
      // ignore
    }
  };

  const onSignal = (signal) => {
    console.log(`[e2e:just-run] received ${signal}, stopping...`);
    void stopAll().finally(() => process.exit(130));
  };
  process.on('SIGINT', onSignal);
  process.on('SIGTERM', onSignal);

  try {
    serverProc = spawnLogged(
      'just',
      ['run', '127.0.0.1', String(serverPort), 'false'],
      {
        stdio: 'inherit',
        env: {
          ...envCommon,
          VIBE_ASSET_DIR: assetDir,
          VK_CONFIG_DIR: configDir,
          VK_ENABLE_FAKE_AGENT: '1',
          HOST: '127.0.0.1',
          PORT: String(serverPort),
        },
      }
    );

    await waitForHttpOk(`${baseUrl}/health`, {
      timeoutMs: 240_000,
      intervalMs: 250,
    });
    await waitForHttpOk(`${baseUrl}/`, {
      timeoutMs: 240_000,
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
  console.error('[e2e:just-run] failed:', err);
  process.exit(1);
});
