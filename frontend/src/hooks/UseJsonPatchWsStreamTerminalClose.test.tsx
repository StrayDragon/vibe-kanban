import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { useJsonPatchWsStream } from './useJsonPatchWsStream';

const createWebSocketMock = vi.fn();

vi.mock('@/lib/api', () => ({
  createWebSocket: (url: string) => createWebSocketMock(url),
}));

class StubWebSocket {
  readyState = 0;
  onopen: (() => void) | null = null;
  onmessage: ((evt: MessageEvent) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: ((evt: CloseEvent) => void) | null = null;
  close = vi.fn();
}

describe('useJsonPatchWsStream', () => {
  it('does not reconnect on 4404 terminal close', async () => {
    const wsConstants = {
      CONNECTING: 0,
      OPEN: 1,
      CLOSING: 2,
      CLOSED: 3,
    };
    const globalWebSocket = globalThis as unknown as {
      WebSocket?: typeof wsConstants;
    };
    const prevWebSocket = globalWebSocket.WebSocket;
    globalWebSocket.WebSocket = wsConstants;

    const ws = new StubWebSocket();
    ws.readyState = wsConstants.CONNECTING;
    createWebSocketMock.mockReturnValue(ws);

    const initialData = () => ({
      execution_processes: {},
    });

    const { result } = renderHook(() =>
      useJsonPatchWsStream(
        '/api/execution-processes/stream/ws?workspace_id=missing',
        true,
        initialData
      )
    );

    await act(async () => {});
    expect(createWebSocketMock).toHaveBeenCalledTimes(1);

    vi.useFakeTimers();

    act(() => {
      ws.readyState = wsConstants.OPEN;
      ws.onopen?.();
    });

    act(() => {
      ws.readyState = wsConstants.CLOSED;
      ws.onclose?.({
        code: 4404,
        reason: 'workspace_not_found',
        wasClean: true,
      } as unknown as CloseEvent);
    });

    // Advance time past the max backoff; should not schedule reconnect.
    act(() => {
      vi.advanceTimersByTime(30_000);
    });

    expect(createWebSocketMock).toHaveBeenCalledTimes(1);
    expect(result.current.error).toBe('Workspace not found');

    globalWebSocket.WebSocket = prevWebSocket;
    vi.useRealTimers();
  });
});
