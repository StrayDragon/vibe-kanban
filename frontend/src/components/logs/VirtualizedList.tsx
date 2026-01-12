import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';

import DisplayConversationEntry from '../NormalizedConversation/DisplayConversationEntry';
import { useEntries } from '@/contexts/EntriesContext';
import {
  AddEntryType,
  PatchTypeWithKey,
  useConversationHistory,
} from '@/hooks/useConversationHistory';
import { Loader2 } from 'lucide-react';
import { TaskWithAttemptStatus } from 'shared/types';
import type { WorkspaceWithSession } from '@/types/attempt';
import { ApprovalFormProvider } from '@/contexts/ApprovalFormContext';

interface VirtualizedListProps {
  attempt: WorkspaceWithSession;
  task?: TaskWithAttemptStatus;
}

interface MessageListContext {
  attempt: WorkspaceWithSession;
  task?: TaskWithAttemptStatus;
}

const ListHeader = () => <div className="h-2"></div>;
const ListFooter = () => <div className="h-2"></div>;

type PendingScrollAction =
  | {
      type: 'bottom';
      behavior?: 'auto' | 'smooth';
    }
  | {
      type: 'index';
      index: number;
      align: 'start' | 'center' | 'end';
      offset?: number;
      behavior?: 'auto' | 'smooth';
    };

const renderItem = (
  _index: number,
  data: PatchTypeWithKey,
  context: MessageListContext
) => {
  const attempt = context?.attempt;
  const task = context?.task;

  if (data.type === 'STDOUT') {
    return <p>{data.content}</p>;
  }
  if (data.type === 'STDERR') {
    return <p>{data.content}</p>;
  }
  if (data.type === 'NORMALIZED_ENTRY' && attempt) {
    return (
      <DisplayConversationEntry
        expansionKey={data.patchKey}
        entry={data.content}
        executionProcessId={data.executionProcessId}
        taskAttempt={attempt}
        task={task}
      />
    );
  }

  return null;
};

const computeItemKey = (_index: number, data: PatchTypeWithKey) =>
  `l-${data.patchKey}`;

const VirtualizedList = ({ attempt, task }: VirtualizedListProps) => {
  const [channelData, setChannelData] = useState<PatchTypeWithKey[]>([]);
  const [loading, setLoading] = useState(true);
  const [atBottom, setAtBottom] = useState(true);
  const { entries, setEntries, reset } = useEntries();
  const virtuosoRef = useRef<VirtuosoHandle | null>(null);
  const scrollerElementRef = useRef<HTMLElement | null>(null);
  const prevLengthRef = useRef<number>(0);
  const pendingHistoricAnchorRef = useRef<{
    key: string;
    offset: number;
    index: number;
  } | null>(null);
  const pendingResizeAnchorRef = useRef<{
    key: string;
    offset: number;
    index: number;
  } | null>(null);
  const pendingScrollActionRef = useRef<PendingScrollAction | null>(null);

  const isNearBottom = useCallback(() => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return true;
    const remaining =
      scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight;
    return remaining <= 48;
  }, []);

  const captureAnchor = useCallback((targetRef: {
    current: { key: string; offset: number; index: number } | null;
  }) => {
    const scroller = scrollerElementRef.current;
    if (!scroller || entries.length === 0) return;

    const scrollerRect = scroller.getBoundingClientRect();
    const items = Array.from(
      scroller.querySelectorAll<HTMLElement>('[data-index]')
    );

    if (items.length === 0) return;

    const visibleItems = items
      .map((el) => ({ el, rect: el.getBoundingClientRect() }))
      .filter(
        ({ rect }) =>
          rect.bottom > scrollerRect.top && rect.top < scrollerRect.bottom
      )
      .sort((a, b) => a.rect.top - b.rect.top);

    const anchorItem = visibleItems[0] ?? {
      el: items[0],
      rect: items[0].getBoundingClientRect(),
    };

    const indexAttr = anchorItem.el.getAttribute('data-index');
    const index = indexAttr ? Number(indexAttr) : Number.NaN;
    if (!Number.isFinite(index)) return;

    const entry = entries[index];
    if (!entry) return;

    targetRef.current = {
      key: entry.patchKey,
      offset: anchorItem.rect.top - scrollerRect.top,
      index,
    };
  }, [entries]);

  const captureHistoricAnchor = useCallback(() => {
    captureAnchor(pendingHistoricAnchorRef);
  }, [captureAnchor]);

  const captureResizeAnchor = useCallback(() => {
    captureAnchor(pendingResizeAnchorRef);
  }, [captureAnchor]);

  useEffect(() => {
    setLoading(true);
    setChannelData([]);
    prevLengthRef.current = 0;
    pendingHistoricAnchorRef.current = null;
    pendingResizeAnchorRef.current = null;
    pendingScrollActionRef.current = null;
    reset();
  }, [attempt.id, reset]);

  const onEntriesUpdated = (
    newEntries: PatchTypeWithKey[],
    addType: AddEntryType,
    newLoading: boolean
  ) => {
    const wasNearBottom = isNearBottom();
    const prevEntriesByKey = new Map(
      entries.map((entry) => [entry.patchKey, entry])
    );
    let minChangedIndex: number | null = null;
    newEntries.forEach((entry, index) => {
      const prevEntry = prevEntriesByKey.get(entry.patchKey);
      if (prevEntry && prevEntry !== entry) {
        minChangedIndex =
          minChangedIndex === null ? index : Math.min(minChangedIndex, index);
      }
    });
    const hasContentChanges = minChangedIndex !== null;
    const prevLen = prevLengthRef.current;
    const nextLen = newEntries.length;
    const appended = nextLen > prevLen;
    let pendingScrollAction: PendingScrollAction | null = null;
    const shouldCaptureResizeAnchor =
      addType === 'running' && hasContentChanges && !loading && !wasNearBottom;

    if (shouldCaptureResizeAnchor) {
      captureResizeAnchor();
    }

    if (addType === 'historic') {
      const anchor = pendingHistoricAnchorRef.current;
      if (anchor) {
        pendingHistoricAnchorRef.current = null;
        const anchorIndex = newEntries.findIndex(
          (entry) => entry.patchKey === anchor.key
        );
        if (anchorIndex >= 0) {
          pendingScrollAction = {
            type: 'index',
            index: anchorIndex,
            align: 'start',
            offset: anchor.offset,
            behavior: 'auto',
          };
        }
      }
    }

    if (loading || addType === 'initial') {
      pendingScrollAction = { type: 'bottom', behavior: 'auto' };
    } else if (shouldCaptureResizeAnchor) {
      const anchor = pendingResizeAnchorRef.current;
      const shouldPreserveAnchor =
        anchor &&
        minChangedIndex !== null &&
        minChangedIndex <= anchor.index;

      if (shouldPreserveAnchor) {
        pendingResizeAnchorRef.current = null;
        const anchorIndex = newEntries.findIndex(
          (entry) => entry.patchKey === anchor.key
        );
        if (anchorIndex >= 0) {
          pendingScrollAction = {
            type: 'index',
            index: anchorIndex,
            align: 'start',
            offset: anchor.offset,
            behavior: 'auto',
          };
        }
      } else if (anchor) {
        pendingResizeAnchorRef.current = null;
      }
    }

    if (
      !pendingScrollAction &&
      wasNearBottom &&
      appended &&
      !pendingHistoricAnchorRef.current
    ) {
      pendingScrollAction = { type: 'bottom', behavior: 'smooth' };
    }

    prevLengthRef.current = newEntries.length;

    setChannelData(newEntries);
    setEntries(newEntries);
    pendingScrollActionRef.current = pendingScrollAction;

    if (loading) {
      setLoading(newLoading);
    }
  };

  const { loadOlderHistory, hasMoreHistory, loadingOlder, historyTruncated } =
    useConversationHistory({ attempt, onEntriesUpdated });

  const handleLoadOlderHistory = useCallback(() => {
    if (loadingOlder) return;
    captureHistoricAnchor();
    void loadOlderHistory();
  }, [captureHistoricAnchor, loadOlderHistory, loadingOlder]);

  const messageListContext = useMemo(
    () => ({ attempt, task }),
    [attempt, task]
  );

  useLayoutEffect(() => {
    const action = pendingScrollActionRef.current;
    if (!action) return;
    pendingScrollActionRef.current = null;

    requestAnimationFrame(() => {
      if (!virtuosoRef.current || channelData.length === 0) return;
      if (action.type === 'bottom') {
        virtuosoRef.current.scrollToIndex({
          index: 'LAST',
          align: 'end',
          behavior: action.behavior ?? 'auto',
        });
        return;
      }
      virtuosoRef.current.scrollToIndex({
        index: action.index,
        align: action.align,
        offset: action.offset,
        behavior: action.behavior ?? 'auto',
      });
    });
  }, [channelData]);

  const handleScrollerRef = useCallback(
    (element: HTMLElement | Window | null) => {
      scrollerElementRef.current =
        element instanceof HTMLElement ? element : null;
    },
    []
  );
  const followOutput = pendingScrollActionRef.current
    ? false
    : atBottom
      ? 'smooth'
      : false;

  return (
    <ApprovalFormProvider>
      {(historyTruncated || hasMoreHistory) && !loading && (
        <div className="flex flex-col items-center gap-2 py-2 text-xs text-muted-foreground">
          {historyTruncated && (
            <span>Some earlier conversation history is unavailable.</span>
          )}
          {hasMoreHistory && (
            <button
              onClick={handleLoadOlderHistory}
              disabled={loadingOlder}
              className="flex items-center gap-1 px-2 py-1 rounded border border-border hover:text-foreground disabled:opacity-50"
            >
              {loadingOlder && <Loader2 className="h-3 w-3 animate-spin" />}
              Load earlier history
            </button>
          )}
        </div>
      )}
      <Virtuoso<PatchTypeWithKey, MessageListContext>
        ref={virtuosoRef}
        className="flex-1"
        data={channelData}
        alignToBottom
        context={messageListContext}
        computeItemKey={computeItemKey}
        itemContent={renderItem}
        components={{ Header: ListHeader, Footer: ListFooter }}
        atBottomStateChange={setAtBottom}
        atBottomThreshold={48}
        followOutput={followOutput}
        scrollerRef={handleScrollerRef}
      />
      {loading && (
        <div className="float-left top-0 left-0 w-full h-full bg-primary flex flex-col gap-2 justify-center items-center">
          <Loader2 className="h-8 w-8 animate-spin" />
          <p>Loading History</p>
        </div>
      )}
    </ApprovalFormProvider>
  );
};

export default VirtualizedList;
