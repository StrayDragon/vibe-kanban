import {
  ExecutionProcessStatus,
  type CommandExitStatus,
  type ExecutionProcessPublic as ExecutionProcess,
  type ExecutorAction,
  type NormalizedEntry,
  type PatchType,
  type ToolStatus,
} from 'shared/types';

export type PatchTypeWithKey = PatchType & {
  patchKey: string;
  executionProcessId: string;
};

export const patchWithKey = (
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

const isSyntheticEntry = (entry: PatchTypeWithKey) =>
  entry.type === 'NORMALIZED_ENTRY' &&
  (entry.content.entry_type.type === 'user_message' ||
    entry.content.entry_type.type === 'next_action' ||
    entry.content.entry_type.type === 'loading');

const countEntriesForLimit = (entries: PatchTypeWithKey[]) =>
  entries.reduce(
    (count, entry) => count + (isSyntheticEntry(entry) ? 0 : 1),
    0
  );

export type ConversationProcessDerived = {
  entriesForEmit: PatchTypeWithKey[];
  nonSyntheticCount: number;
  hasPendingApproval: boolean;
  hasSetupRequired: boolean;
  isRunning: boolean;
  failedOrKilled: boolean;
  showsLoadingIndicator: boolean;
};

export type ConversationProcessState = {
  executionProcess: {
    id: string;
    executor_action: ExecutorAction;
  };
  entries: PatchTypeWithKey[];
};

export const deriveConversationProcess = (
  processState: ConversationProcessState,
  liveProcess: ExecutionProcess | undefined
): ConversationProcessDerived => {
  const { executionProcess, entries } = processState;

  const liveStatus = liveProcess?.status;
  const isRunning = liveStatus === ExecutionProcessStatus.running;
  const failedOrKilled =
    liveStatus === ExecutionProcessStatus.failed ||
    liveStatus === ExecutionProcessStatus.killed;

  const derivedEntries: PatchTypeWithKey[] = [];
  let hasPendingApproval = false;
  let hasSetupRequired = false;
  let showsLoadingIndicator = false;

  if (
    executionProcess.executor_action.typ.type === 'CodingAgentInitialRequest' ||
    executionProcess.executor_action.typ.type === 'CodingAgentFollowUpRequest'
  ) {
    const userNormalizedEntry: NormalizedEntry = {
      entry_type: {
        type: 'user_message',
      },
      content: executionProcess.executor_action.typ.prompt,
      metadata: null,
      timestamp: null,
    };
    const userPatch: PatchType = {
      type: 'NORMALIZED_ENTRY',
      content: userNormalizedEntry,
    };
    derivedEntries.push(patchWithKey(userPatch, executionProcess.id, 'user'));

    const entriesExcludingUser = entries.filter(
      (e) =>
        e.type !== 'NORMALIZED_ENTRY' ||
        e.content.entry_type.type !== 'user_message'
    );

    hasPendingApproval = entriesExcludingUser.some((entry) => {
      if (entry.type !== 'NORMALIZED_ENTRY') return false;
      const entryType = entry.content.entry_type;
      return (
        entryType.type === 'tool_use' &&
        entryType.status.status === 'pending_approval'
      );
    });

    hasSetupRequired = entriesExcludingUser.some((entry) => {
      if (entry.type !== 'NORMALIZED_ENTRY') return false;
      return (
        entry.content.entry_type.type === 'error_message' &&
        entry.content.entry_type.error_type.type === 'setup_required'
      );
    });

    derivedEntries.push(...entriesExcludingUser);

    if (isRunning && !hasPendingApproval) {
      showsLoadingIndicator = true;
    }
  } else if (executionProcess.executor_action.typ.type === 'ScriptRequest') {
    let toolName = '';
    switch (executionProcess.executor_action.typ.context) {
      case 'SetupScript':
        toolName = 'Setup Script';
        break;
      case 'CleanupScript':
        toolName = 'Cleanup Script';
        break;
      case 'DevServer':
        toolName = 'Dev Server';
        break;
      case 'ToolInstallScript':
        toolName = 'Tool Install Script';
        break;
    }

    const exitCode = Number(liveProcess?.exit_code) || 0;
    const exit_status: CommandExitStatus | null =
      liveStatus === ExecutionProcessStatus.running
        ? null
        : {
            type: 'exit_code',
            code: exitCode,
          };

    const toolStatus: ToolStatus =
      liveStatus === ExecutionProcessStatus.running
        ? { status: 'created' }
        : exitCode === 0
          ? { status: 'success' }
          : { status: 'failed' };

    const output = entries.map((line) => line.content).join('\n');

    const toolNormalizedEntry: NormalizedEntry = {
      entry_type: {
        type: 'tool_use',
        tool_name: toolName,
        action_type: {
          action: 'command_run',
          command: executionProcess.executor_action.typ.script,
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
    derivedEntries.push(patchWithKey(toolPatch, executionProcess.id, 0));
  }

  const nonSyntheticCount = countEntriesForLimit(derivedEntries);

  return {
    entriesForEmit: derivedEntries,
    nonSyntheticCount,
    hasPendingApproval,
    hasSetupRequired,
    isRunning,
    failedOrKilled,
    showsLoadingIndicator,
  };
};
