#!/usr/bin/env node
/* eslint-disable no-console */

const fs = require('fs');
const path = require('path');

const REPO_ROOT = path.resolve(__dirname, '..');
const OUTPUT_PATH = path.join(REPO_ROOT, 'docs', 'env.gen.md');

const ENV_VARS = [
  {
    category: 'Server',
    name: 'HOST',
    type: 'string',
    defaultValue: '127.0.0.1',
    appliesTo: ['server', 'mcp_task_server'],
    description: 'Bind address for the backend HTTP server.',
  },
  {
    category: 'Server',
    name: 'BACKEND_PORT',
    type: 'int',
    defaultValue: '0 (auto-assign if unset)',
    appliesTo: ['server', 'mcp_task_server'],
    description:
      'Preferred port for the backend HTTP server. Falls back to PORT when unset.',
  },
  {
    category: 'Server',
    name: 'PORT',
    type: 'int',
    defaultValue: '0 (auto-assign if unset)',
    appliesTo: ['server', 'mcp_task_server'],
    description: 'Fallback alias for BACKEND_PORT.',
  },
  {
    category: 'Server',
    name: 'RUST_LOG',
    type: 'string',
    defaultValue: 'info',
    appliesTo: ['server'],
    description:
      'Controls module log levels. Used to build the tracing filter for the backend.',
  },
  {
    category: 'Storage',
    name: 'VIBE_ASSET_DIR',
    type: 'path',
    defaultValue: 'dev_assets/ (debug) or OS app data dir (release)',
    appliesTo: ['server', 'local-deployment'],
    description:
      'Overrides the asset directory used for config, credentials, and the SQLite database.',
  },
  {
    category: 'Storage',
    name: 'DATABASE_URL',
    type: 'string',
    defaultValue: 'sqlite://<asset_dir>/db.sqlite?mode=rwc',
    appliesTo: ['server'],
    description:
      'SQLite database URL. Only sqlite URLs are supported. When unset, defaults under VIBE_ASSET_DIR.',
  },
  {
    category: 'Storage',
    name: 'VIBE_DB_RESET_ON_MIGRATION_ERROR',
    type: 'bool',
    defaultValue: 'false',
    appliesTo: ['server'],
    description:
      'When true, resets the local SQLite DB files on migration error (destructive).',
  },

  {
    category: 'Workspace cleanup',
    name: 'VK_WORKSPACE_EXPIRED_TTL_SECS',
    type: 'duration-secs',
    defaultValue: '259200 (72h)',
    appliesTo: ['local-deployment'],
    description:
      'TTL threshold for expired workspaces. Workspaces older than this and without running processes are eligible for cleanup.',
  },
  {
    category: 'Workspace cleanup',
    name: 'VK_WORKSPACE_CLEANUP_INTERVAL_SECS',
    type: 'duration-secs',
    defaultValue: '1800 (30m)',
    appliesTo: ['local-deployment'],
    description: 'Tick interval for the periodic workspace cleanup loop.',
  },
  {
    category: 'Workspace cleanup',
    name: 'DISABLE_WORKSPACE_EXPIRED_CLEANUP',
    type: 'bool (presence)',
    defaultValue: 'unset',
    appliesTo: ['local-deployment'],
    description:
      'When set, disables TTL-based expired workspace cleanup (orphan cleanup remains separately controlled).',
  },
  {
    category: 'Workspace cleanup',
    name: 'DISABLE_WORKTREE_ORPHAN_CLEANUP',
    type: 'bool (presence)',
    defaultValue: 'unset',
    appliesTo: ['local-deployment'],
    description: 'When set, disables orphan workspace cleanup on disk.',
  },

  {
    category: 'Idempotency',
    name: 'VK_IDEMPOTENCY_IN_PROGRESS_TTL_SECS',
    type: 'duration-secs',
    defaultValue: '3600 (1h)',
    appliesTo: ['server'],
    description:
      'TTL for stale in-progress idempotency keys. Set to 0 to disable stale cleanup.',
  },
  {
    category: 'Idempotency',
    name: 'VK_IDEMPOTENCY_COMPLETED_TTL_SECS',
    type: 'duration-secs',
    defaultValue: '604800 (7d)',
    appliesTo: ['server'],
    description:
      'TTL for completed idempotency keys. Set to 0 to disable pruning.',
  },

  {
    category: 'Logs',
    name: 'VK_LOG_HISTORY_MAX_BYTES',
    type: 'int',
    defaultValue: '8388608 (8 MiB)',
    appliesTo: ['server'],
    description:
      'Maximum in-memory log history size per MsgStore (bytes). Values of 0 are normalized to 1.',
  },
  {
    category: 'Logs',
    name: 'VK_LOG_HISTORY_MAX_ENTRIES',
    type: 'int',
    defaultValue: '5000',
    appliesTo: ['server'],
    description:
      'Maximum in-memory log history entries per MsgStore. Values of 0 are normalized to 1.',
  },
  {
    category: 'Logs',
    name: 'VK_NORMALIZED_LOG_HISTORY_PAGE_SIZE',
    type: 'int',
    defaultValue: '20',
    appliesTo: ['server'],
    description: 'Default page size for normalized log history v2 endpoints.',
  },
  {
    category: 'Logs',
    name: 'VK_RAW_LOG_HISTORY_PAGE_SIZE',
    type: 'int',
    defaultValue: '200',
    appliesTo: ['server'],
    description: 'Default page size for raw log history v2 endpoints.',
  },
  {
    category: 'Logs',
    name: 'VK_LOG_PERSISTENCE_MODE',
    type: 'string',
    defaultValue: 'auto',
    appliesTo: ['server'],
    description:
      "Controls log persistence backend ('auto' | 'log_entries' | 'legacy_jsonl').",
  },
  {
    category: 'Logs',
    name: 'VK_LOG_BACKFILL_CONCURRENCY',
    type: 'int',
    defaultValue: '4',
    appliesTo: ['server'],
    description: 'Concurrency for log backfill jobs.',
  },
  {
    category: 'Logs',
    name: 'VK_LOG_BACKFILL_COMPLETION_MAX_ENTRIES',
    type: 'int',
    defaultValue: '10000',
    appliesTo: ['server'],
    description:
      'In-memory cache size for log backfill completion tracking (entries).',
  },
  {
    category: 'Logs',
    name: 'VK_LOG_BACKFILL_COMPLETION_TTL_SECS',
    type: 'duration-secs',
    defaultValue: '86400 (24h)',
    appliesTo: ['server'],
    description: 'TTL for log backfill completion tracking cache.',
  },
  {
    category: 'Logs',
    name: 'VK_LEGACY_JSONL_RETENTION_DAYS',
    type: 'int',
    defaultValue: '14',
    appliesTo: ['server'],
    description:
      'Retention window for legacy JSONL logs. Values <= 0 disable cleanup.',
  },

  {
    category: 'Cache budgets',
    name: 'VK_FILE_SEARCH_CACHE_MAX_REPOS',
    type: 'int',
    defaultValue: '25',
    appliesTo: ['server'],
    description: 'Maximum repos tracked in the file search cache.',
  },
  {
    category: 'Cache budgets',
    name: 'VK_FILE_SEARCH_CACHE_TTL_SECS',
    type: 'duration-secs',
    defaultValue: '3600 (1h)',
    appliesTo: ['server'],
    description: 'TTL for file search cache entries.',
  },
  {
    category: 'Cache budgets',
    name: 'VK_FILE_SEARCH_MAX_FILES',
    type: 'int',
    defaultValue: '200000',
    appliesTo: ['server'],
    description: 'Maximum file count considered during file search indexing.',
  },
  {
    category: 'Cache budgets',
    name: 'VK_FILE_SEARCH_WATCHERS_MAX',
    type: 'int',
    defaultValue: '25',
    appliesTo: ['server'],
    description: 'Maximum active file watchers.',
  },
  {
    category: 'Cache budgets',
    name: 'VK_FILE_SEARCH_WATCHER_TTL_SECS',
    type: 'duration-secs',
    defaultValue: '21600 (6h)',
    appliesTo: ['server'],
    description: 'TTL for file watcher entries.',
  },
  {
    category: 'Cache budgets',
    name: 'VK_FILE_STATS_CACHE_MAX_REPOS',
    type: 'int',
    defaultValue: '25',
    appliesTo: ['server'],
    description: 'Maximum repos tracked in the file stats cache.',
  },
  {
    category: 'Cache budgets',
    name: 'VK_FILE_STATS_CACHE_TTL_SECS',
    type: 'duration-secs',
    defaultValue: '3600 (1h)',
    appliesTo: ['server'],
    description: 'TTL for file stats cache entries.',
  },
  {
    category: 'Cache budgets',
    name: 'VK_APPROVALS_COMPLETED_TTL_SECS',
    type: 'duration-secs',
    defaultValue: '86400 (24h)',
    appliesTo: ['server'],
    description: 'TTL for completed approvals cache.',
  },
  {
    category: 'Cache budgets',
    name: 'VK_QUEUED_MESSAGES_TTL_SECS',
    type: 'duration-secs',
    defaultValue: '86400 (24h)',
    appliesTo: ['server'],
    description: 'TTL for queued messages.',
  },
  {
    category: 'Cache budgets',
    name: 'VK_CACHE_WARN_AT_RATIO',
    type: 'float',
    defaultValue: '0.9',
    appliesTo: ['server'],
    description:
      'Warn when cache usage reaches this ratio of the configured maximum.',
  },
  {
    category: 'Cache budgets',
    name: 'VK_CACHE_WARN_SAMPLE_SECS',
    type: 'duration-secs',
    defaultValue: '300 (5m)',
    appliesTo: ['server'],
    description: 'Minimum interval between repeated cache warnings.',
  },

  {
    category: 'Build',
    name: 'POSTHOG_API_KEY',
    type: 'string',
    defaultValue: 'unset',
    appliesTo: ['build'],
    description: 'PostHog API key (build-time, embedded into the server binary).',
  },
  {
    category: 'Build',
    name: 'POSTHOG_API_ENDPOINT',
    type: 'string',
    defaultValue: 'unset',
    appliesTo: ['build'],
    description:
      'PostHog API endpoint (build-time, embedded into the server binary).',
  },
  {
    category: 'Build',
    name: 'VK_SHARED_API_BASE',
    type: 'string',
    defaultValue: 'unset',
    appliesTo: ['build'],
    description:
      'Optional shared API base URL embedded at build time (used by the server build script).',
  },

  {
    category: 'Translation',
    name: 'KANBAN_OPENAI_API_BASE',
    type: 'string',
    defaultValue: 'unset',
    appliesTo: ['server'],
    description: 'OpenAI-compatible base URL for translation.',
  },
  {
    category: 'Translation',
    name: 'KANBAN_OPENAI_API_KEY',
    type: 'string',
    defaultValue: 'unset',
    appliesTo: ['server'],
    description: 'API key for translation.',
  },
  {
    category: 'Translation',
    name: 'KANBAN_OPENAI_DEFAULT_MODEL',
    type: 'string',
    defaultValue: 'unset',
    appliesTo: ['server'],
    description: 'Default model name for translation.',
  },
  {
    category: 'Translation',
    name: 'OPENAI_API_BASE',
    type: 'string',
    defaultValue: 'unset',
    appliesTo: ['server'],
    description:
      'Fallback translation base URL (used only when KANBAN_OPENAI_API_BASE is unset).',
  },
  {
    category: 'Translation',
    name: 'OPENAI_API_KEY',
    type: 'string',
    defaultValue: 'unset',
    appliesTo: ['server'],
    description:
      'Fallback translation API key (used only when KANBAN_OPENAI_API_KEY is unset).',
  },
  {
    category: 'Translation',
    name: 'OPENAI_DEFAULT_MODEL',
    type: 'string',
    defaultValue: 'unset',
    appliesTo: ['server'],
    description:
      'Fallback translation default model (used only when KANBAN_OPENAI_DEFAULT_MODEL is unset).',
  },

  {
    category: 'MCP',
    name: 'VIBE_BACKEND_URL',
    type: 'string',
    defaultValue: 'unset',
    appliesTo: ['mcp_task_server'],
    description:
      'Backend base URL for the MCP task server (overrides HOST/BACKEND_PORT/port file discovery).',
  },

  {
    category: 'Dev frontend',
    name: 'FRONTEND_PORT',
    type: 'int',
    defaultValue: '3000',
    appliesTo: ['dev', 'frontend'],
    description: 'Port used by `pnpm run frontend:dev` and `pnpm run dev`.',
  },
  {
    category: 'Dev frontend',
    name: 'BACKEND_HOST',
    type: 'string',
    defaultValue: '127.0.0.1',
    appliesTo: ['frontend'],
    description:
      'Host for the Vite dev proxy to reach the backend (used with BACKEND_PORT).',
  },
  {
    category: 'Dev frontend',
    name: 'VITE_OPEN',
    type: 'bool',
    defaultValue: 'false',
    appliesTo: ['frontend'],
    description: 'When true, Vite dev server opens a browser window.',
  },
  {
    category: 'Dev frontend',
    name: 'VITE_SOURCEMAP',
    type: 'bool',
    defaultValue: 'false',
    appliesTo: ['frontend'],
    description: 'When true, Vite builds/generates source maps.',
  },
  {
    category: 'Dev frontend',
    name: 'VITE_PARENT_ORIGIN',
    type: 'string',
    defaultValue: 'unset',
    appliesTo: ['frontend'],
    description:
      'Optional parent origin used by the embedded VS Code bridge integration.',
  },

  {
    category: 'Fake agent',
    name: 'VIBE_FAKE_AGENT_PATH',
    type: 'path',
    defaultValue: 'unset',
    appliesTo: ['executors'],
    description:
      'Path override for the fake-agent executable used for local reproducible runs.',
  },
  {
    category: 'Fake agent',
    name: 'VIBE_FAKE_AGENT_CONFIG',
    type: 'path',
    defaultValue: 'unset',
    appliesTo: ['executors'],
    description: 'Path to a fake-agent scenario/config file.',
  },

  {
    category: 'Executor env (set by Vibe Kanban)',
    name: 'VK_PROJECT_NAME',
    type: 'string',
    defaultValue: 'dynamic',
    appliesTo: ['executors'],
    description:
      'Injected into executor processes to identify the project name.',
  },
  {
    category: 'Executor env (set by Vibe Kanban)',
    name: 'VK_PROJECT_ID',
    type: 'uuid',
    defaultValue: 'dynamic',
    appliesTo: ['executors'],
    description: 'Injected into executor processes to identify the project id.',
  },
  {
    category: 'Executor env (set by Vibe Kanban)',
    name: 'VK_TASK_ID',
    type: 'uuid',
    defaultValue: 'dynamic',
    appliesTo: ['executors'],
    description: 'Injected into executor processes to identify the task id.',
  },
  {
    category: 'Executor env (set by Vibe Kanban)',
    name: 'VK_WORKSPACE_ID',
    type: 'uuid',
    defaultValue: 'dynamic',
    appliesTo: ['executors'],
    description: 'Injected into executor processes to identify the workspace id.',
  },
  {
    category: 'Executor env (set by Vibe Kanban)',
    name: 'VK_WORKSPACE_BRANCH',
    type: 'string',
    defaultValue: 'dynamic',
    appliesTo: ['executors'],
    description:
      'Injected into executor processes to identify the workspace branch name.',
  },
];

function usage() {
  console.error('Usage: node scripts/generate-env-docs.js [--check]');
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function listFiles(dir) {
  const results = [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      const rel = path.relative(REPO_ROOT, full);
      const parts = rel.split(path.sep);
      if (
        parts.includes('node_modules') ||
        parts.includes('.git') ||
        parts.includes('target') ||
        rel.startsWith(`frontend${path.sep}dist`) ||
        rel.startsWith(`dev_assets${path.sep}`) ||
        rel.startsWith(`dev_assets_seed${path.sep}`)
      ) {
        continue;
      }
      results.push(...listFiles(full));
      continue;
    }

    const ext = path.extname(entry.name);
    if (
      ext === '.rs' ||
      ext === '.js' ||
      ext === '.ts' ||
      ext === '.tsx' ||
      entry.name === 'package.json' ||
      entry.name === 'justfile'
    ) {
      results.push(full);
    }
  }
  return results;
}

function buildSourcesIndex(names) {
  const roots = [
    path.join(REPO_ROOT, 'crates'),
    path.join(REPO_ROOT, 'scripts'),
    path.join(REPO_ROOT, 'frontend'),
    path.join(REPO_ROOT, 'package.json'),
    path.join(REPO_ROOT, 'justfile'),
  ].filter((p) => fs.existsSync(p));

  const files = [];
  for (const root of roots) {
    const stat = fs.statSync(root);
    if (stat.isDirectory()) {
      files.push(...listFiles(root));
    } else {
      files.push(root);
    }
  }

  const sources = new Map();
  for (const name of names) {
    sources.set(name, new Set());
  }

  const regexes = new Map();
  for (const name of names) {
    regexes.set(name, new RegExp(`\\b${escapeRegExp(name)}\\b`));
  }

  for (const file of files) {
    if (path.resolve(file) === path.resolve(__filename)) {
      continue;
    }

    let text;
    try {
      text = fs.readFileSync(file, 'utf8');
    } catch {
      continue;
    }

    for (const name of names) {
      const re = regexes.get(name);
      if (re && re.test(text)) {
        sources.get(name).add(path.relative(REPO_ROOT, file));
      }
    }
  }

  return sources;
}

function groupByCategory(vars) {
  const groups = new Map();
  for (const v of vars) {
    if (!groups.has(v.category)) groups.set(v.category, []);
    groups.get(v.category).push(v);
  }
  for (const [category, items] of groups) {
    items.sort((a, b) => a.name.localeCompare(b.name));
    groups.set(category, items);
  }
  return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b));
}

function renderMarkdown(vars, sourcesIndex) {
  const lines = [];
  lines.push('# Environment Variables (Generated)');
  lines.push('');
  lines.push(
    'This file is generated by `pnpm run generate-env-docs`. Do not edit it by hand.'
  );
  lines.push('');
  lines.push('Notes:');
  lines.push(
    '- Defaults reflect code defaults; some values are only used when the env var is set.'
  );
  lines.push(
    '- Lowering workspace TTL can delete uncommitted worktree changes; use with care.'
  );
  lines.push('');

  for (const [category, items] of groupByCategory(vars)) {
    lines.push(`## ${category}`);
    lines.push('');
    lines.push('| Name | Type | Default | Applies to | Description |');
    lines.push('| --- | --- | --- | --- | --- |');
    for (const v of items) {
      const applies = v.appliesTo.join(', ');
      const def = v.defaultValue ?? '—';
      lines.push(
        `| \`${v.name}\` | ${v.type} | ${def} | ${applies} | ${v.description} |`
      );
    }
    lines.push('');
  }

  lines.push('## Sources');
  lines.push('');
  lines.push(
    'The list below shows where each env var name appears in the repo (best-effort).'
  );
  lines.push('');

  const sorted = [...vars].sort((a, b) => a.name.localeCompare(b.name));
  for (const v of sorted) {
    const sources = [...(sourcesIndex.get(v.name) ?? new Set())].sort();
    const sourcesStr = sources.length ? sources.map((s) => `\`${s}\``).join(', ') : '—';
    lines.push(`- \`${v.name}\`: ${sourcesStr}`);
  }
  lines.push('');
  return lines.join('\n');
}

function main() {
  const args = process.argv.slice(2);
  const check = args.includes('--check');
  if (args.length > 0 && !check) {
    usage();
    process.exit(2);
  }

  const names = ENV_VARS.map((v) => v.name);
  const sourcesIndex = buildSourcesIndex(names);
  const markdown = renderMarkdown(ENV_VARS, sourcesIndex);

  if (check) {
    const current = fs.existsSync(OUTPUT_PATH)
      ? fs.readFileSync(OUTPUT_PATH, 'utf8')
      : null;
    if (current !== markdown) {
      console.error('docs/env.gen.md is out of date. Run: pnpm run generate-env-docs');
      process.exit(1);
    }
    return;
  }

  fs.mkdirSync(path.dirname(OUTPUT_PATH), { recursive: true });
  fs.writeFileSync(OUTPUT_PATH, markdown);
}

main();

