import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { scrollToIndexMock } = vi.hoisted(() => ({
  scrollToIndexMock: vi.fn(),
}));

vi.mock('react-virtuoso', async () => {
  const React = await import('react');

  type VirtuosoMockProps = {
    data?: unknown[];
    context?: unknown;
    computeItemKey?: (index: number, data: unknown) => string;
    itemContent?: (index: number, data: unknown, context: unknown) => React.ReactNode;
    scrollerRef?: (element: HTMLElement | Window | null) => void;
  };

  type VirtuosoHandleLike = {
    scrollToIndex: (opts: unknown) => void;
  };

  const Virtuoso = React.forwardRef<VirtuosoHandleLike, VirtuosoMockProps>(
    (props, ref) => {
      const {
        data = [],
        context,
        computeItemKey,
        itemContent,
        scrollerRef: scrollerRefProp,
      } = props;

    React.useImperativeHandle(ref, () => ({ scrollToIndex: scrollToIndexMock }));

    const scrollerRef = React.useRef<HTMLDivElement | null>(null);
    const itemElsRef = React.useRef<Map<number, HTMLDivElement>>(new Map());

    React.useEffect(() => {
      scrollerRefProp?.(scrollerRef.current);
    }, [scrollerRefProp]);

    React.useLayoutEffect(() => {
      const scroller = scrollerRef.current;
      if (scroller) {
        scroller.getBoundingClientRect = () =>
          ({
            top: 0,
            bottom: 100,
            left: 0,
            right: 100,
            width: 100,
            height: 100,
            x: 0,
            y: 0,
            toJSON: () => {},
          }) as DOMRect;
      }

      for (const [index, el] of itemElsRef.current.entries()) {
        el.getBoundingClientRect = () =>
          ({
            top: index * 20,
            bottom: (index + 1) * 20,
            left: 0,
            right: 100,
            width: 100,
            height: 20,
            x: 0,
            y: index * 20,
            toJSON: () => {},
          }) as DOMRect;
      }
    }, [data]);

    return (
      <div ref={scrollerRef}>
        {data.map((item, index) => {
          const key = computeItemKey?.(index, item) ?? String(index);
          return (
            <div
              key={key}
              data-index={index}
              ref={(el) => {
                if (el) itemElsRef.current.set(index, el);
              }}
            >
              {itemContent?.(index, item, context)}
            </div>
          );
        })}
      </div>
    );
    }
  );

  Virtuoso.displayName = 'Virtuoso';

  return {
    Virtuoso,
    VirtuosoHandle: {},
  };
});

vi.mock('@/hooks/execution-processes/useConversationHistory', async () => {
  const React = await import('react');

  type PatchTypeWithKeyLike = {
    type: string;
    content: unknown;
    patchKey: string;
    executionProcessId: string;
  };

  type AddEntryTypeLike = 'initial' | 'running' | 'historic';

  type OnEntriesUpdatedLike = (
    newEntries: PatchTypeWithKeyLike[],
    addType: AddEntryTypeLike,
    loading: boolean
  ) => void;

  type UseConversationHistoryParamsLike = {
    attempt: { id: string };
    onEntriesUpdated: OnEntriesUpdatedLike;
  };

  const initialEntries: PatchTypeWithKeyLike[] = [
    {
      type: 'STDOUT',
      content: 'line-1',
      patchKey: 'process-1:1',
      executionProcessId: 'process-1',
    },
    {
      type: 'STDOUT',
      content: 'line-2',
      patchKey: 'process-1:2',
      executionProcessId: 'process-1',
    },
  ];

  const historicEntries: PatchTypeWithKeyLike[] = [
    {
      type: 'STDOUT',
      content: 'line-0',
      patchKey: 'process-1:0',
      executionProcessId: 'process-1',
    },
    ...initialEntries,
  ];

  return {
    useConversationHistory: ({
      attempt,
      onEntriesUpdated,
    }: UseConversationHistoryParamsLike) => {
      const onEntriesUpdatedRef = React.useRef<OnEntriesUpdatedLike>(
        onEntriesUpdated
      );

      React.useEffect(() => {
        onEntriesUpdatedRef.current = onEntriesUpdated;
      }, [onEntriesUpdated]);

      React.useEffect(() => {
        onEntriesUpdatedRef.current(initialEntries, 'initial', false);
      }, [attempt.id]);

      return {
        loadOlderHistory: async () => {
          onEntriesUpdatedRef.current(historicEntries, 'historic', false);
        },
        hasMoreHistory: true,
        loadingOlder: false,
        historyTruncated: false,
      };
    },
  };
});

import { EntriesProvider } from '@/contexts/EntriesContext';
import type { WorkspaceWithSession } from '@/types/attempt';
import VirtualizedList from './VirtualizedList';

describe('VirtualizedList', () => {
  beforeEach(() => {
    scrollToIndexMock.mockClear();
    vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
      cb(0);
      return 0;
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('preserves scroll anchor when older history is prepended', async () => {
    const now = new Date().toISOString();
    const attempt: WorkspaceWithSession = {
      id: 'workspace-1',
      task_id: 'task-1',
      container_ref: null,
      branch: 'main',
      agent_working_dir: null,
      setup_completed_at: null,
      created_at: now,
      updated_at: now,
      session: undefined,
    };

    render(
      <EntriesProvider>
        <VirtualizedList attempt={attempt} />
      </EntriesProvider>
    );

    await waitFor(() => {
      expect(screen.queryByText('Load earlier history')).not.toBeNull();
    });

    fireEvent.click(screen.getByText('Load earlier history'));

    await waitFor(() => {
      const calls = scrollToIndexMock.mock.calls.map((call) => call[0]);
      expect(calls).toEqual(
        expect.arrayContaining([expect.objectContaining({ index: 1 })])
      );
    });
  });
});
