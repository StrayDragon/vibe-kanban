import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { useLogStream } from './useLogStream';
import { streamLogEntries } from '@/utils/streamLogEntries';
import type { ApiResponse, LogHistoryPage, PatchType } from 'shared/types';

vi.mock('@/utils/streamLogEntries', () => ({
  streamLogEntries: vi.fn(),
}));

const streamLogEntriesMock = vi.mocked(streamLogEntries);

const makeApiResponse = (data: LogHistoryPage): ApiResponse<LogHistoryPage> => ({
  success: true,
  data,
  error_data: null,
  message: null,
});

describe('useLogStream', () => {
  it('tracks hasMoreHistory without marking history as truncated', async () => {
    const rawEntryA: PatchType = { type: 'STDOUT', content: 'line-a' };
    const rawEntryB: PatchType = { type: 'STDERR', content: 'line-b' };

    const pageOne: LogHistoryPage = {
      entries: [
        { entry_index: 1n, entry: rawEntryA },
        { entry_index: 2n, entry: rawEntryB },
      ],
      next_cursor: 1n,
      has_more: true,
      history_truncated: false,
    };

    const pageTwo: LogHistoryPage = {
      entries: [{ entry_index: 0n, entry: rawEntryA }],
      next_cursor: null,
      has_more: false,
      history_truncated: false,
    };

    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce({
        ok: true,
        json: async () => makeApiResponse(pageOne),
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => makeApiResponse(pageTwo),
      });

    globalThis.fetch = fetchMock as typeof fetch;

    streamLogEntriesMock.mockImplementation(() => ({
      close: vi.fn(),
      isConnected: () => true,
    }));

    const { result } = renderHook(() => useLogStream('process-1'));

    await waitFor(() => expect(result.current.hasMoreHistory).toBe(true));
    expect(result.current.historyTruncated).toBe(false);
    expect(result.current.bufferTruncated).toBe(false);

    await act(async () => {
      await result.current.loadOlder();
    });

    await waitFor(() => expect(result.current.hasMoreHistory).toBe(false));
    await waitFor(() => expect(result.current.historyTruncated).toBe(false));
  });

  it('flags historyTruncated when server reports partial history', async () => {
    const rawEntry: PatchType = { type: 'STDOUT', content: 'partial' };

    const page: LogHistoryPage = {
      entries: [{ entry_index: 1n, entry: rawEntry }],
      next_cursor: null,
      has_more: false,
      history_truncated: true,
    };

    const fetchMock = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: async () => makeApiResponse(page),
    });

    globalThis.fetch = fetchMock as typeof fetch;

    streamLogEntriesMock.mockImplementation(() => ({
      close: vi.fn(),
      isConnected: () => true,
    }));

    const { result } = renderHook(() => useLogStream('process-2'));

    await waitFor(() => expect(result.current.historyTruncated).toBe(true));
  });
});
