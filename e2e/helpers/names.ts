function hash32(input: string): number {
  // FNV-1a 32-bit
  let hash = 0x811c9dc5;
  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i);
    // 32-bit overflow is intentional
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

function slugify(input: string): string {
  return input
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48);
}

export function makeDeterministicName(args: {
  seed: number;
  scope: string;
  prefix: string;
}): string {
  const scopeSlug = slugify(args.scope);
  const prefixSlug = slugify(args.prefix);
  const h = hash32(`${args.seed}:${args.scope}:${args.prefix}`);
  // Keep names short but informative; these show up in UI + filesystem paths.
  return `vk-e2e-${prefixSlug}-${args.seed}-${scopeSlug}-${h.toString(16)}`;
}

