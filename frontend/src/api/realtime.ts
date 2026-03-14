import { withApiTokenQuery } from './token';

export function toWebSocketUrl(url: string): string {
  // Convert http(s) -> ws(s) while keeping relative URLs intact.
  const wsLike = url.replace(/^http/i, 'ws');
  return withApiTokenQuery(wsLike);
}

export function createWebSocket(url: string): WebSocket {
  return new WebSocket(toWebSocketUrl(url));
}

export function createEventSource(url: string): EventSource {
  return new EventSource(withApiTokenQuery(url));
}

