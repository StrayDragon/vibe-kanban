import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { useConversationHistory } from './useConversationHistory';
import { streamLogEntries } from '@/utils/streamLogEntries';
import type {
  ApiResponse,
  ExecutionProcess,
  LogHistoryPage,
  PatchType,
  Workspace,
} from 'shared/types';
import { BaseCodingAgent, ExecutionProcessStatus } from 'shared/types';

const mockExecutionContext = {
  executionProcessesVisible: [] as ExecutionProcess[],
  isLoading: false,
};

vi.mock('@/contexts/ExecutionProcessesContext', () => ({
  useExecutionProcessesContext: () => mockExecutionContext,
}));

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

describe('useConversationHistory', () => {
  it('loads older history pages on demand', async () => {
    const now = new Date().toISOString();
    const executionProcess: ExecutionProcess = {
      id: 'process-1',
      session_id: 'session-1',
      run_reason: 'codingagent',
      executor_action: {
        typ: {
          type: 'CodingAgentInitialRequest',
          prompt: 'hello',
          executor_profile_id: {
            executor: BaseCodingAgent.CODEX,
            variant: null,
          },
          working_dir: null,
        },
        next_action: null,
      },
      status: ExecutionProcessStatus.completed,
      exit_code: null,
      dropped: false,
      started_at: now,
      completed_at: now,
      created_at: now,
      updated_at: now,
    };

    mockExecutionContext.executionProcessesVisible = [executionProcess];

    const normalizedEntry: PatchType = {
      type: 'NORMALIZED_ENTRY',
      content: {
        entry_type: { type: 'assistant_message' },
        content: 'hi',
        metadata: null,
        timestamp: null,
      },
    };

    const pageOne: LogHistoryPage = {
      entries: [
        { entry_index: 3n, entry: normalizedEntry },
        { entry_index: 4n, entry: normalizedEntry },
      ],
      next_cursor: 3n,
      has_more: true,
    };

    const pageTwo: LogHistoryPage = {
      entries: [
        { entry_index: 1n, entry: normalizedEntry },
        { entry_index: 2n, entry: normalizedEntry },
      ],
      next_cursor: 1n,
      has_more: false,
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

    const attempt: Workspace = {
      id: 'workspace-1',
      task_id: 'task-1',
      container_ref: null,
      branch: 'main',
      agent_working_dir: null,
      setup_completed_at: null,
      created_at: now,
      updated_at: now,
    };

    const onEntriesUpdated = vi.fn();
    const { result } = renderHook(() =>
      useConversationHistory({ attempt, onEntriesUpdated })
    );

    await waitFor(() => expect(result.current.hasMoreHistory).toBe(true));

    const calls = onEntriesUpdated.mock.calls;
    const initialEntries = calls[calls.length - 1]?.[0] ?? [];

    await act(async () => {
      await result.current.loadOlderHistory();
    });

    await waitFor(() => {
      const latestCalls = onEntriesUpdated.mock.calls;
      const latestEntries = latestCalls[latestCalls.length - 1]?.[0] ?? [];
      expect(latestEntries.length).toBeGreaterThan(initialEntries.length);
    });

    expect(fetchMock.mock.calls[1][0]).toContain('cursor=3');
  });

  it('loads older processes when history is paged', async () => {
    const now = new Date().toISOString();
    const oldest = '2024-01-01T00:00:00.000Z';
    const middle = '2024-01-02T00:00:00.000Z';
    const newest = '2024-01-03T00:00:00.000Z';

    const makeProcess = (id: string, createdAt: string): ExecutionProcess => ({
      id,
      session_id: `session-${id}`,
      run_reason: 'codingagent',
      executor_action: {
        typ: {
          type: 'CodingAgentInitialRequest',
          prompt: 'hello',
          executor_profile_id: {
            executor: BaseCodingAgent.CODEX,
            variant: null,
          },
          working_dir: null,
        },
        next_action: null,
      },
      status: ExecutionProcessStatus.completed,
      exit_code: null,
      dropped: false,
      started_at: createdAt,
      completed_at: createdAt,
      created_at: createdAt,
      updated_at: createdAt,
    });

    const normalizedEntry: PatchType = {
      type: 'NORMALIZED_ENTRY',
      content: {
        entry_type: { type: 'assistant_message' },
        content: 'hi',
        metadata: null,
        timestamp: null,
      },
    };

    const makePage = (): LogHistoryPage => ({
      entries: Array.from({ length: 20 }, (_, i) => ({
        entry_index: BigInt(i + 1),
        entry: normalizedEntry,
      })),
      next_cursor: 1n,
      has_more: false,
    });

    const oldestProcess = makeProcess('process-0', oldest);
    const middleProcess = makeProcess('process-1', middle);
    const newestProcess = makeProcess('process-2', newest);

    mockExecutionContext.executionProcessesVisible = [
      oldestProcess,
      middleProcess,
      newestProcess,
    ];

    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce({
        ok: true,
        json: async () => makeApiResponse(makePage()),
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => makeApiResponse(makePage()),
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => makeApiResponse(makePage()),
      });

    globalThis.fetch = fetchMock as typeof fetch;

    streamLogEntriesMock.mockImplementation(() => ({
      close: vi.fn(),
      isConnected: () => true,
    }));

    const attempt: Workspace = {
      id: 'workspace-2',
      task_id: 'task-2',
      container_ref: null,
      branch: 'main',
      agent_working_dir: null,
      setup_completed_at: null,
      created_at: now,
      updated_at: now,
    };

    const onEntriesUpdated = vi.fn();
    const { result } = renderHook(() =>
      useConversationHistory({ attempt, onEntriesUpdated })
    );

    await waitFor(() => expect(result.current.hasMoreHistory).toBe(true));

    const beforeEntries =
      onEntriesUpdated.mock.calls[onEntriesUpdated.mock.calls.length - 1]?.[0]
        ?.length ?? 0;

    await act(async () => {
      await result.current.loadOlderHistory();
    });

    await waitFor(() => {
      const latestCalls = onEntriesUpdated.mock.calls;
      const latestEntries = latestCalls[latestCalls.length - 1]?.[0] ?? [];
      expect(latestEntries.length).toBeGreaterThan(beforeEntries);
    });

    expect(fetchMock.mock.calls[2][0]).toContain(oldestProcess.id);
  });
});
