#!/usr/bin/env node
/**
 * External link guardrail for "product surfaces".
 *
 * Policy:
 * - Ban specific upstream-hosted domains that cause drift/confusion.
 * - Allow only explicit upstream GitHub links in user-facing docs/UI.
 *
 * Notes:
 * - This intentionally does NOT scan OpenSpec artifacts (they may reference
 *   historical upstream details).
 * - This intentionally excludes dependency manifests (Cargo.toml, Cargo.lock,
 *   package manifests) because they legitimately reference external sources.
 */

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const REPO_ROOT = process.cwd();

const BANNED_SUBSTRINGS = [
  'vibekanban.com',
  'api.vibekanban.com',
  'review.fast',
];

const ALLOWED_URL_PREFIXES_USER_FACING = [
  'https://github.com/BloopAI/vibe-kanban',
  // SVG namespace (not a navigational link).
  'http://www.w3.org/2000/svg',
  'http://localhost',
  'http://127.0.0.1',
  'http://[::1]',
];

const USER_FACING_PATH_PREFIXES = [
  'frontend/src/',
  'docs/',
  'e2e/',
];

const USER_FACING_FILES = new Set(['README.md', 'ARCH.md', 'NOTICE']);

const EXCLUDED_PATH_PREFIXES = [
  '.git/',
  'node_modules/',
  'target/',
  'openspec/',
  'test-results/',
];

const EXCLUDED_FILES = new Set([
  'LICENSE',
  'Cargo.lock',
  'pnpm-lock.yaml',
  // Guardrail scripts necessarily reference the banned domains/policy.
  'scripts/check-external-links.js',
  'scripts/check-doc-links.js',
]);

const EXCLUDED_EXTS = new Set([
  '.png',
  '.jpg',
  '.jpeg',
  '.gif',
  '.webp',
  '.svg',
  '.ico',
  '.pdf',
  '.zip',
  '.7z',
  '.gz',
  '.tar',
  '.woff',
  '.woff2',
  '.ttf',
  '.eot',
  '.mp4',
  '.mov',
  '.mp3',
]);

function listTrackedFiles() {
  const out = execFileSync('git', ['ls-files', '-z'], { encoding: 'utf8' });
  return out
    .split('\0')
    .map((p) => p.trim())
    .filter(Boolean);
}

function isExcludedFile(p) {
  if (EXCLUDED_FILES.has(p)) return true;
  for (const prefix of EXCLUDED_PATH_PREFIXES) {
    if (p.startsWith(prefix)) return true;
  }
  const ext = path.extname(p);
  if (ext && EXCLUDED_EXTS.has(ext)) return true;
  return false;
}

function isUserFacingFile(p) {
  if (USER_FACING_FILES.has(p)) return true;
  return USER_FACING_PATH_PREFIXES.some((prefix) => p.startsWith(prefix));
}

function isTextFile(p) {
  const ext = path.extname(p);
  if (!ext) return true;
  if (EXCLUDED_EXTS.has(ext)) return false;
  return true;
}

function findAllOccurrences(haystack, needle) {
  const result = [];
  let idx = haystack.indexOf(needle);
  while (idx !== -1) {
    result.push(idx);
    idx = haystack.indexOf(needle, idx + needle.length);
  }
  return result;
}

function offsetToLineCol(text, offset) {
  // 1-based line/column
  let line = 1;
  let col = 1;
  for (let i = 0; i < offset; i++) {
    if (text.charCodeAt(i) === 10) {
      line++;
      col = 1;
    } else {
      col++;
    }
  }
  return { line, col };
}

function extractHttpUrls(text) {
  // Conservative URL matcher; good enough for guardrails.
  const urls = [];
  const re = /https?:\/\/[^\s"'<>)]{1,2048}/g;
  let match;
  while ((match = re.exec(text)) !== null) {
    urls.push({ url: match[0], offset: match.index });
  }
  return urls;
}

function isAllowedUserFacingUrl(url) {
  return ALLOWED_URL_PREFIXES_USER_FACING.some((prefix) => url.startsWith(prefix));
}

function main() {
  const files = listTrackedFiles().filter((p) => !isExcludedFile(p));

  const violations = [];

  for (const relPath of files) {
    if (!isTextFile(relPath)) continue;

    const absPath = path.join(REPO_ROOT, relPath);
    let content;
    try {
      content = fs.readFileSync(absPath, 'utf8');
    } catch {
      // Ignore unreadable or non-utf8 files.
      continue;
    }

    // 1) Banned domain substrings anywhere in scanned repo surfaces.
    for (const banned of BANNED_SUBSTRINGS) {
      const offsets = findAllOccurrences(content, banned);
      for (const offset of offsets) {
        const { line } = offsetToLineCol(content, offset);
        violations.push({
          type: 'banned-domain',
          file: relPath,
          line,
          detail: banned,
        });
      }
    }

    // 2) User-facing allowlist for external URLs (docs/UI/README/etc).
    if (isUserFacingFile(relPath)) {
      for (const { url, offset } of extractHttpUrls(content)) {
        if (isAllowedUserFacingUrl(url)) continue;
        const { line } = offsetToLineCol(content, offset);
        violations.push({
          type: 'disallowed-url',
          file: relPath,
          line,
          detail: url,
        });
      }
    }
  }

  if (violations.length > 0) {
    console.error('External link guardrail failed:\n');
    for (const v of violations) {
      console.error(`- ${v.file}:${v.line} [${v.type}] ${v.detail}`);
    }
    console.error(
      '\nPolicy: ban upstream-hosted domains and allow only upstream GitHub links in user-facing docs/UI.'
    );
    process.exit(1);
  }

  process.stdout.write('External link guardrail passed.\n');
}

main();
