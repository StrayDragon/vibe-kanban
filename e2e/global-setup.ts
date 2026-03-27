import { randomUUID } from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';

function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Missing required env var: ${name}`);
  }
  return value;
}

async function writeJsonAsYaml(filePath: string, value: unknown): Promise<void> {
  // `serde_yaml` accepts JSON as valid YAML, which keeps E2E config generation simple.
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function postJson(url: string, body: unknown): Promise<void> {
  const res = await fetch(url, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });

  if (res.ok) return;
  const text = await res.text().catch(() => '');
  throw new Error(`Request failed (${res.status}) POST ${url}: ${text}`);
}

export default async function globalSetup(): Promise<void> {
  const seed = Number(process.env.VK_E2E_SEED ?? '42');

  const runDir = requireEnv('VK_E2E_RUN_DIR');
  const assetDir = requireEnv('VK_E2E_ASSET_DIR');
  const configDir = requireEnv('VK_E2E_CONFIG_DIR');
  const reposDir = requireEnv('VK_E2E_REPOS_DIR');
  const backendBaseUrl = requireEnv('VK_E2E_BACKEND_BASE_URL');

  // The default run dir is unique per invocation, so we avoid deleting it proactively.
  // (Playwright may start the webServer before globalSetup depending on runner behavior.)
  await fs.mkdir(assetDir, { recursive: true });
  await fs.mkdir(configDir, { recursive: true });
  await fs.mkdir(reposDir, { recursive: true });
  await fs.mkdir(path.join(reposDir, 'worktrees'), { recursive: true });

  await writeJsonAsYaml(path.join(configDir, 'config.yaml'), {
    config_version: 'v10',
    theme: 'LIGHT',
    language: 'EN',
    workspace_dir: reposDir,
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

  // Create a unique parent workspace dir for attempts that use it.
  const defaultWorkspaceDir = path.join(reposDir, `workspace-${randomUUID()}`);
  await fs.mkdir(defaultWorkspaceDir, { recursive: true });

  // Ask the server to reload config if it's already running.
  // If it isn't running yet, it will pick up the files on startup.
  const reloadUrl = `${backendBaseUrl}/api/config/reload`;
  for (let attempt = 0; attempt < 20; attempt += 1) {
    try {
      await postJson(reloadUrl, {});
      return;
    } catch (err) {
      const message = String((err as any)?.message ?? err);
      if (
        message.includes('ECONNREFUSED') ||
        message.includes('fetch failed') ||
        message.includes('ENOTFOUND')
      ) {
        await sleep(250);
        continue;
      }
      throw err;
    }
  }
}
