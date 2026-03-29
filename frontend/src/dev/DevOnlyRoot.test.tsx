import { describe, expect, it } from 'vitest';

import mainSource from '../main.tsx?raw';

describe('dev-only tooling guardrails', () => {
  it('does not statically import dev-only tooling from main entry', async () => {
    expect(mainSource).not.toContain("from 'click-to-react-component'");
    expect(mainSource).not.toContain('click-to-react-component');
    expect(mainSource).not.toContain("from 'vibe-kanban-web-companion'");
    expect(mainSource).not.toContain('vibe-kanban-web-companion');
  });

  it('loads dev-only tooling via dynamic import in development', async () => {
    expect(mainSource).toContain(
      "React.lazy(() => import('./dev/DevOnlyRoot'))"
    );
    expect(mainSource).toContain('import.meta.env.DEV');
  });
});
