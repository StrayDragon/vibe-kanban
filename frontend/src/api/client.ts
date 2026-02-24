import type { ApiResponse } from 'shared/types';
import { getApiToken } from './token';

export class ApiError<E = unknown> extends Error {
  public status?: number;
  public error_data?: E;

  constructor(
    message: string,
    public statusCode?: number,
    public response?: Response,
    error_data?: E
  ) {
    super(message);
    this.name = 'ApiError';
    this.status = statusCode;
    this.error_data = error_data;
  }
}

export const makeRequest = async (url: string, options: RequestInit = {}) => {
  const headers = new Headers(options.headers ?? {});
  if (!headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }

  const token = getApiToken();
  if (token && !headers.has('Authorization') && !headers.has('X-API-Token')) {
    headers.set('Authorization', `Bearer ${token}`);
  }

  return fetch(url, {
    ...options,
    headers,
  });
};

export type Ok<T> = { success: true; data: T };
export type Err<E> = { success: false; error: E | undefined; message?: string };

export type Result<T, E> = Ok<T> | Err<E>;

export const handleApiResponseAsResult = async <T, E>(
  response: Response
): Promise<Result<T, E>> => {
  if (!response.ok) {
    let errorMessage = `Request failed with status ${response.status}`;
    let errorData: E | undefined;

    try {
      const payload = (await response.json()) as Partial<
        ApiResponse<unknown, E>
      > & {
        message?: unknown;
        error_data?: unknown;
      };
      if (typeof payload.message === 'string') {
        errorMessage = payload.message;
      }
      errorData = payload.error_data as E | undefined;
    } catch {
      errorMessage = response.statusText || errorMessage;
    }

    return {
      success: false,
      error: errorData,
      message: errorMessage,
    };
  }

  const result: ApiResponse<T, E> = await response.json();

  if (!result.success) {
    return {
      success: false,
      error: result.error_data || undefined,
      message: result.message || undefined,
    };
  }

  return { success: true, data: result.data as T };
};

export const handleApiResponse = async <T, E = T>(
  response: Response
): Promise<T> => {
  if (!response.ok) {
    let errorMessage = `Request failed with status ${response.status}`;
    let errorData: E | undefined;

    try {
      const payload = (await response.json()) as Partial<
        ApiResponse<unknown, E>
      > & {
        message?: unknown;
        error_data?: unknown;
      };
      if (typeof payload.message === 'string') {
        errorMessage = payload.message;
      }
      errorData = payload.error_data as E | undefined;
    } catch {
      errorMessage = response.statusText || errorMessage;
    }

    console.error('[API Error]', {
      message: errorMessage,
      status: response.status,
      response,
      endpoint: response.url,
      timestamp: new Date().toISOString(),
    });
    throw new ApiError<E>(errorMessage, response.status, response, errorData);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  const result: ApiResponse<T, E> = await response.json();

  if (!result.success) {
    if (result.error_data) {
      console.error('[API Error with data]', {
        error_data: result.error_data,
        message: result.message,
        status: response.status,
        response,
        endpoint: response.url,
        timestamp: new Date().toISOString(),
      });
      throw new ApiError<E>(
        result.message || 'API request failed',
        response.status,
        response,
        result.error_data
      );
    }

    console.error('[API Error]', {
      message: result.message || 'API request failed',
      status: response.status,
      response,
      endpoint: response.url,
      timestamp: new Date().toISOString(),
    });
    throw new ApiError<E>(
      result.message || 'API request failed',
      response.status,
      response
    );
  }

  return result.data as T;
};
