import type { Operation } from 'rfc6902';

const decodePointerSegment = (segment: string) =>
  segment.replace(/~1/g, '/').replace(/~0/g, '~');

const extractIdFromPath = (path: string, prefix: string) => {
  if (!path.startsWith(prefix)) return null;
  const remainder = path.slice(prefix.length);
  if (!remainder) return null;
  const [rawId] = remainder.split('/');
  return rawId ? decodePointerSegment(rawId) : null;
};

export const normalizeIdMapPatches = (
  patches: Operation[],
  map: Record<string, unknown> | undefined,
  prefix: string
): Operation[] => {
  if (!map) return patches;

  return patches.flatMap((patch) => {
    const id = extractIdFromPath(patch.path, prefix);
    if (!id) return [patch];

    const exists = Object.prototype.hasOwnProperty.call(map, id);
    if (patch.op === 'remove') {
      return exists ? [patch] : [];
    }
    if (patch.op === 'replace' && !exists) {
      return [{ ...patch, op: 'add' } as Operation];
    }
    return [patch];
  });
};
