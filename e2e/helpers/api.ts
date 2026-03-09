import type { APIRequestContext, APIResponse } from '@playwright/test';
import { expect } from '@playwright/test';

type ApiEnvelope<T> =
  | { success: true; data: T; message?: string }
  | { success: false; error_data?: unknown; message?: string };

async function unwrap<T>(response: APIResponse): Promise<T> {
  expect(response.ok()).toBeTruthy();
  const json = (await response.json()) as ApiEnvelope<T>;
  if (!json.success) {
    throw new Error(json.message || 'API request failed');
  }
  return json.data;
}

export async function apiPost<T>(
  request: APIRequestContext,
  url: string,
  data: unknown
): Promise<T> {
  const response = await request.post(url, { data });
  return unwrap<T>(response);
}

export async function apiPut<T>(
  request: APIRequestContext,
  url: string,
  data: unknown
): Promise<T> {
  const response = await request.put(url, { data });
  return unwrap<T>(response);
}

export async function apiDelete(
  request: APIRequestContext,
  url: string
): Promise<void> {
  const response = await request.delete(url);
  await unwrap<void>(response);
}

