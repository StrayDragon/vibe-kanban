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

    const pages: LogHistoryPage[] = Array.from({ length: 11 }, (_, pageIndex) => {
      const start = 201 - pageIndex * 20;
      const entries = Array.from({ length: 20 }, (_, i) => ({
        entry_index: BigInt(start + i),
        entry: normalizedEntry,
      }));
      const next_cursor = BigInt(start);
      const has_more = pageIndex < 10;
      return { entries, next_cursor, has_more, history_truncated: false };
    });

    const fetchMock = vi.fn();
    pages.forEach((page) => {
      fetchMock.mockResolvedValueOnce({
        ok: true,
        json: async () => makeApiResponse(page),
      });
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
    await waitFor(() => expect(result.current.historyTruncated).toBe(false));

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

    expect(fetchMock.mock.calls[10][0]).toContain('cursor=21');
  });

  it('flags historyTruncated when server reports partial history', async () => {
    const now = new Date().toISOString();
    const executionProcess: ExecutionProcess = {
      id: 'process-truncated',
      session_id: 'session-truncated',
      run_reason: 'codingagent',
      executor_action: {
        typ: {
          type: 'CodingAgentInitialRequest',
          prompt: 'hi',
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

    const page: LogHistoryPage = {
      entries: [{ entry_index: 1n, entry: normalizedEntry }],
      next_cursor: null,
      has_more: false,
      history_truncated: true,
    };

    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => makeApiResponse(page),
    });

    globalThis.fetch = fetchMock as typeof fetch;

    streamLogEntriesMock.mockImplementation(() => ({
      close: vi.fn(),
      isConnected: () => true,
    }));

    const attempt: Workspace = {
      id: 'workspace-truncated',
      task_id: 'task-truncated',
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

    await waitFor(() => expect(result.current.historyTruncated).toBe(true));
  });

  it('loads older processes when history is paged', async () => {
    const now = new Date().toISOString();
    const baseDate = new Date('2024-01-01T00:00:00.000Z');

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
      history_truncated: false,
    });

    const processes = Array.from({ length: 12 }, (_, index) => {
      const date = new Date(baseDate);
      date.setDate(baseDate.getDate() + index);
      return makeProcess(`process-${index}`, date.toISOString());
    });

    mockExecutionContext.executionProcessesVisible = processes;

    const fetchMock = vi.fn().mockResolvedValue({
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

    expect(
      fetchMock.mock.calls[fetchMock.mock.calls.length - 1]?.[0]
    ).toContain(processes[2].id);
  });
});
