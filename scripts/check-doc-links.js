#!/usr/bin/env node
/**
 * Docs link guardrail.
 *
 * Goal: detect references in markdown to missing repo files (dead internal links),
 * such as `docs/operations.md` when the file is absent.
 *
 * Scope: README + docs/*.md + ARCH.md (excludes OpenSpec).
 */

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const REPO_ROOT = process.cwd();

const MARKDOWN_FILES = [
  'README.md',
  'ARCH.md',
  ...listTrackedMarkdownUnder('docs'),
];

function listTrackedMarkdownUnder(dir) {
  const out = execFileSync('git', ['ls-files', '-z', dir], { encoding: 'utf8' });
  return out
    .split('\0')
    .map((p) => p.trim())
    .filter((p) => p.endsWith('.md'));
}

function readFileUtf8(relPath) {
  const absPath = path.join(REPO_ROOT, relPath);
  return fs.readFileSync(absPath, 'utf8');
}

function extractCandidateRepoPaths(mdText) {
  const results = [];

  // Markdown links: [text](path)
  // Capture the raw target and normalize later.
  const linkRe = /\]\(([^)]+)\)/g;
  let match;
  while ((match = linkRe.exec(mdText)) !== null) {
    results.push({ raw: match[1], offset: match.index });
  }

  // Also catch common inline references like `docs/foo.md` or plain `docs/foo.md`.
  const docsPathRe = /\b(?:\.\/)?docs\/[A-Za-z0-9._/-]+\.md\b/g;
  while ((match = docsPathRe.exec(mdText)) !== null) {
    results.push({ raw: match[0], offset: match.index });
  }

  return results;
}

function normalizeTarget(raw) {
  let t = raw.trim();
  if (t.startsWith('<') && t.endsWith('>')) t = t.slice(1, -1).trim();

  // Strip title portion: (path "title")
  if (t.includes(' ')) {
    t = t.split(' ')[0];
  }

  // Strip anchors.
  const hashIdx = t.indexOf('#');
  if (hashIdx !== -1) t = t.slice(0, hashIdx);

  // Ignore empty, anchors, and external schemes.
  if (!t) return null;
  if (t.startsWith('#')) return null;
  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(t)) return null;

  // Ignore absolute paths.
  if (path.isAbsolute(t)) return null;

  // Normalize ./ prefix.
  if (t.startsWith('./')) t = t.slice(2);

  return t;
}

function offsetToLine(text, offset) {
  let line = 1;
  for (let i = 0; i < offset; i++) {
    if (text.charCodeAt(i) === 10) line++;
  }
  return line;
}

function existsRepoPath(relPath) {
  const absPath = path.join(REPO_ROOT, relPath);
  return fs.existsSync(absPath);
}

function main() {
  const violations = [];

  for (const mdPath of MARKDOWN_FILES) {
    if (!fs.existsSync(path.join(REPO_ROOT, mdPath))) continue;
    const text = readFileUtf8(mdPath);
    const candidates = extractCandidateRepoPaths(text);

    for (const c of candidates) {
      const target = normalizeTarget(c.raw);
      if (!target) continue;

      // Only enforce for repo-relative links that point into docs/ (current need).
      // This keeps the check low-noise and targeted.
      if (!target.startsWith('docs/')) continue;

      if (!existsRepoPath(target)) {
        violations.push({
          file: mdPath,
          line: offsetToLine(text, c.offset),
          target,
        });
      }
    }
  }

  if (violations.length > 0) {
    console.error('Docs link guardrail failed:\n');
    for (const v of violations) {
      console.error(`- ${v.file}:${v.line} missing ${v.target}`);
    }
    process.exit(1);
  }

  process.stdout.write('Docs link guardrail passed.\n');
}

main();
