// useConversationHistory.ts
import {
  ApiResponse,
  CommandExitStatus,
  ExecutionProcess,
  ExecutionProcessStatus,
  ExecutorAction,
  IndexedLogEntry,
  LogHistoryPage,
  NormalizedEntry,
  PatchType,
  ToolStatus,
  Workspace,
} from 'shared/types';
import { useExecutionProcessesContext } from '@/contexts/ExecutionProcessesContext';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { streamLogEntries } from '@/utils/streamLogEntries';

export type PatchTypeWithKey = PatchType & {
  patchKey: string;
  executionProcessId: string;
};

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
}

const CONVERSATION_PAGE_SIZE = 20;
const CONVERSATION_CACHE_LIMIT = 200;
const CONVERSATION_CACHE_LIMIT_MAX = 2000;
const MIN_INITIAL_ENTRIES = 20;

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

const patchWithKey = (
  patch: PatchType,
  executionProcessId: string,
  index: bigint | number | 'user'
): PatchTypeWithKey => {
  return {
    ...patch,
    patchKey: `${executionProcessId}:${index}`,
    executionProcessId,
  };
};

const mapIndexedEntries = (
  entries: IndexedLogEntry[],
  executionProcessId: string
): PatchTypeWithKey[] =>
  entries.map((entry) =>
    patchWithKey(entry.entry, executionProcessId, entry.entry_index)
  );

const applyEntryLimit = (
  entries: PatchTypeWithKey[],
  limit: number
): PatchTypeWithKey[] => {
  if (entries.length <= limit) {
    return entries;
  }
  return entries.slice(entries.length - limit);
};

const entryIndexForSort = (entry: PatchTypeWithKey): number => {
  const parts = entry.patchKey.split(':');
  const index = Number(parts[1]);
  return Number.isFinite(index) ? index : -1;
};

const sortEntriesByIndex = (entries: PatchTypeWithKey[]) =>
  entries.sort((a, b) => entryIndexForSort(a) - entryIndexForSort(b));

const mergeHistoryEntries = (
  existing: PatchTypeWithKey[],
  incoming: PatchTypeWithKey[],
  prepend: boolean,
  limit: number
): PatchTypeWithKey[] => {
  const existingKeys = new Set(existing.map((entry) => entry.patchKey));
  const filtered = incoming.filter((entry) => !existingKeys.has(entry.patchKey));
  const merged = prepend ? [...filtered, ...existing] : [...existing, ...filtered];
  return applyEntryLimit(merged, limit);
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
  const endpoint = isScriptProcess(executionProcess)
    ? 'raw-logs/v2'
    : 'normalized-logs/v2';
  const params = new URLSearchParams();
  params.set('limit', String(CONVERSATION_PAGE_SIZE));
  if (cursor !== null) {
    params.set('cursor', String(cursor));
  }

  const res = await fetch(
    `/api/execution-processes/${executionProcess.id}/${endpoint}?${params.toString()}`
  );
  if (!res.ok) {
    throw new Error(`Failed to load logs for ${executionProcess.id}`);
  }
  const body = (await res.json()) as ApiResponse<LogHistoryPage>;
  if (!body.data) {
    throw new Error(`No log history returned for ${executionProcess.id}`);
  }
  return body.data;
};

export const useConversationHistory = ({
  attempt,
  onEntriesUpdated,
}: UseConversationHistoryParams): UseConversationHistoryResult => {
  const {
    executionProcessesVisible: executionProcessesRaw,
    isLoading: executionProcessesLoading,
  } = useExecutionProcessesContext();
  const executionProcesses = useRef<ExecutionProcess[]>(executionProcessesRaw);
  const displayedExecutionProcesses = useRef<ExecutionProcessStateStore>({});
  const loadedInitialEntries = useRef(false);
  const streamingProcessIdsRef = useRef<Set<string>>(new Set());
  const pendingHistoricLoadIdsRef = useRef<Set<string>>(new Set());
  const entryLimitRef = useRef(CONVERSATION_CACHE_LIMIT);
  const knownProcessIdsRef = useRef<Set<string>>(new Set());
  const hasSeededProcessIdsRef = useRef(false);
  const onEntriesUpdatedRef = useRef<OnEntriesUpdated | null>(null);
  const [hasMoreHistory, setHasMoreHistory] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);

  const mergeIntoDisplayed = (
    mutator: (state: ExecutionProcessStateStore) => void
  ) => {
    const state = displayedExecutionProcesses.current;
    mutator(state);
  };

  const refreshHasMoreHistory = useCallback(() => {
    const displayed = displayedExecutionProcesses.current;
    const hasMore = Object.values(displayed).some((process) => process.hasMore);
    const displayedIds = new Set(Object.keys(displayed));
    const hasHiddenProcesses = executionProcesses.current.some(
      (process) =>
        !displayedIds.has(process.id) &&
        process.status !== ExecutionProcessStatus.running
    );
    setHasMoreHistory(hasMore || hasHiddenProcesses);
  }, []);

  useEffect(() => {
    onEntriesUpdatedRef.current = onEntriesUpdated;
  }, [onEntriesUpdated]);

  // Keep executionProcesses up to date
  useEffect(() => {
    executionProcesses.current = executionProcessesRaw.filter(
      (ep) =>
        ep.run_reason === 'setupscript' ||
        ep.run_reason === 'cleanupscript' ||
        ep.run_reason === 'codingagent'
    );
  }, [executionProcessesRaw]);

  const flattenEntries = (
    executionProcessState: ExecutionProcessStateStore
  ): PatchTypeWithKey[] => {
    return Object.values(executionProcessState)
      .filter(
        (p) =>
          p.executionProcess.executor_action.typ.type ===
            'CodingAgentFollowUpRequest' ||
          p.executionProcess.executor_action.typ.type ===
            'CodingAgentInitialRequest'
      )
      .sort(
        (a, b) =>
          new Date(
            a.executionProcess.created_at as unknown as string
          ).getTime() -
          new Date(
            b.executionProcess.created_at as unknown as string
          ).getTime()
      )
      .flatMap((p) => p.entries);
  };

  const getActiveAgentProcesses = (): ExecutionProcess[] => {
    return (
      executionProcesses?.current.filter(
        (p) =>
          p.status === ExecutionProcessStatus.running &&
          p.run_reason !== 'devserver'
      ) ?? []
    );
  };

  const flattenEntriesForEmit = useCallback(
    (executionProcessState: ExecutionProcessStateStore): PatchTypeWithKey[] => {
      // Flags to control Next Action bar emit
      let hasPendingApproval = false;
      let hasRunningProcess = false;
      let lastProcessFailedOrKilled = false;
      let needsSetup = false;

      // Create user messages + tool calls for setup/cleanup scripts
      const allEntries = Object.values(executionProcessState)
        .sort(
          (a, b) =>
            new Date(
              a.executionProcess.created_at as unknown as string
            ).getTime() -
            new Date(
              b.executionProcess.created_at as unknown as string
            ).getTime()
        )
        .flatMap((p, index) => {
          const entries: PatchTypeWithKey[] = [];
          if (
            p.executionProcess.executor_action.typ.type ===
              'CodingAgentInitialRequest' ||
            p.executionProcess.executor_action.typ.type ===
              'CodingAgentFollowUpRequest'
          ) {
            // New user message
            const userNormalizedEntry: NormalizedEntry = {
              entry_type: {
                type: 'user_message',
              },
              content: p.executionProcess.executor_action.typ.prompt,
              metadata: null,
              timestamp: null,
            };
            const userPatch: PatchType = {
              type: 'NORMALIZED_ENTRY',
              content: userNormalizedEntry,
            };
            const userPatchTypeWithKey = patchWithKey(
              userPatch,
              p.executionProcess.id,
              'user'
            );
            entries.push(userPatchTypeWithKey);

            // Remove all coding agent added user messages, replace with our custom one
            const entriesExcludingUser = p.entries.filter(
              (e) =>
                e.type !== 'NORMALIZED_ENTRY' ||
                e.content.entry_type.type !== 'user_message'
            );

            const hasPendingApprovalEntry = entriesExcludingUser.some(
              (entry) => {
                if (entry.type !== 'NORMALIZED_ENTRY') return false;
                const entryType = entry.content.entry_type;
                return (
                  entryType.type === 'tool_use' &&
                  entryType.status.status === 'pending_approval'
                );
              }
            );

            if (hasPendingApprovalEntry) {
              hasPendingApproval = true;
            }

            entries.push(...entriesExcludingUser);

            const liveProcessStatus = getLiveExecutionProcess(
              p.executionProcess.id
            )?.status;
            const isProcessRunning =
              liveProcessStatus === ExecutionProcessStatus.running;
            const processFailedOrKilled =
              liveProcessStatus === ExecutionProcessStatus.failed ||
              liveProcessStatus === ExecutionProcessStatus.killed;

            if (isProcessRunning) {
              hasRunningProcess = true;
            }

            if (
              processFailedOrKilled &&
              index === Object.keys(executionProcessState).length - 1
            ) {
              lastProcessFailedOrKilled = true;

              // Check if this failed process has a SetupRequired entry
              const hasSetupRequired = entriesExcludingUser.some((entry) => {
                if (entry.type !== 'NORMALIZED_ENTRY') return false;
                return (
                  entry.content.entry_type.type === 'error_message' &&
                  entry.content.entry_type.error_type.type === 'setup_required'
                );
              });

              if (hasSetupRequired) {
                needsSetup = true;
              }
            }

            if (isProcessRunning && !hasPendingApprovalEntry) {
              entries.push(makeLoadingPatch(p.executionProcess.id));
            }
          } else if (
            p.executionProcess.executor_action.typ.type === 'ScriptRequest'
          ) {
            // Add setup and cleanup script as a tool call
            let toolName = '';
            switch (p.executionProcess.executor_action.typ.context) {
              case 'SetupScript':
                toolName = 'Setup Script';
                break;
              case 'CleanupScript':
                toolName = 'Cleanup Script';
                break;
              case 'ToolInstallScript':
                toolName = 'Tool Install Script';
                break;
              default:
                return [];
            }

            const executionProcess = getLiveExecutionProcess(
              p.executionProcess.id
            );

            if (executionProcess?.status === ExecutionProcessStatus.running) {
              hasRunningProcess = true;
            }

            if (
              (executionProcess?.status === ExecutionProcessStatus.failed ||
                executionProcess?.status === ExecutionProcessStatus.killed) &&
              index === Object.keys(executionProcessState).length - 1
            ) {
              lastProcessFailedOrKilled = true;
            }

            const exitCode = Number(executionProcess?.exit_code) || 0;
            const exit_status: CommandExitStatus | null =
              executionProcess?.status === 'running'
                ? null
                : {
                    type: 'exit_code',
                    code: exitCode,
                  };

            const toolStatus: ToolStatus =
              executionProcess?.status === ExecutionProcessStatus.running
                ? { status: 'created' }
                : exitCode === 0
                  ? { status: 'success' }
                  : { status: 'failed' };

            const output = p.entries.map((line) => line.content).join('\n');

            const toolNormalizedEntry: NormalizedEntry = {
              entry_type: {
                type: 'tool_use',
                tool_name: toolName,
                action_type: {
                  action: 'command_run',
                  command: p.executionProcess.executor_action.typ.script,
                  result: {
                    output,
                    exit_status,
                  },
                },
                status: toolStatus,
              },
              content: toolName,
              metadata: null,
              timestamp: null,
            };
            const toolPatch: PatchType = {
              type: 'NORMALIZED_ENTRY',
              content: toolNormalizedEntry,
            };
            const toolPatchWithKey: PatchTypeWithKey = patchWithKey(
              toolPatch,
              p.executionProcess.id,
              0
            );

            entries.push(toolPatchWithKey);
          }

          return entries;
        });

      // Emit the next action bar if no process running
      if (!hasRunningProcess && !hasPendingApproval) {
        allEntries.push(
          nextActionPatch(
            lastProcessFailedOrKilled,
            Object.keys(executionProcessState).length,
            needsSetup
          )
        );
      }

      return allEntries;
    },
    []
  );

  const emitEntries = useCallback(
    (
      executionProcessState: ExecutionProcessStateStore,
      addEntryType: AddEntryType,
      loading: boolean
    ) => {
      const entries = flattenEntriesForEmit(executionProcessState);
      onEntriesUpdatedRef.current?.(entries, addEntryType, loading);
      refreshHasMoreHistory();
    },
    [flattenEntriesForEmit, refreshHasMoreHistory]
  );

  const getLiveExecutionProcess = (
    executionProcessId: string
  ): ExecutionProcess | undefined => {
    return executionProcesses?.current.find(
      (executionProcess) => executionProcess.id === executionProcessId
    );
  };

const loadEntriesForHistoricExecutionProcess = async (
  executionProcess: ExecutionProcess,
  cursor: bigint | null
): Promise<{ page: LogHistoryPage | null; entriesWithKey: PatchTypeWithKey[] }> => {
    try {
      const page = await fetchLogHistoryPage(executionProcess, cursor);
      const entriesWithKey = mapIndexedEntries(page.entries, executionProcess.id);
      return { page, entriesWithKey };
    } catch (err) {
      console.warn(
        `Error loading entries for execution process ${executionProcess.id}`,
        err
      );
      return { page: null, entriesWithKey: [] };
    }
  };

  const ensureProcessVisible = useCallback((p: ExecutionProcess) => {
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
        };
      }
    });
  }, []);

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
        };

        const mergedEntries = mergeHistoryEntries(
          existing.entries,
          entriesWithKey,
          prepend,
          entryLimitRef.current
        );

        state[executionProcess.id] = {
          executionProcess: existing.executionProcess,
          entries: mergedEntries,
          cursor: page?.next_cursor ?? existing.cursor,
          hasMore: page?.has_more ?? existing.hasMore,
        };
      });
    },
    []
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
        };

        const mergedEntries = mergeHistoryEntries(
          existing.entries,
          entriesWithKey,
          false,
          entryLimitRef.current
        );

        state[executionProcess.id] = {
          executionProcess: existing.executionProcess,
          entries: mergedEntries,
          cursor: mergeCursorForRefresh(existing.cursor, page.next_cursor ?? null),
          hasMore: existing.hasMore || page.has_more,
        };
      });
    },
    []
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

        const controller = streamLogEntries(
          `/api/execution-processes/${executionProcess.id}/${endpoint}`,
          {
            onOpen: () => {
              refreshRunningHistory(executionProcess).catch(() => null);
            },
            onAppend: (entryIndex, entry) => {
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
                current.entries = sortEntriesByIndex(current.entries);
                current.entries = applyEntryLimit(
                  current.entries,
                  entryLimitRef.current
                );
              });
              emitEntries(displayedExecutionProcesses.current, 'running', false);
            },
            onReplace: (entryIndex, entry) => {
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
                current.entries = sortEntriesByIndex(current.entries);
                current.entries = applyEntryLimit(
                  current.entries,
                  entryLimitRef.current
                );
              });
              emitEntries(displayedExecutionProcesses.current, 'running', false);
            },
            onFinished: () => {
              emitEntries(displayedExecutionProcesses.current, 'running', false);
              controller.close();
              resolve();
            },
            onError: () => {
              controller.close();
              reject();
            },
          }
        );
      });
    },
    [emitEntries, refreshRunningHistory]
  );

  const loadRunningAndEmitWithBackoff = useCallback(
    async (executionProcess: ExecutionProcess) => {
      for (let i = 0; i < 20; i++) {
        try {
          await attachLiveStream(executionProcess);
          break;
        } catch (_) {
          await new Promise((resolve) => setTimeout(resolve, 500));
        }
      }
    },
    [attachLiveStream]
  );

  const loadInitialEntries = useCallback(async () => {
    const localDisplayedExecutionProcesses: ExecutionProcessStateStore = {};

    if (!executionProcesses?.current) return localDisplayedExecutionProcesses;

    for (const executionProcess of [...executionProcesses.current].reverse()) {
      if (executionProcess.status === ExecutionProcessStatus.running) {
        continue;
      }
      if (pendingHistoricLoadIdsRef.current.has(executionProcess.id)) {
        continue;
      }
      pendingHistoricLoadIdsRef.current.add(executionProcess.id);

      try {
        const { page, entriesWithKey } =
          await loadEntriesForHistoricExecutionProcess(
            executionProcess,
            null
          );
        localDisplayedExecutionProcesses[executionProcess.id] = {
          executionProcess: {
            id: executionProcess.id,
            created_at: executionProcess.created_at,
            updated_at: executionProcess.updated_at,
            executor_action: executionProcess.executor_action,
          },
          entries: entriesWithKey,
          cursor: page?.next_cursor ?? null,
          hasMore: page?.has_more ?? false,
        };
      } finally {
        pendingHistoricLoadIdsRef.current.delete(executionProcess.id);
      }

      if (flattenEntries(localDisplayedExecutionProcesses).length > MIN_INITIAL_ENTRIES) {
        break;
      }
    }

    return localDisplayedExecutionProcesses;
  }, [executionProcesses]);

  const loadOlderHistory = useCallback(async () => {
    if (loadingOlder) return;
    if (!executionProcesses?.current) return;

    setLoadingOlder(true);
    const processesByAge = [...executionProcesses.current].sort(
      (a, b) =>
        new Date(a.created_at as unknown as string).getTime() -
        new Date(b.created_at as unknown as string).getTime()
    );

    let addedEntries = 0;

    for (const process of processesByAge) {
      if (process.status === ExecutionProcessStatus.running) {
        continue;
      }
      if (pendingHistoricLoadIdsRef.current.has(process.id)) {
        continue;
      }

      const current = displayedExecutionProcesses.current[process.id];
      if (!current) {
        pendingHistoricLoadIdsRef.current.add(process.id);
        try {
          const { page, entriesWithKey } =
            await loadEntriesForHistoricExecutionProcess(process, null);
          if (page) {
            if (entriesWithKey.length > 0) {
              entryLimitRef.current = Math.min(
                entryLimitRef.current + entriesWithKey.length,
                CONVERSATION_CACHE_LIMIT_MAX
              );
            }
            updateProcessHistory(process, entriesWithKey, page, true);
            addedEntries += entriesWithKey.length;
          }
        } finally {
          pendingHistoricLoadIdsRef.current.delete(process.id);
        }
      } else if (current.hasMore) {
        pendingHistoricLoadIdsRef.current.add(process.id);
        try {
          const { page, entriesWithKey } =
            await loadEntriesForHistoricExecutionProcess(
              process,
              current.cursor
            );
          if (page) {
            if (entriesWithKey.length > 0) {
              entryLimitRef.current = Math.min(
                entryLimitRef.current + entriesWithKey.length,
                CONVERSATION_CACHE_LIMIT_MAX
              );
            }
            updateProcessHistory(process, entriesWithKey, page, true);
            addedEntries += entriesWithKey.length;
          }
        } finally {
          pendingHistoricLoadIdsRef.current.delete(process.id);
        }
      }

      if (addedEntries >= CONVERSATION_PAGE_SIZE) {
        break;
      }
    }

    emitEntries(displayedExecutionProcesses.current, 'historic', false);
    setLoadingOlder(false);
  }, [
    executionProcesses,
    emitEntries,
    loadingOlder,
    updateProcessHistory,
  ]);

  const idListKey = useMemo(
    () => executionProcessesRaw?.map((p) => p.id).join(','),
    [executionProcessesRaw]
  );

  const idStatusKey = useMemo(
    () => executionProcessesRaw?.map((p) => `${p.id}:${p.status}`).join(','),
    [executionProcessesRaw]
  );

  // Initial load when attempt changes
  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (
        executionProcesses?.current.length === 0 ||
        loadedInitialEntries.current
      )
        return;

      const allInitialEntries = await loadInitialEntries();
      if (cancelled) return;
      mergeIntoDisplayed((state) => {
        Object.assign(state, allInitialEntries);
      });
      emitEntries(displayedExecutionProcesses.current, 'initial', false);
      loadedInitialEntries.current = true;
    })();
    return () => {
      cancelled = true;
    };
  }, [attempt.id, idListKey, loadInitialEntries, emitEntries]);

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
        emitEntries(displayedExecutionProcesses.current, runningOrInitial, false);
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
    if (!loadedInitialEntries.current) return;
    const hasRunningProcess = executionProcesses.current.some(
      (process) => process.status === ExecutionProcessStatus.running
    );
    const addType: AddEntryType = hasRunningProcess ? 'running' : 'historic';
    emitEntries(displayedExecutionProcesses.current, addType, false);
  }, [attempt.id, idStatusKey, emitEntries]);

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
  }, [attempt.id, idListKey, emitEntries, ensureProcessVisible, updateProcessHistory]);

  // If an execution process is removed, remove it from the state
  useEffect(() => {
    if (!executionProcessesRaw) return;

    const removedProcessIds = Object.keys(
      displayedExecutionProcesses.current
    ).filter((id) => !executionProcessesRaw.some((p) => p.id === id));

    if (removedProcessIds.length > 0) {
      mergeIntoDisplayed((state) => {
        removedProcessIds.forEach((id) => {
          delete state[id];
        });
      });
    }
  }, [attempt.id, idListKey, executionProcessesRaw]);

  // Reset state when attempt changes
  useEffect(() => {
    displayedExecutionProcesses.current = {};
    loadedInitialEntries.current = false;
    streamingProcessIdsRef.current.clear();
    pendingHistoricLoadIdsRef.current.clear();
    entryLimitRef.current = CONVERSATION_CACHE_LIMIT;
    knownProcessIdsRef.current.clear();
    hasSeededProcessIdsRef.current = false;
    emitEntries(displayedExecutionProcesses.current, 'initial', true);
  }, [attempt.id, emitEntries]);

  return { loadOlderHistory, hasMoreHistory, loadingOlder };
};
