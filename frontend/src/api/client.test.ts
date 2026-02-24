import { beforeEach, describe, expect, it, vi } from 'vitest';

import { makeRequest } from './client';
import { clearApiToken, setApiToken } from './token';

describe('makeRequest', () => {
  const fetchMock = vi.fn().mockResolvedValue({ ok: true } as Response);

  beforeEach(() => {
    localStorage.clear();
    clearApiToken();
    fetchMock.mockClear();
    globalThis.fetch = fetchMock as typeof fetch;
  });

  it('attaches Authorization when token present', async () => {
    setApiToken('sekrit');

    await makeRequest('/api/info');

    const init = fetchMock.mock.calls[0]?.[1] as RequestInit | undefined;
    const headers = new Headers(init?.headers);
    expect(headers.get('Authorization')).toBe('Bearer sekrit');
  });

  it('does not attach Authorization when token missing', async () => {
    await makeRequest('/api/info');

    const init = fetchMock.mock.calls[0]?.[1] as RequestInit | undefined;
    const headers = new Headers(init?.headers);
    expect(headers.has('Authorization')).toBe(false);
  });
});
