// useConversationHistory.ts
import {
  ExecutionProcessPublic as ExecutionProcess,
  ExecutionProcessStatus,
  ExecutorAction,
  IndexedLogEntry,
  LogHistoryPage,
  PatchType,
  Workspace,
} from 'shared/types';
import { useExecutionProcessesContext } from '@/contexts/ExecutionProcessesContext';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { executionProcessesApi } from '@/lib/api';
import { openLogStream } from '@/realtime';
import {
  deriveConversationProcess,
  patchWithKey,
  type ConversationProcessDerived,
  type PatchTypeWithKey,
} from './conversationHistoryDerivation';

export type { PatchTypeWithKey } from './conversationHistoryDerivation';

export type AddEntryType = 'initial' | 'running' | 'historic';

export type OnEntriesUpdated = (
  newEntries: PatchTypeWithKey[],
  addType: AddEntryType,
  loading: boolean
) => void;

type ExecutionProcessStaticInfo = {
  id: string;
  created_at: string;
  updated_at: string;
  executor_action: ExecutorAction;
};

type ExecutionProcessState = {
  executionProcess: ExecutionProcessStaticInfo;
  entries: PatchTypeWithKey[];
  cursor: bigint | null;
  hasMore: boolean;
  historyTruncated: boolean;
};

type ExecutionProcessStateStore = Record<string, ExecutionProcessState>;

interface UseConversationHistoryParams {
  attempt: Workspace;
  onEntriesUpdated: OnEntriesUpdated;
}

interface UseConversationHistoryResult {
  loadOlderHistory: () => Promise<void>;
  hasMoreHistory: boolean;
  loadingOlder: boolean;
  historyTruncated: boolean;
}

const CONVERSATION_PAGE_SIZE = 20;
const CONVERSATION_CACHE_LIMIT = 20;
const CONVERSATION_CACHE_LIMIT_MAX = 2000;

const makeLoadingPatch = (executionProcessId: string): PatchTypeWithKey => ({
  type: 'NORMALIZED_ENTRY',
  content: {
    entry_type: {
      type: 'loading',
    },
    content: '',
    metadata: null,
    timestamp: null,
  },
  patchKey: `${executionProcessId}:loading`,
  executionProcessId,
});

const nextActionPatch: (
  failed: boolean,
  execution_processes: number,
  needs_setup: boolean
) => PatchTypeWithKey = (failed, execution_processes, needs_setup) => ({
  type: 'NORMALIZED_ENTRY',
  content: {
    entry_type: {
      type: 'next_action',
      failed: failed,
      execution_processes: execution_processes,
      needs_setup: needs_setup,
    },
    content: '',
    metadata: null,
    timestamp: null,
  },
  patchKey: 'next_action',
  executionProcessId: '',
});

const isScriptProcess = (executionProcess: ExecutionProcess) =>
  executionProcess.executor_action.typ.type === 'ScriptRequest';

const isUserMessagePatch = (patch: PatchType) =>
  patch.type === 'NORMALIZED_ENTRY' &&
  patch.content.entry_type.type === 'user_message';

const mapIndexedEntries = (
  entries: IndexedLogEntry[],
  executionProcessId: string
): PatchTypeWithKey[] =>
  entries
    .filter((entry) => !isUserMessagePatch(entry.entry))
    .map((entry) =>
      patchWithKey(entry.entry, executionProcessId, entry.entry_index)
    );

const entryIndexForSort = (entry: PatchTypeWithKey): number => {
  const parts = entry.patchKey.split(':');
  const index = Number(parts[1]);
  return Number.isFinite(index) ? index : -1;
};

const entryIndexForCursor = (entry: PatchTypeWithKey): bigint | null => {
  const parts = entry.patchKey.split(':');
  if (parts.length < 2) return null;
  try {
    return BigInt(parts[1]);
  } catch {
    return null;
  }
};

const minEntryIndex = (entries: PatchTypeWithKey[]): bigint | null => {
  let min: bigint | null = null;
  for (const entry of entries) {
    const index = entryIndexForCursor(entry);
    if (index === null) continue;
    min = min === null || index < min ? index : min;
  }
  return min;
};

const sortEntriesByIndex = (entries: PatchTypeWithKey[]) =>
  entries.sort((a, b) => entryIndexForSort(a) - entryIndexForSort(b));

const applyEntryLimitWithCursor = (
  entries: PatchTypeWithKey[],
  limit: number,
  cursor: bigint | null
): { entries: PatchTypeWithKey[]; trimmed: boolean; cursor: bigint | null } => {
  if (entries.length <= limit) {
    return { entries, trimmed: false, cursor };
  }
  const trimmedEntries = entries.slice(entries.length - limit);
  const minIndex = minEntryIndex(trimmedEntries);
  return { entries: trimmedEntries, trimmed: true, cursor: minIndex ?? cursor };
};

const mergeHistoryEntries = (
  existing: PatchTypeWithKey[],
  incoming: PatchTypeWithKey[],
  prepend: boolean
): PatchTypeWithKey[] => {
  const existingKeys = new Set(existing.map((entry) => entry.patchKey));
  const filtered = incoming.filter(
    (entry) => !existingKeys.has(entry.patchKey)
  );
  const merged = prepend
    ? [...filtered, ...existing]
    : [...existing, ...filtered];
  return merged;
};

const mergeCursorForRefresh = (
  current: bigint | null,
  next: bigint | null
): bigint | null => {
  if (next === null) return current;
  if (current === null) return next;
  return current < next ? current : next;
};

const fetchLogHistoryPage = async (
  executionProcess: ExecutionProcess,
  cursor: bigint | null
): Promise<LogHistoryPage> => {
  const params = { limit: CONVERSATION_PAGE_SIZE, cursor };
  return isScriptProcess(executionProcess)
    ? executionProcessesApi.getRawLogsPage(executionProcess.id, params)
    : executionProcessesApi.getNormalizedLogsPage(executionProcess.id, params);
};

export const useConversationHistory = ({
  attempt,
  onEntriesUpdated,
}: UseConversationHistoryParams): UseConversationHistoryResult => {
  const {
    executionProcessesVisibleSorted: executionProcessesRawSorted,
    executionProcessesByIdVisible: executionProcessesByIdRaw,
    isLoading: executionProcessesLoading,
  } = useExecutionProcessesContext();
  const executionProcesses = useRef<ExecutionProcess[]>(
    executionProcessesRawSorted
  );
  const executionProcessesById = useRef<Record<string, ExecutionProcess>>(
    executionProcessesByIdRaw
  );
  const displayedExecutionProcesses = useRef<ExecutionProcessStateStore>({});
  const derivedExecutionProcesses = useRef<
    Map<string, ConversationProcessDerived>
  >(new Map());
  const loadedInitialEntries = useRef(false);
  const initialLoadInFlightRef = useRef(false);
  const streamingProcessIdsRef = useRef<Set<string>>(new Set());
  const pendingHistoricLoadIdsRef = useRef<Set<string>>(new Set());
  const entryLimitRef = useRef(CONVERSATION_CACHE_LIMIT);
  const knownProcessIdsRef = useRef<Set<string>>(new Set());
  const hasSeededProcessIdsRef = useRef(false);
  const lastStatusByIdRef = useRef<Map<string, ExecutionProcessStatus>>(
    new Map()
  );
  const onEntriesUpdatedRef = useRef<OnEntriesUpdated | null>(null);
  const [hasMoreHistory, setHasMoreHistory] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [historyTruncated, setHistoryTruncated] = useState(false);
  const processCount = executionProcessesRawSorted?.length ?? 0;

  const mergeIntoDisplayed = (
    mutator: (state: ExecutionProcessStateStore) => void
  ) => {
    const state = displayedExecutionProcesses.current;
    mutator(state);
  };

  const refreshHasMoreHistory = useCallback(() => {
    const displayed = displayedExecutionProcesses.current;
    const displayedIds = new Set(Object.keys(displayed));
    const processesByAge = executionProcesses.current;

    if (processesByAge.length === 0) {
      setHasMoreHistory(false);
      return;
    }

    let oldestDisplayedIndex = -1;
    for (let idx = 0; idx < processesByAge.length; idx++) {
      if (displayedIds.has(processesByAge[idx].id)) {
        oldestDisplayedIndex = idx;
        break;
      }
    }

    if (oldestDisplayedIndex === -1) {
      setHasMoreHistory(processesByAge.length > 0);
      return;
    }

    const oldestDisplayed = processesByAge[oldestDisplayedIndex];
    const hasOlderHidden = oldestDisplayedIndex > 0;
    const hasMoreInOldest = displayed[oldestDisplayed.id]?.hasMore ?? false;

    setHasMoreHistory(hasOlderHidden || hasMoreInOldest);
  }, []);

  const refreshHistoryTruncated = useCallback(() => {
    const displayed = displayedExecutionProcesses.current;
    const truncated = Object.values(displayed).some(
      (process) => process.historyTruncated
    );
    setHistoryTruncated(truncated);
  }, []);

  useEffect(() => {
    onEntriesUpdatedRef.current = onEntriesUpdated;
  }, [onEntriesUpdated]);

  // Keep executionProcesses up to date
  useEffect(() => {
    executionProcesses.current = executionProcessesRawSorted.filter(
      (ep) =>
        ep.run_reason === 'setupscript' ||
        ep.run_reason === 'cleanupscript' ||
        ep.run_reason === 'codingagent'
    );
    executionProcessesById.current = executionProcessesByIdRaw;
  }, [executionProcessesRawSorted, executionProcessesByIdRaw]);

  const getActiveAgentProcesses = (): ExecutionProcess[] => {
    return (
      executionProcesses?.current.filter(
        (p) =>
          p.status === ExecutionProcessStatus.running &&
          p.run_reason !== 'devserver'
      ) ?? []
    );
  };

  const recomputeDerivedForProcess = useCallback((processId: string) => {
    const state = displayedExecutionProcesses.current[processId];
    if (!state) {
      derivedExecutionProcesses.current.delete(processId);
      return;
    }
    const live = executionProcessesById.current[processId];
    derivedExecutionProcesses.current.set(
      processId,
      deriveConversationProcess(state, live)
    );
  }, []);

  const flattenEntriesForEmit = useCallback(
    (executionProcessState: ExecutionProcessStateStore): PatchTypeWithKey[] => {
      const entries: PatchTypeWithKey[] = [];
      const derived = derivedExecutionProcesses.current;

      let hasPendingApproval = false;
      let hasRunningProcess = false;
      let lastProcessFailedOrKilled = false;
      let needsSetup = false;
      let loadingIndicatorProcessId: string | null = null;

      const displayedCount = Object.keys(executionProcessState).length;

      let lastDisplayedProcessId: string | null = null;
      for (let idx = executionProcesses.current.length - 1; idx >= 0; idx--) {
        const candidate = executionProcesses.current[idx];
        if (candidate && executionProcessState[candidate.id]) {
          lastDisplayedProcessId = candidate.id;
          break;
        }
      }

      for (const process of executionProcesses.current) {
        const processState = executionProcessState[process.id];
        if (!processState) continue;

        let next = derived.get(process.id);
        if (!next) {
          next = deriveConversationProcess(
            processState,
            executionProcessesById.current[process.id]
          );
          derived.set(process.id, next);
        }

        entries.push(...next.entriesForEmit);

        if (next.hasPendingApproval) hasPendingApproval = true;
        if (next.isRunning) hasRunningProcess = true;
        if (next.showsLoadingIndicator) {
          loadingIndicatorProcessId = process.id;
        }

        if (process.id === lastDisplayedProcessId && next.failedOrKilled) {
          lastProcessFailedOrKilled = true;
          if (next.hasSetupRequired) needsSetup = true;
        }
      }

      if (loadingIndicatorProcessId) {
        entries.push(makeLoadingPatch(loadingIndicatorProcessId));
      }

      if (!hasRunningProcess && !hasPendingApproval) {
        entries.push(
          nextActionPatch(lastProcessFailedOrKilled, displayedCount, needsSetup)
        );
      }

      return entries;
    },
    []
  );

  const trimOldestProcessesToLimit = useCallback(
    (executionProcessState: ExecutionProcessStateStore) => {
      const limit = entryLimitRef.current;
      const displayedIds = Object.keys(executionProcessState);
      if (displayedIds.length <= 1) return;

      let total = 0;
      for (const processId of displayedIds) {
        const d = derivedExecutionProcesses.current.get(processId);
        if (d) {
          total += d.nonSyntheticCount;
        } else {
          recomputeDerivedForProcess(processId);
          total +=
            derivedExecutionProcesses.current.get(processId)
              ?.nonSyntheticCount ?? 0;
        }
      }

      if (total <= limit) return;

      for (const process of executionProcesses.current) {
        if (Object.keys(executionProcessState).length <= 1) break;
        if (!executionProcessState[process.id]) continue;
        if (process.status === ExecutionProcessStatus.running) continue;

        const d = derivedExecutionProcesses.current.get(process.id);
        if (d) total -= d.nonSyntheticCount;

        delete executionProcessState[process.id];
        derivedExecutionProcesses.current.delete(process.id);

        if (total <= limit) break;
      }
    },
    [recomputeDerivedForProcess]
  );

  const emitEntries = useCallback(
    (
      executionProcessState: ExecutionProcessStateStore,
      addEntryType: AddEntryType,
      loading: boolean
    ) => {
      trimOldestProcessesToLimit(executionProcessState);
      const entries = flattenEntriesForEmit(executionProcessState);
      onEntriesUpdatedRef.current?.(entries, addEntryType, loading);
      refreshHasMoreHistory();
      refreshHistoryTruncated();
    },
    [
      flattenEntriesForEmit,
      refreshHasMoreHistory,
      refreshHistoryTruncated,
      trimOldestProcessesToLimit,
    ]
  );

  // Reset state when attempt changes
  useEffect(() => {
    displayedExecutionProcesses.current = {};
    derivedExecutionProcesses.current.clear();
    loadedInitialEntries.current = false;
    initialLoadInFlightRef.current = false;
    streamingProcessIdsRef.current.clear();
    pendingHistoricLoadIdsRef.current.clear();
    entryLimitRef.current = CONVERSATION_CACHE_LIMIT;
    knownProcessIdsRef.current.clear();
    hasSeededProcessIdsRef.current = false;
    lastStatusByIdRef.current.clear();
    emitEntries(displayedExecutionProcesses.current, 'initial', true);
  }, [attempt.id, emitEntries]);

  const loadEntriesForHistoricExecutionProcess = async (
    executionProcess: ExecutionProcess,
    cursor: bigint | null
  ): Promise<{
    page: LogHistoryPage | null;
    entriesWithKey: PatchTypeWithKey[];
  }> => {
    try {
      const page = await fetchLogHistoryPage(executionProcess, cursor);
      const entriesWithKey = mapIndexedEntries(
        page.entries,
        executionProcess.id
      );
      return { page, entriesWithKey };
    } catch (err) {
      console.warn(
        `Error loading entries for execution process ${executionProcess.id}`,
        err
      );
      return { page: null, entriesWithKey: [] };
    }
  };

  const ensureProcessVisible = useCallback(
    (p: ExecutionProcess) => {
      let created = false;
      mergeIntoDisplayed((state) => {
        if (!state[p.id]) {
          state[p.id] = {
            executionProcess: {
              id: p.id,
              created_at: p.created_at,
              updated_at: p.updated_at,
              executor_action: p.executor_action,
            },
            entries: [],
            cursor: null,
            hasMore: false,
            historyTruncated: false,
          };
          created = true;
        }
      });
      if (created) {
        recomputeDerivedForProcess(p.id);
      }
    },
    [recomputeDerivedForProcess]
  );

  const updateProcessHistory = useCallback(
    (
      executionProcess: ExecutionProcess,
      entriesWithKey: PatchTypeWithKey[],
      page: LogHistoryPage | null,
      prepend: boolean
    ) => {
      mergeIntoDisplayed((state) => {
        const existing = state[executionProcess.id] ?? {
          executionProcess: {
            id: executionProcess.id,
            created_at: executionProcess.created_at,
            updated_at: executionProcess.updated_at,
            executor_action: executionProcess.executor_action,
          },
          entries: [],
          cursor: null,
          hasMore: false,
          historyTruncated: false,
        };

        const mergedEntries = mergeHistoryEntries(
          existing.entries,
          entriesWithKey,
          prepend
        );
        const nextCursor = page?.next_cursor ?? existing.cursor;
        const limited = applyEntryLimitWithCursor(
          mergedEntries,
          entryLimitRef.current,
          nextCursor
        );

        state[executionProcess.id] = {
          executionProcess: existing.executionProcess,
          entries: limited.entries,
          cursor: limited.cursor,
          hasMore: (page?.has_more ?? existing.hasMore) || limited.trimmed,
          historyTruncated:
            existing.historyTruncated || (page?.history_truncated ?? false),
        };
      });
      recomputeDerivedForProcess(executionProcess.id);
    },
    [recomputeDerivedForProcess]
  );

  const mergeLatestHistory = useCallback(
    (
      executionProcess: ExecutionProcess,
      entriesWithKey: PatchTypeWithKey[],
      page: LogHistoryPage
    ) => {
      mergeIntoDisplayed((state) => {
        const existing = state[executionProcess.id] ?? {
          executionProcess: {
            id: executionProcess.id,
            created_at: executionProcess.created_at,
            updated_at: executionProcess.updated_at,
            executor_action: executionProcess.executor_action,
          },
          entries: [],
          cursor: null,
          hasMore: false,
          historyTruncated: false,
        };

        const mergedEntries = mergeHistoryEntries(
          existing.entries,
          entriesWithKey,
          false
        );
        const nextCursor = mergeCursorForRefresh(
          existing.cursor,
          page.next_cursor ?? null
        );
        const limited = applyEntryLimitWithCursor(
          mergedEntries,
          entryLimitRef.current,
          nextCursor
        );

        state[executionProcess.id] = {
          executionProcess: existing.executionProcess,
          entries: limited.entries,
          cursor: limited.cursor,
          hasMore: page.has_more || limited.trimmed,
          historyTruncated: existing.historyTruncated || page.history_truncated,
        };
      });
      recomputeDerivedForProcess(executionProcess.id);
    },
    [recomputeDerivedForProcess]
  );

  const refreshRunningHistory = useCallback(
    async (executionProcess: ExecutionProcess) => {
      const { page, entriesWithKey } =
        await loadEntriesForHistoricExecutionProcess(executionProcess, null);
      if (!page) {
        return;
      }
      mergeLatestHistory(executionProcess, entriesWithKey, page);
      emitEntries(displayedExecutionProcesses.current, 'running', false);
    },
    [emitEntries, mergeLatestHistory]
  );

  const attachLiveStream = useCallback(
    async (executionProcess: ExecutionProcess) => {
      return new Promise<void>((resolve, reject) => {
        const endpoint = isScriptProcess(executionProcess)
          ? 'raw-logs/v2/ws'
          : 'normalized-logs/v2/ws';

        let controller: ReturnType<typeof openLogStream> | null = null;
        let shouldCloseAfterInit = false;

        controller = openLogStream(
          `/api/execution-processes/${executionProcess.id}/${endpoint}`,
          {
            onOpen: () => {
              refreshRunningHistory(executionProcess).catch(() => null);
            },
            onAppend: (entryIndex, entry) => {
              if (isUserMessagePatch(entry)) return;
              const patch = patchWithKey(
                entry,
                executionProcess.id,
                entryIndex
              );
              mergeIntoDisplayed((state) => {
                const current = state[executionProcess.id];
                if (!current) {
                  return;
                }
                const existingIndex = current.entries.findIndex(
                  (e) => e.patchKey === patch.patchKey
                );
                if (existingIndex >= 0) {
                  current.entries[existingIndex] = patch;
                } else {
                  current.entries.push(patch);
                }
                if (existingIndex < 0 && current.entries.length > 1) {
                  const prevIndex = entryIndexForCursor(
                    current.entries[current.entries.length - 2]
                  );
                  const nextIndex =
                    typeof entryIndex === 'bigint'
                      ? entryIndex
                      : BigInt(entryIndex);
                  if (prevIndex !== null && prevIndex > nextIndex) {
                    current.entries = sortEntriesByIndex(current.entries);
                  }
                }
                const limited = applyEntryLimitWithCursor(
                  current.entries,
                  entryLimitRef.current,
                  current.cursor
                );
                current.entries = limited.entries;
                current.cursor = limited.cursor;
                current.hasMore = current.hasMore || limited.trimmed;
              });
              recomputeDerivedForProcess(executionProcess.id);
              emitEntries(
                displayedExecutionProcesses.current,
                'running',
                false
              );
            },
            onReplace: (entryIndex, entry) => {
              if (isUserMessagePatch(entry)) return;
              const patch = patchWithKey(
                entry,
                executionProcess.id,
                entryIndex
              );
              mergeIntoDisplayed((state) => {
                const current = state[executionProcess.id];
                if (!current) {
                  return;
                }
                const existingIndex = current.entries.findIndex(
                  (e) => e.patchKey === patch.patchKey
                );
                if (existingIndex >= 0) {
                  current.entries[existingIndex] = patch;
                } else {
                  current.entries.push(patch);
                }
                if (existingIndex < 0) {
                  current.entries = sortEntriesByIndex(current.entries);
                }
                const limited = applyEntryLimitWithCursor(
                  current.entries,
                  entryLimitRef.current,
                  current.cursor
                );
                current.entries = limited.entries;
                current.cursor = limited.cursor;
                current.hasMore = current.hasMore || limited.trimmed;
              });
              recomputeDerivedForProcess(executionProcess.id);
              emitEntries(
                displayedExecutionProcesses.current,
                'running',
                false
              );
            },
            onFinished: () => {
              emitEntries(
                displayedExecutionProcesses.current,
                'running',
                false
              );
              shouldCloseAfterInit = true;
              controller?.close();
              resolve();
            },
            onError: (err) => {
              shouldCloseAfterInit = true;
              controller?.close();
              reject(err);
            },
          }
        );

        if (shouldCloseAfterInit) {
          controller?.close();
        }
      });
    },
    [emitEntries, recomputeDerivedForProcess, refreshRunningHistory]
  );

  const loadRunningAndEmitWithBackoff = useCallback(
    async (executionProcess: ExecutionProcess) => {
      for (let i = 0; i < 20; i++) {
        try {
          await attachLiveStream(executionProcess);
          break;
        } catch (err) {
          if (
            typeof err === 'object' &&
            err !== null &&
            'code' in err &&
            (err as { code?: number }).code === 4404
          ) {
            await refreshRunningHistory(executionProcess).catch(() => null);
            break;
          }
          await new Promise((resolve) => setTimeout(resolve, 500));
        }
      }
    },
    [attachLiveStream, refreshRunningHistory]
  );

  const loadInitialEntries = useCallback(async () => {
    const localDisplayedExecutionProcesses: ExecutionProcessStateStore = {};

    if (!executionProcesses?.current) return localDisplayedExecutionProcesses;

    const derivedById = new Map<string, ConversationProcessDerived>();
    let totalNonSynthetic = 0;

    for (const executionProcess of [...executionProcesses.current].reverse()) {
      if (executionProcess.status === ExecutionProcessStatus.running) {
        continue;
      }
      if (pendingHistoricLoadIdsRef.current.has(executionProcess.id)) {
        continue;
      }
      pendingHistoricLoadIdsRef.current.add(executionProcess.id);

      let cursor: bigint | null = null;
      let hasMore = false;
      let entries: PatchTypeWithKey[] = [];
      let historyTruncated = false;

      try {
        do {
          const { page, entriesWithKey } =
            await loadEntriesForHistoricExecutionProcess(
              executionProcess,
              cursor
            );
          if (!page) {
            break;
          }
          if (entriesWithKey.length === 0) {
            break;
          }
          const mergedEntries = mergeHistoryEntries(
            entries,
            entriesWithKey,
            true
          );
          const limited = applyEntryLimitWithCursor(
            mergedEntries,
            entryLimitRef.current,
            page.next_cursor ?? cursor
          );
          entries = limited.entries;
          cursor = limited.cursor;
          hasMore = page.has_more || limited.trimmed;
          historyTruncated = historyTruncated || page.history_truncated;

          localDisplayedExecutionProcesses[executionProcess.id] = {
            executionProcess: {
              id: executionProcess.id,
              created_at: executionProcess.created_at,
              updated_at: executionProcess.updated_at,
              executor_action: executionProcess.executor_action,
            },
            entries,
            cursor,
            hasMore,
            historyTruncated,
          };

          const nextDerived = deriveConversationProcess(
            localDisplayedExecutionProcesses[executionProcess.id],
            executionProcess
          );
          const prevDerived = derivedById.get(executionProcess.id);
          if (prevDerived) {
            totalNonSynthetic -= prevDerived.nonSyntheticCount;
          }
          derivedById.set(executionProcess.id, nextDerived);
          totalNonSynthetic += nextDerived.nonSyntheticCount;

          if (totalNonSynthetic >= entryLimitRef.current) break;
        } while (hasMore);
      } finally {
        pendingHistoricLoadIdsRef.current.delete(executionProcess.id);
      }

      if (totalNonSynthetic >= entryLimitRef.current) break;
    }

    return localDisplayedExecutionProcesses;
  }, []);

  const loadOlderHistory = useCallback(async () => {
    if (loadingOlder) return;
    if (!executionProcesses?.current) return;

    setLoadingOlder(true);
    const processesByAge = executionProcesses.current;

    if (processesByAge.length === 0) {
      setLoadingOlder(false);
      return;
    }

    const displayedIds = new Set(
      Object.keys(displayedExecutionProcesses.current)
    );
    const oldestDisplayedIndex = processesByAge.findIndex((process) =>
      displayedIds.has(process.id)
    );

    let targetProcess: ExecutionProcess | null = null;
    let targetCursor: bigint | null = null;

    if (oldestDisplayedIndex > 0) {
      targetProcess = processesByAge[oldestDisplayedIndex - 1];
    } else if (oldestDisplayedIndex >= 0) {
      const oldestDisplayed = processesByAge[oldestDisplayedIndex];
      const current = displayedExecutionProcesses.current[oldestDisplayed.id];
      if (current?.hasMore) {
        targetProcess = oldestDisplayed;
        targetCursor = current.cursor;
      }
    }

    if (!targetProcess) {
      setLoadingOlder(false);
      return;
    }

    if (pendingHistoricLoadIdsRef.current.has(targetProcess.id)) {
      setLoadingOlder(false);
      return;
    }

    pendingHistoricLoadIdsRef.current.add(targetProcess.id);
    try {
      const { page, entriesWithKey } =
        await loadEntriesForHistoricExecutionProcess(
          targetProcess,
          targetCursor
        );
      if (page) {
        const addedLimit = entriesWithKey.length;
        if (addedLimit > 0) {
          entryLimitRef.current = Math.min(
            entryLimitRef.current + addedLimit,
            CONVERSATION_CACHE_LIMIT_MAX
          );
        }
        updateProcessHistory(targetProcess, entriesWithKey, page, true);
      }
    } finally {
      pendingHistoricLoadIdsRef.current.delete(targetProcess.id);
    }

    emitEntries(displayedExecutionProcesses.current, 'historic', false);
    setLoadingOlder(false);
  }, [executionProcesses, emitEntries, loadingOlder, updateProcessHistory]);

  const idListKey = useMemo(
    () => executionProcessesRawSorted.map((p) => p.id).join(','),
    [executionProcessesRawSorted]
  );

  const idStatusKey = useMemo(
    () =>
      executionProcessesRawSorted.map((p) => `${p.id}:${p.status}`).join(','),
    [executionProcessesRawSorted]
  );

  // Initial load when attempt changes
  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (
        processCount === 0 ||
        loadedInitialEntries.current ||
        initialLoadInFlightRef.current
      )
        return;

      initialLoadInFlightRef.current = true;
      try {
        const allInitialEntries = await loadInitialEntries();
        if (cancelled) return;
        mergeIntoDisplayed((state) => {
          Object.assign(state, allInitialEntries);
        });
        emitEntries(displayedExecutionProcesses.current, 'initial', false);
        loadedInitialEntries.current = true;
      } finally {
        initialLoadInFlightRef.current = false;
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [attempt.id, processCount, loadInitialEntries, emitEntries]);

  // Stop showing the loading overlay when there are no processes for this attempt.
  useEffect(() => {
    if (executionProcessesLoading) return;
    if (loadedInitialEntries.current) return;
    if (processCount > 0) return;

    loadedInitialEntries.current = true;
    emitEntries(displayedExecutionProcesses.current, 'initial', false);
  }, [attempt.id, executionProcessesLoading, processCount, emitEntries]);

  useEffect(() => {
    const activeProcesses = getActiveAgentProcesses();
    if (activeProcesses.length === 0) return;

    for (const activeProcess of activeProcesses) {
      if (!displayedExecutionProcesses.current[activeProcess.id]) {
        const runningOrInitial =
          Object.keys(displayedExecutionProcesses.current).length > 1
            ? 'running'
            : 'initial';
        ensureProcessVisible(activeProcess);
        emitEntries(
          displayedExecutionProcesses.current,
          runningOrInitial,
          false
        );
      }

      if (
        activeProcess.status === ExecutionProcessStatus.running &&
        !streamingProcessIdsRef.current.has(activeProcess.id)
      ) {
        streamingProcessIdsRef.current.add(activeProcess.id);
        loadRunningAndEmitWithBackoff(activeProcess).finally(() => {
          streamingProcessIdsRef.current.delete(activeProcess.id);
        });
      }
    }
  }, [
    attempt.id,
    idStatusKey,
    emitEntries,
    ensureProcessVisible,
    loadRunningAndEmitWithBackoff,
  ]);

  // Emit updates when process statuses change, even if no new log entries arrive.
  useEffect(() => {
    const changedDisplayedIds: string[] = [];
    const statusById = lastStatusByIdRef.current;
    for (const process of executionProcesses.current) {
      const prevStatus = statusById.get(process.id);
      if (prevStatus === process.status) continue;
      statusById.set(process.id, process.status);
      if (displayedExecutionProcesses.current[process.id]) {
        changedDisplayedIds.push(process.id);
      }
    }

    if (!loadedInitialEntries.current) return;
    changedDisplayedIds.forEach(recomputeDerivedForProcess);
    const hasRunningProcess = executionProcesses.current.some(
      (process) => process.status === ExecutionProcessStatus.running
    );
    const addType: AddEntryType = hasRunningProcess ? 'running' : 'historic';
    emitEntries(displayedExecutionProcesses.current, addType, false);
  }, [attempt.id, idStatusKey, emitEntries, recomputeDerivedForProcess]);

  useEffect(() => {
    if (executionProcessesLoading) return;
    if (hasSeededProcessIdsRef.current) return;

    executionProcesses.current.forEach((process) => {
      knownProcessIdsRef.current.add(process.id);
    });
    hasSeededProcessIdsRef.current = true;
  }, [executionProcessesLoading, idListKey]);

  // Load newly created non-running processes so recent messages appear promptly.
  useEffect(() => {
    if (!hasSeededProcessIdsRef.current) return;
    const currentProcesses = executionProcesses.current;
    if (!currentProcesses.length) return;

    const newProcesses = currentProcesses.filter(
      (process) => !knownProcessIdsRef.current.has(process.id)
    );

    if (newProcesses.length === 0) return;

    newProcesses.forEach((process) => {
      knownProcessIdsRef.current.add(process.id);
      ensureProcessVisible(process);
    });
    emitEntries(displayedExecutionProcesses.current, 'running', false);

    let cancelled = false;
    (async () => {
      for (const process of newProcesses) {
        if (cancelled) return;
        if (process.status === ExecutionProcessStatus.running) continue;
        if (pendingHistoricLoadIdsRef.current.has(process.id)) continue;
        pendingHistoricLoadIdsRef.current.add(process.id);

        try {
          const { page, entriesWithKey } =
            await loadEntriesForHistoricExecutionProcess(process, null);
          if (cancelled) return;
          updateProcessHistory(process, entriesWithKey, page, false);
          emitEntries(displayedExecutionProcesses.current, 'historic', false);
        } finally {
          pendingHistoricLoadIdsRef.current.delete(process.id);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [
    attempt.id,
    idListKey,
    emitEntries,
    ensureProcessVisible,
    updateProcessHistory,
  ]);

  // If an execution process is removed, remove it from the state
  useEffect(() => {
    const removedProcessIds = Object.keys(
      displayedExecutionProcesses.current
    ).filter((id) => !executionProcessesById.current[id]);

    if (removedProcessIds.length > 0) {
      mergeIntoDisplayed((state) => {
        removedProcessIds.forEach((id) => {
          delete state[id];
        });
      });
      removedProcessIds.forEach((id) => {
        derivedExecutionProcesses.current.delete(id);
      });
    }
  }, [attempt.id, idListKey]);

  return {
    loadOlderHistory,
    hasMoreHistory,
    loadingOlder,
    historyTruncated,
  };
};
