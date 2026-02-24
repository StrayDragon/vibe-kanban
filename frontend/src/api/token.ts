const API_TOKEN_STORAGE_KEY = 'vk_api_token';

export function getApiToken(): string | null {
  if (typeof window === 'undefined') return null;

  try {
    const raw = window.localStorage.getItem(API_TOKEN_STORAGE_KEY);
    const token = raw?.trim() ?? '';
    return token.length ? token : null;
  } catch {
    return null;
  }
}

export function setApiToken(token: string) {
  if (typeof window === 'undefined') return;

  const trimmed = token.trim();
  if (!trimmed) {
    clearApiToken();
    return;
  }

  window.localStorage.setItem(API_TOKEN_STORAGE_KEY, trimmed);
}

export function clearApiToken() {
  if (typeof window === 'undefined') return;
  window.localStorage.removeItem(API_TOKEN_STORAGE_KEY);
}

export function withApiTokenQuery(url: string, tokenOverride?: string): string {
  const token = tokenOverride ?? getApiToken();
  if (!token) return url;

  try {
    const base =
      typeof window !== 'undefined'
        ? window.location.origin
        : 'http://localhost';
    const parsed = new URL(url, base);
    parsed.searchParams.set('token', token);

    // Preserve relative URLs when the input was relative.
    if (!/^(https?:|wss?:)/i.test(url)) {
      return `${parsed.pathname}${parsed.search}${parsed.hash}`;
    }

    return parsed.toString();
  } catch {
    const separator = url.includes('?') ? '&' : '?';
    return `${url}${separator}token=${encodeURIComponent(token)}`;
  }
}
