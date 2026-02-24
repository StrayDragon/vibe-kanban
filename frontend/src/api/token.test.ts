import { beforeEach, describe, expect, it } from 'vitest';

import { clearApiToken, setApiToken, withApiTokenQuery } from './token';

describe('withApiTokenQuery', () => {
  beforeEach(() => {
    localStorage.clear();
    clearApiToken();
  });

  it('adds token as query param when present', () => {
    setApiToken('sekrit');

    const result = withApiTokenQuery('/api/events');
    const parsed = new URL(result, window.location.origin);
    expect(parsed.searchParams.get('token')).toBe('sekrit');
  });

  it('keeps the url unchanged when token is missing', () => {
    expect(withApiTokenQuery('/api/events')).toBe('/api/events');
  });
});
