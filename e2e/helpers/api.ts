import type { APIRequestContext, APIResponse } from '@playwright/test';

type ApiEnvelope<T> =
  | { success: true; data: T; message?: string }
  | { success: false; error_data?: unknown; message?: string };

function resolveApiUrl(url: string): string {
  if (url.startsWith('http://') || url.startsWith('https://')) return url;
  const base =
    process.env.VK_E2E_BACKEND_BASE_URL ?? process.env.VK_E2E_BASE_URL ?? '';
  if (!base) return url;
  return new URL(url, base).toString();
}

async function unwrap<T>(response: APIResponse): Promise<T> {
  if (!response.ok()) {
    const text = await response.text().catch(() => '');
    throw new Error(
      `API request failed: ${response.status()} ${response.url()}\n${text}`
    );
  }
  const json = (await response.json()) as ApiEnvelope<T>;
  if (!json.success) {
    throw new Error(json.message || 'API request failed');
  }
  return json.data;
}

export async function apiGet<T>(
  request: APIRequestContext,
  url: string
): Promise<T> {
  const response = await request.get(resolveApiUrl(url));
  return unwrap<T>(response);
}

export async function apiPost<T>(
  request: APIRequestContext,
  url: string,
  data: unknown
): Promise<T> {
  const response = await request.post(resolveApiUrl(url), { data });
  return unwrap<T>(response);
}

export async function apiPut<T>(
  request: APIRequestContext,
  url: string,
  data: unknown
): Promise<T> {
  const response = await request.put(resolveApiUrl(url), { data });
  return unwrap<T>(response);
}

export async function apiDelete(
  request: APIRequestContext,
  url: string
): Promise<void> {
  const response = await request.delete(resolveApiUrl(url));
  await unwrap<void>(response);
}
