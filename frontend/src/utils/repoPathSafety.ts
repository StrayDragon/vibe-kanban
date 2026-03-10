export type UnsafeRepoPathWarning = 'temp_dir' | 'git_worktree';

type WarningRule = {
  warning: UnsafeRepoPathWarning;
  matches: (normalizedPath: string) => boolean;
};

const normalizePath = (path: string): string =>
  path.replace(/\\/g, '/').replace(/\/+/g, '/').toLowerCase();

const RULES: WarningRule[] = [
  {
    warning: 'temp_dir',
    matches: (p) =>
      p.startsWith('/tmp/') ||
      p === '/tmp' ||
      p.startsWith('/var/tmp/') ||
      p === '/var/tmp' ||
      p.startsWith('/private/tmp/') ||
      p === '/private/tmp' ||
      p.startsWith('/private/var/folders/') ||
      p === '/private/var/folders',
  },
  {
    warning: 'git_worktree',
    matches: (p) =>
      p.includes('/.git/worktrees/') || /(^|\/)worktrees?(\/|$)/.test(p),
  },
];

export function getUnsafeRepoPathWarnings(
  path: string
): UnsafeRepoPathWarning[] {
  const trimmed = path.trim();
  if (!trimmed) return [];
  const normalized = normalizePath(trimmed);

  const warnings = new Set<UnsafeRepoPathWarning>();
  RULES.forEach((rule) => {
    if (rule.matches(normalized)) {
      warnings.add(rule.warning);
    }
  });
  return [...warnings];
}

export function isUnsafeRepoPath(path: string): boolean {
  return getUnsafeRepoPathWarnings(path).length > 0;
}
