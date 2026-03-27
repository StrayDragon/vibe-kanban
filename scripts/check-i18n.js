#!/usr/bin/env node
/* eslint-disable no-console */

const { execSync, spawnSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

const RULE = 'i18next/no-literal-string';
const REPO_ROOT = path.resolve(__dirname, '..');

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

function writeFileEnsuringDir(filePath, contents) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, contents);
}

function runEslintForI18nCount(targetRepoRoot, configPath, outputPath) {
  const frontendDir = path.join(targetRepoRoot, 'frontend');
  if (!fs.existsSync(frontendDir)) return;

  spawnSync(
    'npx',
    [
      '--prefix',
      path.join(REPO_ROOT, 'frontend'),
      'eslint',
      '.',
      '--config',
      configPath,
      '--ext',
      'ts,tsx',
      '--format',
      'json',
      '--output-file',
      outputPath,
      '--no-error-on-unmatched-pattern',
    ],
    {
      cwd: frontendDir,
      env: { ...process.env, LINT_I18N: 'true' },
      stdio: 'ignore',
    }
  );
}

function lintCount(repoDir) {
  const frontendDir = path.join(repoDir, 'frontend');
  if (!fs.existsSync(frontendDir)) return 0;

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'vk-eslint-i18n-'));
  const configPath = path.join(tmpDir, 'eslint-i18n.config.mjs');
  const outputPath = path.join(tmpDir, 'eslint-i18n.output.json');

  try {
    const frontendPkg = path.join(REPO_ROOT, 'frontend', 'package.json');
    const config = `import { createRequire } from 'node:module';

const require = createRequire(${JSON.stringify(frontendPkg)});
const tsParser = require('@typescript-eslint/parser');
const i18next = require('eslint-plugin-i18next');

const i18nCheck = process.env.LINT_I18N === 'true';

export default [
  { ignores: ['dist/**'] },
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
        ecmaFeatures: { jsx: true },
      },
    },
    plugins: { i18next },
    rules: {
      'i18next/no-literal-string': i18nCheck
        ? [
            'warn',
            {
              markupOnly: true,
              ignoreAttribute: [
                'data-testid',
                'to',
                'href',
                'id',
                'key',
                'type',
                'role',
                'className',
                'style',
                'aria-describedby',
              ],
              'jsx-components': {
                exclude: ['code'],
              },
            },
          ]
        : 'off',
    },
  },
];
`;

    writeFileEnsuringDir(configPath, config);
    runEslintForI18nCount(repoDir, configPath, outputPath);

    if (!fs.existsSync(outputPath)) return 0;

    const raw = fs.readFileSync(outputPath, 'utf8');
    const results = JSON.parse(raw);
    if (!Array.isArray(results)) return 0;

    let count = 0;
    for (const file of results) {
      const messages = Array.isArray(file?.messages) ? file.messages : [];
      for (const msg of messages) {
        if (msg?.ruleId === RULE) count += 1;
      }
    }
    return count;
  } catch {
    return 0;
  } finally {
    rmDirIfExists(tmpDir);
  }
}

function cloneBaselineOrNull(baseRef) {
  let remoteUrl = '';
  try {
    remoteUrl = execSync('git remote get-url origin', {
      cwd: REPO_ROOT,
      stdio: ['ignore', 'pipe', 'ignore'],
      encoding: 'utf8',
    }).trim();
  } catch {
    return null;
  }

  const baseDir = fs.mkdtempSync(path.join(os.tmpdir(), 'vk-i18n-base-'));
  try {
    execSync(
      `git clone --depth=1 --branch ${baseRef} --single-branch ${remoteUrl} ${baseDir}`,
      { stdio: 'ignore' }
    );
    return baseDir;
  } catch {
    rmDirIfExists(baseDir);
    return null;
  }
}

function listJsonFiles(dir) {
  const results = [];
  const stack = [dir];
  while (stack.length) {
    const current = stack.pop();
    let entries;
    try {
      entries = fs.readdirSync(current, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      const fullPath = path.join(current, entry.name);
      if (entry.isDirectory()) stack.push(fullPath);
      else if (entry.isFile() && entry.name.endsWith('.json')) results.push(fullPath);
    }
  }
  results.sort();
  return results;
}

function parseJsonWithDuplicateKeyDetection(text) {
  const duplicates = [];
  let index = 0;

  const len = text.length;
  const isWhitespace = (ch) => ch === ' ' || ch === '\n' || ch === '\r' || ch === '\t';

  function skipWs() {
    while (index < len && isWhitespace(text[index])) index += 1;
  }

  function expectChar(ch) {
    skipWs();
    if (text[index] !== ch) {
      throw new Error(`Expected '${ch}' at position ${index}`);
    }
    index += 1;
  }

  function parseString() {
    skipWs();
    if (text[index] !== '"') throw new Error(`Expected string at position ${index}`);
    index += 1;
    let out = '';
    while (index < len) {
      const ch = text[index];
      if (ch === '"') {
        index += 1;
        return out;
      }
      if (ch === '\\') {
        index += 1;
        if (index >= len) throw new Error('Unexpected end of string');
        const esc = text[index];
        switch (esc) {
          case '"':
          case '\\':
          case '/':
            out += esc;
            index += 1;
            break;
          case 'b':
            out += '\b';
            index += 1;
            break;
          case 'f':
            out += '\f';
            index += 1;
            break;
          case 'n':
            out += '\n';
            index += 1;
            break;
          case 'r':
            out += '\r';
            index += 1;
            break;
          case 't':
            out += '\t';
            index += 1;
            break;
          case 'u': {
            const hex = text.slice(index + 1, index + 5);
            if (!/^[0-9a-fA-F]{4}$/.test(hex)) {
              throw new Error(`Invalid unicode escape at position ${index}`);
            }
            out += String.fromCharCode(parseInt(hex, 16));
            index += 5;
            break;
          }
          default:
            throw new Error(`Invalid escape '\\${esc}' at position ${index}`);
        }
        continue;
      }
      out += ch;
      index += 1;
    }
    throw new Error('Unterminated string');
  }

  function parseNumber() {
    skipWs();
    const start = index;
    if (text[index] === '-') index += 1;
    while (index < len && /[0-9]/.test(text[index])) index += 1;
    if (text[index] === '.') {
      index += 1;
      while (index < len && /[0-9]/.test(text[index])) index += 1;
    }
    if (text[index] === 'e' || text[index] === 'E') {
      index += 1;
      if (text[index] === '+' || text[index] === '-') index += 1;
      while (index < len && /[0-9]/.test(text[index])) index += 1;
    }
    const raw = text.slice(start, index);
    if (!raw) throw new Error(`Invalid number at position ${start}`);
  }

  function parseLiteral(literal) {
    skipWs();
    if (text.slice(index, index + literal.length) !== literal) {
      throw new Error(`Expected '${literal}' at position ${index}`);
    }
    index += literal.length;
  }

  function parseValue(pathParts) {
    skipWs();
    const ch = text[index];
    if (ch === '{') return parseObject(pathParts);
    if (ch === '[') return parseArray(pathParts);
    if (ch === '"') {
      parseString();
      return;
    }
    if (ch === '-' || /[0-9]/.test(ch)) {
      parseNumber();
      return;
    }
    if (text.startsWith('true', index)) return parseLiteral('true');
    if (text.startsWith('false', index)) return parseLiteral('false');
    if (text.startsWith('null', index)) return parseLiteral('null');
    throw new Error(`Unexpected token at position ${index}`);
  }

  function parseArray(pathParts) {
    expectChar('[');
    skipWs();
    if (text[index] === ']') {
      index += 1;
      return;
    }
    while (true) {
      parseValue(pathParts);
      skipWs();
      if (text[index] === ',') {
        index += 1;
        continue;
      }
      if (text[index] === ']') {
        index += 1;
        return;
      }
      throw new Error(`Expected ',' or ']' at position ${index}`);
    }
  }

  function parseObject(pathParts) {
    expectChar('{');
    skipWs();
    const seen = new Set();
    if (text[index] === '}') {
      index += 1;
      return;
    }
    while (true) {
      const key = parseString();
      if (seen.has(key)) duplicates.push([...pathParts, key].join('.'));
      seen.add(key);
      expectChar(':');
      parseValue([...pathParts, key]);
      skipWs();
      if (text[index] === ',') {
        index += 1;
        continue;
      }
      if (text[index] === '}') {
        index += 1;
        return;
      }
      throw new Error(`Expected ',' or '}' at position ${index}`);
    }
  }

  skipWs();
  parseValue([]);
  skipWs();
  if (index < len) throw new Error(`Unexpected trailing content at position ${index}`);
  return { duplicates };
}

function checkDuplicateJsonKeys() {
  const localesDir = path.join(REPO_ROOT, 'frontend/src/i18n/locales');
  if (!fs.existsSync(localesDir)) {
    console.error(`❌ Locales directory not found: ${localesDir}`);
    return false;
  }

  let ok = true;
  for (const filePath of listJsonFiles(localesDir)) {
    const relPath = path.relative(localesDir, filePath);
    const raw = fs.readFileSync(filePath, 'utf8');
    try {
      const { duplicates } = parseJsonWithDuplicateKeyDetection(raw);
      if (duplicates.length) {
        console.error(`❌ [${relPath}] Duplicate keys found:`);
        for (const dup of duplicates.slice(0, 50)) {
          console.error(`   - ${dup}`);
        }
        if (duplicates.length > 50) {
          console.error(`   ... and ${duplicates.length - 50} more.`);
        }
        console.error('   JSON silently overwrites duplicate keys - only the last occurrence is used!');
        ok = false;
      }
    } catch (err) {
      console.error(`❌ [${relPath}] Invalid JSON: ${err?.message ?? String(err)}`);
      ok = false;
    }
  }
  return ok;
}

function collectStringLeafKeys(value, pathParts, out) {
  if (typeof value === 'string') {
    out.push(pathParts.join('.'));
    return;
  }
  if (!value || typeof value !== 'object') return;
  if (Array.isArray(value)) return;
  for (const [k, v] of Object.entries(value)) {
    collectStringLeafKeys(v, [...pathParts, k], out);
  }
}

function getStringLeafKeySet(filePath) {
  const raw = fs.readFileSync(filePath, 'utf8');
  const value = JSON.parse(raw);
  const keys = [];
  collectStringLeafKeys(value, [], keys);
  keys.sort();
  return new Set(keys);
}

function checkKeyConsistency() {
  const localesDir = path.join(REPO_ROOT, 'frontend/src/i18n/locales');
  const verbose = process.env.I18N_VERBOSE === '1';
  const failOnExtra = process.env.I18N_FAIL_ON_EXTRA === '1';

  const enDir = path.join(localesDir, 'en');
  if (!fs.existsSync(enDir)) {
    console.error(`❌ Missing source locale directory: ${enDir}`);
    return false;
  }

  const namespaces = fs
    .readdirSync(enDir)
    .filter((f) => f.endsWith('.json'))
    .map((f) => f.slice(0, -'.json'.length))
    .sort();

  const languages = fs
    .readdirSync(localesDir, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => e.name)
    .sort();

  if (!languages.includes('en')) {
    console.error(`❌ Source language 'en' not found in ${localesDir}`);
    return false;
  }

  let ok = true;

  for (const ns of namespaces) {
    const refFile = path.join(enDir, `${ns}.json`);
    let refKeys;
    try {
      refKeys = getStringLeafKeySet(refFile);
    } catch (err) {
      console.error(`❌ Invalid or unreadable JSON: ${refFile}`);
      ok = false;
      continue;
    }

    for (const lang of languages) {
      if (lang === 'en') continue;

      const tgtFile = path.join(localesDir, lang, `${ns}.json`);
      let tgtKeys;
      try {
        tgtKeys = getStringLeafKeySet(tgtFile);
      } catch {
        console.error(`❌ [${lang}/${ns}] Missing or invalid JSON: ${tgtFile}`);
        console.error(`   All keys from en/${ns} are considered missing.`);
        tgtKeys = new Set();
        ok = false;
      }

      const missing = [];
      for (const key of refKeys) {
        if (!tgtKeys.has(key)) missing.push(key);
      }

      const extra = [];
      for (const key of tgtKeys) {
        if (!refKeys.has(key)) extra.push(key);
      }

      if (missing.length) {
        console.error(`❌ [${lang}/${ns}] Missing keys:`);
        const display = verbose ? missing : missing.slice(0, 50);
        for (const key of display) console.error(`   - ${key}`);
        if (!verbose && missing.length > 50) {
          console.error(
            `   ... and ${missing.length - 50} more. Set I18N_VERBOSE=1 to print all.`
          );
        }
        ok = false;
      }

      if (extra.length) {
        const header = failOnExtra
          ? `❌ [${lang}/${ns}] Extra keys (not in en):`
          : `⚠️  [${lang}/${ns}] Extra keys (not in en):`;
        console.log(header);

        const display = verbose ? extra : extra.slice(0, 50);
        for (const key of display) console.log(`   - ${key}`);
        if (!verbose && extra.length > 50) {
          console.log(
            `   ... and ${extra.length - 50} more. Set I18N_VERBOSE=1 to print all.`
          );
        }
        if (failOnExtra) ok = false;
      }
    }
  }

  return ok;
}

function printFilesWithNewViolations() {
  const frontendDir = path.join(REPO_ROOT, 'frontend');
  if (!fs.existsSync(frontendDir)) return;
  try {
    spawnSync(
      'npx',
      ['eslint', '.', '--ext', 'ts,tsx', '--rule', `${RULE}:error`, '-f', 'codeframe'],
      {
        cwd: frontendDir,
        env: { ...process.env, LINT_I18N: 'true' },
        stdio: 'inherit',
      }
    );
  } catch {
    // ignore
  }
}

function main() {
  console.log('▶️  Counting literal strings in PR branch...');
  const prCount = lintCount(REPO_ROOT);

  const baseRef = process.env.GITHUB_BASE_REF || 'main';
  console.log(`▶️  Fetching ${baseRef} for baseline (shallow clone)...`);
  const baseDir = cloneBaselineOrNull(baseRef);
  const baseCount = baseDir ? lintCount(baseDir) : 0;
  if (!baseDir) console.log(`⚠️  Could not clone ${baseRef}; defaulting baseline to 0.`);
  if (baseDir) rmDirIfExists(baseDir);

  console.log('');
  console.log('📊 I18n Violation Summary:');
  console.log(`   Base branch (${baseRef}): ${baseCount} violations`);
  console.log(`   PR branch: ${prCount} violations`);
  console.log('');

  let exitStatus = 0;

  if (prCount > baseCount) {
    console.error(`❌ PR introduces ${prCount - baseCount} new hard-coded strings.`);
    console.error('');
    console.error('💡 To fix, replace hardcoded strings with translation calls:');
    console.error('   Before: <Button>Save</Button>');
    console.error("   After:  <Button>{t('buttons.save')}</Button>");
    console.error('');
    console.error('Files with new violations:');
    printFilesWithNewViolations();
    exitStatus = 1;
  } else if (prCount < baseCount) {
    console.log(`🎉 Great job! PR removes ${baseCount - prCount} hard-coded strings.`);
    console.log('   This helps improve i18n coverage!');
  } else {
    console.log('✅ No new literal strings introduced.');
  }

  console.log('');
  console.log('▶️  Checking for duplicate JSON keys...');
  if (!checkDuplicateJsonKeys()) {
    exitStatus = 1;
  } else {
    console.log('✅ No duplicate keys found in JSON files.');
  }

  console.log('');
  console.log('▶️  Checking translation key consistency...');
  if (!checkKeyConsistency()) {
    exitStatus = 1;
  } else {
    console.log('✅ Translation keys are consistent across locales.');
  }

  process.exit(exitStatus);
}

main();

