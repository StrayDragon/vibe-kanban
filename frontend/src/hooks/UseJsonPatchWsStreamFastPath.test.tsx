import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

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

describe('useJsonPatchWsStream id-map fast-path', () => {
  const wsConstants = {
    CONNECTING: 0,
    OPEN: 1,
    CLOSING: 2,
    CLOSED: 3,
  };

  let prevWebSocket: unknown;
  let prevRaf: unknown;

  beforeEach(() => {
    createWebSocketMock.mockReset();
    vi.useFakeTimers();

    const globalWebSocket = globalThis as unknown as {
      WebSocket?: typeof wsConstants;
    };
    prevWebSocket = globalWebSocket.WebSocket;
    globalWebSocket.WebSocket = wsConstants;

    // Force timer-based batching in tests (easier to control than rAF).
    prevRaf = (window as unknown as { requestAnimationFrame?: unknown })
      .requestAnimationFrame;
    (
      window as unknown as { requestAnimationFrame?: unknown }
    ).requestAnimationFrame = undefined;
  });

  afterEach(() => {
    const globalWebSocket = globalThis as unknown as {
      WebSocket?: typeof wsConstants;
    };
    globalWebSocket.WebSocket = prevWebSocket as typeof wsConstants;
    (
      window as unknown as { requestAnimationFrame?: unknown }
    ).requestAnimationFrame = prevRaf;
    vi.useRealTimers();
  });

  it('applies id-map patches with structural sharing', async () => {
    const ws = new StubWebSocket();
    ws.readyState = wsConstants.CONNECTING;
    createWebSocketMock.mockReturnValue(ws);

    let markerRef: { v: number } | null = null;
    const initialData = () => {
      const marker = { v: 1 };
      markerRef = marker;
      return { tasks: {} as Record<string, unknown>, marker };
    };

    const { result } = renderHook(() =>
      useJsonPatchWsStream('/api/tasks/stream/ws', true, initialData)
    );

    await act(async () => {});

    act(() => {
      ws.readyState = wsConstants.OPEN;
      ws.onopen?.();
    });

    act(() => {
      ws.onmessage?.({
        data: JSON.stringify({
          seq: 1,
          JsonPatch: [
            { op: 'add', path: '/tasks/task-1', value: { id: 'task-1' } },
          ],
        }),
      } as unknown as MessageEvent);
    });

    act(() => {
      vi.runAllTimers();
    });

    expect(result.current.data?.tasks['task-1']).toEqual({ id: 'task-1' });
    expect(result.current.data?.marker).toBe(markerRef);
  });

  it('falls back to deep cloning for non id-map patches', async () => {
    const ws = new StubWebSocket();
    ws.readyState = wsConstants.CONNECTING;
    createWebSocketMock.mockReturnValue(ws);

    let markerRef: { v: number } | null = null;
    const initialData = () => {
      const marker = { v: 1 };
      markerRef = marker;
      return { tasks: {} as Record<string, unknown>, marker };
    };

    const { result } = renderHook(() =>
      useJsonPatchWsStream('/api/tasks/stream/ws', true, initialData)
    );

    await act(async () => {});

    act(() => {
      ws.readyState = wsConstants.OPEN;
      ws.onopen?.();
    });

    act(() => {
      ws.onmessage?.({
        data: JSON.stringify({
          seq: 1,
          JsonPatch: [
            { op: 'replace', path: '/tasks/task-1', value: { id: 'task-1' } },
            { op: 'replace', path: '/marker/v', value: 2 },
          ],
        }),
      } as unknown as MessageEvent);
    });

    act(() => {
      vi.runAllTimers();
    });

    expect(result.current.data?.tasks['task-1']).toEqual({ id: 'task-1' });
    expect(result.current.data?.marker?.v).toBe(2);
    expect(result.current.data?.marker).not.toBe(markerRef);
  });

  it('batches multiple patch messages into one flush', async () => {
    const ws = new StubWebSocket();
    ws.readyState = wsConstants.CONNECTING;
    createWebSocketMock.mockReturnValue(ws);

    const initialData = () => ({ tasks: {} as Record<string, unknown> });

    const { result } = renderHook(() =>
      useJsonPatchWsStream('/api/tasks/stream/ws', true, initialData)
    );

    await act(async () => {});

    act(() => {
      ws.readyState = wsConstants.OPEN;
      ws.onopen?.();
    });

    act(() => {
      ws.onmessage?.({
        data: JSON.stringify({
          seq: 1,
          JsonPatch: [{ op: 'add', path: '/tasks/a', value: { id: 'a' } }],
        }),
      } as unknown as MessageEvent);
      ws.onmessage?.({
        data: JSON.stringify({
          seq: 2,
          JsonPatch: [{ op: 'add', path: '/tasks/b', value: { id: 'b' } }],
        }),
      } as unknown as MessageEvent);
    });

    act(() => {
      vi.runAllTimers();
    });

    expect(result.current.data?.tasks['a']).toEqual({ id: 'a' });
    expect(result.current.data?.tasks['b']).toEqual({ id: 'b' });
  });

  it('exposes invalidate hints via callback', async () => {
    const ws = new StubWebSocket();
    ws.readyState = wsConstants.CONNECTING;
    createWebSocketMock.mockReturnValue(ws);

    const onInvalidate = vi.fn();
    const initialData = () => ({ tasks: {} as Record<string, unknown> });

    renderHook(() =>
      useJsonPatchWsStream('/api/tasks/stream/ws', true, initialData, {
        onInvalidate,
      })
    );

    await act(async () => {});

    act(() => {
      ws.readyState = wsConstants.OPEN;
      ws.onopen?.();
    });

    act(() => {
      ws.onmessage?.({
        data: JSON.stringify({
          seq: 1,
          invalidate: {
            taskIds: ['a'],
            workspaceIds: [],
            hasExecutionProcess: false,
          },
          JsonPatch: [{ op: 'add', path: '/tasks/a', value: { id: 'a' } }],
        }),
      } as unknown as MessageEvent);
    });

    expect(onInvalidate).toHaveBeenCalledTimes(1);
    expect(onInvalidate).toHaveBeenCalledWith(
      {
        taskIds: ['a'],
        workspaceIds: [],
        hasExecutionProcess: false,
      },
      { seq: 1 }
    );
  });
});
