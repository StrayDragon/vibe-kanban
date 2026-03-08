import type {
  ProjectExecutionMode,
  TaskAutomationReasonCode,
  TaskDispatchState,
  TaskWithAttemptStatus,
  Workspace,
  WorkspaceLifecycleHookPhase,
  WorkspaceLifecycleHookRunSummary,
  WorkspaceLifecycleHookStatus,
} from 'shared/types';

export type AutomationBadgeVariant =
  | 'default'
  | 'secondary'
  | 'destructive'
  | 'outline';

export type OrchestrationFilter =
  | 'all'
  | 'manual'
  | 'managed'
  | 'needs_review'
  | 'blocked';

export type OrchestrationLane = Exclude<OrchestrationFilter, 'all'>;

export const ORCHESTRATION_LANES: readonly OrchestrationLane[] = [
  'manual',
  'managed',
  'needs_review',
  'blocked',
] as const;

export interface AutomationPresentation {
  label: string;
  variant: AutomationBadgeVariant;
  detail?: string;
  reasonCode?: TaskAutomationReasonCode;
  actionable?: boolean;
  className?: string;
  kind?: 'manual' | 'managed' | 'needs_review' | 'blocked';
}

export interface OrchestrationSummary {
  all: number;
  manual: number;
  managed: number;
  needs_review: number;
  blocked: number;
}

const MANUAL_BADGE_CLASS =
  'border-slate-500/30 bg-slate-500/10 text-slate-700 dark:text-slate-200';
const MANAGED_BADGE_CLASS =
  'border-sky-500/30 bg-sky-500/15 text-sky-700 dark:text-sky-200';
const REVIEW_BADGE_CLASS =
  'border-amber-500/30 bg-amber-500/15 text-amber-700 dark:text-amber-200';
const BLOCKED_BADGE_CLASS =
  'border-destructive/30 bg-destructive/10 text-destructive';
const RUNNING_BADGE_CLASS =
  'border-emerald-500/30 bg-emerald-500/15 text-emerald-700 dark:text-emerald-200';
const QUEUED_BADGE_CLASS =
  'border-violet-500/30 bg-violet-500/15 text-violet-700 dark:text-violet-200';

export function getProjectExecutionModeLabel(
  mode: ProjectExecutionMode
): string {
  return mode === 'auto' ? 'Auto-managed' : 'Manual';
}

export function getTaskAutomationModeLabel(
  task: Pick<
    TaskWithAttemptStatus,
    'automation_mode' | 'project_execution_mode' | 'effective_automation_mode'
  >
): string {
  if (task.automation_mode === 'manual') {
    return 'Manual Override';
  }
  if (
    task.automation_mode === 'auto' &&
    task.project_execution_mode !== 'auto'
  ) {
    return 'Auto Override';
  }
  if (task.automation_mode === 'auto') {
    return 'Auto Managed';
  }
  return task.effective_automation_mode === 'auto'
    ? 'Inherit Auto'
    : 'Inherit Manual';
}

type WorkspaceHookAware = Pick<
  Workspace,
  | 'latest_hook_run'
  | 'after_prepare_hook_status'
  | 'after_prepare_hook_ran_at'
  | 'after_prepare_hook_error_summary'
  | 'before_cleanup_hook_status'
  | 'before_cleanup_hook_ran_at'
  | 'before_cleanup_hook_error_summary'
>;

export interface WorkspaceHookSnapshot {
  phase: WorkspaceLifecycleHookPhase;
  status: WorkspaceLifecycleHookStatus;
  ran_at: Date | string;
  error_summary: string | null;
}

function toHookTimestamp(value: Date | string | null | undefined): number {
  if (!value) return 0;
  const timestamp = new Date(value).getTime();
  return Number.isFinite(timestamp) ? timestamp : 0;
}

export function getWorkspaceHookSnapshots(
  workspace: WorkspaceHookAware | null | undefined
): WorkspaceHookSnapshot[] {
  if (!workspace) {
    return [];
  }

  const snapshots: WorkspaceHookSnapshot[] = [];

  if (workspace.after_prepare_hook_status && workspace.after_prepare_hook_ran_at) {
    snapshots.push({
      phase: 'after_prepare',
      status: workspace.after_prepare_hook_status,
      ran_at: workspace.after_prepare_hook_ran_at,
      error_summary: workspace.after_prepare_hook_error_summary ?? null,
    });
  }

  if (workspace.before_cleanup_hook_status && workspace.before_cleanup_hook_ran_at) {
    snapshots.push({
      phase: 'before_cleanup',
      status: workspace.before_cleanup_hook_status,
      ran_at: workspace.before_cleanup_hook_ran_at,
      error_summary: workspace.before_cleanup_hook_error_summary ?? null,
    });
  }

  return snapshots.sort(
    (left, right) =>
      toHookTimestamp(right.ran_at) - toHookTimestamp(left.ran_at)
  );
}

export function getWorkspaceHookOutcome(
  workspace: WorkspaceHookAware | null | undefined
): WorkspaceLifecycleHookRunSummary | null {
  if (!workspace) {
    return null;
  }

  if (workspace.latest_hook_run) {
    return workspace.latest_hook_run;
  }

  const latestSnapshot = getWorkspaceHookSnapshots(workspace)[0];
  if (!latestSnapshot) {
    return null;
  }

  return {
    phase: latestSnapshot.phase,
    status: latestSnapshot.status,
    ran_at: latestSnapshot.ran_at as Date,
    error_summary: latestSnapshot.error_summary,
  };
}

export function hasWorkspaceHookOutcome(
  workspace: WorkspaceHookAware | null | undefined
): boolean {
  return getWorkspaceHookOutcome(workspace) !== null;
}

function formatRetryDetail(state: TaskDispatchState): string | undefined {
  if (state.status === 'retry_scheduled' && state.next_retry_at) {
    return `Retry after ${new Date(state.next_retry_at).toLocaleString()}`;
  }
  if (state.status === 'blocked') {
    return (
      state.blocked_reason ?? state.last_error ?? 'Auto orchestration blocked'
    );
  }
  if (state.status === 'awaiting_human_review') {
    return 'Waiting for a human review decision';
  }
  return undefined;
}

function isBlockedReason(
  reasonCode?: TaskAutomationReasonCode | null
): boolean {
  return (
    reasonCode === 'retry_exhausted' ||
    reasonCode === 'no_project_repos' ||
    reasonCode === 'base_branch_unresolved' ||
    reasonCode === 'workspace_hook_failed' ||
    reasonCode === 'blocked'
  );
}

function getManualOwnershipDetail(task: TaskWithAttemptStatus): string {
  if (task.automation_mode === 'manual') {
    return 'This task stays manual until someone re-enables automation.';
  }
  if (task.project_execution_mode === 'manual') {
    return 'This task follows the project’s manual execution mode.';
  }
  return 'This task is currently under manual control.';
}

function getManagedOwnershipDetail(task: TaskWithAttemptStatus): string {
  if (
    task.automation_mode === 'auto' &&
    task.project_execution_mode !== 'auto'
  ) {
    return 'This task opted into auto-management even though the project stays manual by default.';
  }
  if (task.automation_mode === 'auto') {
    return 'This task is explicitly marked for auto-management.';
  }
  return 'This task follows the project’s auto-managed execution mode.';
}

function presentationFromDiagnostic(
  task: TaskWithAttemptStatus
): AutomationPresentation | null {
  const diagnostic = task.automation_diagnostic;
  if (!diagnostic) {
    return null;
  }

  const base = {
    detail: diagnostic.reason_detail,
    reasonCode: diagnostic.reason_code,
    actionable: diagnostic.actionable,
  };

  switch (diagnostic.reason_code) {
    case 'project_manual':
    case 'task_manual_override':
      return {
        label: getTaskAutomationModeLabel(task),
        variant: 'outline',
        className: MANUAL_BADGE_CLASS,
        ...base,
      };
    case 'task_group_unsupported':
      return {
        label: 'Unsupported',
        variant: 'outline',
        className: MANUAL_BADGE_CLASS,
        ...base,
      };
    case 'retry_not_ready':
      return {
        label: 'Retry Scheduled',
        variant: 'secondary',
        className: QUEUED_BADGE_CLASS,
        ...base,
      };
    case 'retry_exhausted':
      return {
        label: 'Retry Exhausted',
        variant: 'destructive',
        className: BLOCKED_BADGE_CLASS,
        ...base,
      };
    case 'awaiting_human_review':
      return {
        label: 'Awaiting Review',
        variant: 'secondary',
        className: REVIEW_BADGE_CLASS,
        ...base,
      };
    case 'concurrency_limit_reached':
      return {
        label: 'Queued',
        variant: 'secondary',
        className: QUEUED_BADGE_CLASS,
        ...base,
      };
    case 'no_project_repos':
    case 'base_branch_unresolved':
    case 'workspace_hook_failed':
    case 'blocked':
      return {
        label:
          diagnostic.reason_code === 'workspace_hook_failed'
            ? 'Hook Failed'
            : 'Auto Blocked',
        variant: 'destructive',
        className: BLOCKED_BADGE_CLASS,
        ...base,
      };
    default:
      return {
        label: getTaskAutomationModeLabel(task),
        variant: 'outline',
        className: MANUAL_BADGE_CLASS,
        ...base,
      };
  }
}

export function getTaskAutomationPresentation(
  task: TaskWithAttemptStatus
): AutomationPresentation | null {
  const state = task.dispatch_state;
  const diagnosticPresentation = presentationFromDiagnostic(task);

  if (state?.status === 'running') {
    return {
      label: 'Auto Running',
      variant: 'default',
      className: RUNNING_BADGE_CLASS,
      detail: diagnosticPresentation?.detail ?? formatRetryDetail(state),
      reasonCode: diagnosticPresentation?.reasonCode,
      actionable: diagnosticPresentation?.actionable,
    };
  }

  if (state?.status === 'claimed') {
    return {
      label: 'Queued',
      variant: 'secondary',
      className: QUEUED_BADGE_CLASS,
      detail: diagnosticPresentation?.detail ?? formatRetryDetail(state),
      reasonCode: diagnosticPresentation?.reasonCode,
      actionable: diagnosticPresentation?.actionable,
    };
  }

  if (diagnosticPresentation) {
    return diagnosticPresentation;
  }

  if (task.effective_automation_mode === 'auto') {
    return {
      label:
        task.automation_mode === 'auto' &&
        task.project_execution_mode !== 'auto'
          ? 'Auto Override'
          : 'Auto Managed',
      variant: 'secondary',
      className: MANAGED_BADGE_CLASS,
    };
  }

  return {
    label: getTaskAutomationModeLabel(task),
    variant: 'outline',
    className: MANUAL_BADGE_CLASS,
  };
}

export function getTaskOrchestrationLane(
  task: TaskWithAttemptStatus
): Exclude<OrchestrationFilter, 'all'> {
  const state = task.dispatch_state;
  const reasonCode = task.automation_diagnostic?.reason_code;

  if (state?.status === 'awaiting_human_review' || task.status === 'inreview') {
    return 'needs_review';
  }

  if (state?.status === 'blocked' || isBlockedReason(reasonCode)) {
    return 'blocked';
  }

  if (
    task.effective_automation_mode === 'auto' ||
    state?.status === 'running' ||
    state?.status === 'claimed' ||
    state?.status === 'retry_scheduled'
  ) {
    return 'managed';
  }

  return 'manual';
}

export function matchesOrchestrationFilter(
  task: TaskWithAttemptStatus,
  filter: OrchestrationFilter
): boolean {
  if (filter === 'all') {
    return true;
  }

  return getTaskOrchestrationLane(task) === filter;
}

export function normalizeOrchestrationFilters(
  values: Iterable<OrchestrationLane | string | null | undefined>
): OrchestrationLane[] {
  const selected = new Set<OrchestrationLane>();

  for (const value of values) {
    if (
      value === 'manual' ||
      value === 'managed' ||
      value === 'needs_review' ||
      value === 'blocked'
    ) {
      selected.add(value);
    }
  }

  return ORCHESTRATION_LANES.filter((value) => selected.has(value));
}

export function matchesOrchestrationFilters(
  task: TaskWithAttemptStatus,
  filters: OrchestrationLane[]
): boolean {
  if (filters.length === 0) {
    return true;
  }

  return filters.includes(getTaskOrchestrationLane(task));
}

export function summarizeTaskOrchestration(
  tasks: TaskWithAttemptStatus[]
): OrchestrationSummary {
  return tasks.reduce<OrchestrationSummary>(
    (summary, task) => {
      const lane = getTaskOrchestrationLane(task);
      summary.all += 1;
      summary[lane] += 1;
      return summary;
    },
    {
      all: 0,
      manual: 0,
      managed: 0,
      needs_review: 0,
      blocked: 0,
    }
  );
}

export function getTaskOwnershipPresentation(
  task: TaskWithAttemptStatus
): AutomationPresentation {
  const lane = getTaskOrchestrationLane(task);

  if (lane === 'needs_review') {
    return {
      kind: 'needs_review',
      label: 'Needs Review',
      variant: 'secondary',
      className: REVIEW_BADGE_CLASS,
      detail:
        task.automation_diagnostic?.reason_detail ??
        'Automation is paused until a human reviews the latest result.',
      reasonCode: task.automation_diagnostic?.reason_code,
      actionable: true,
    };
  }

  if (lane === 'blocked') {
    return {
      kind: 'blocked',
      label: 'Blocked',
      variant: 'destructive',
      className: BLOCKED_BADGE_CLASS,
      detail:
        task.automation_diagnostic?.reason_detail ??
        task.dispatch_state?.blocked_reason ??
        'Auto orchestration is blocked for this task.',
      reasonCode: task.automation_diagnostic?.reason_code,
      actionable: task.automation_diagnostic?.actionable,
    };
  }

  if (lane === 'managed') {
    return {
      kind: 'managed',
      label: 'Auto-managed',
      variant: 'secondary',
      className: MANAGED_BADGE_CLASS,
      detail: getManagedOwnershipDetail(task),
      reasonCode: task.automation_diagnostic?.reason_code,
      actionable: task.automation_diagnostic?.actionable,
    };
  }

  return {
    kind: 'manual',
    label: 'Manual',
    variant: 'outline',
    className: MANUAL_BADGE_CLASS,
    detail: getManualOwnershipDetail(task),
    reasonCode: task.automation_diagnostic?.reason_code,
    actionable: task.automation_diagnostic?.actionable,
  };
}

export function getTaskRuntimePresentation(
  task: TaskWithAttemptStatus
): AutomationPresentation | null {
  const state = task.dispatch_state;

  if (state?.status === 'running') {
    return {
      label: 'Running',
      variant: 'default',
      className: RUNNING_BADGE_CLASS,
      detail: formatRetryDetail(state),
    };
  }

  if (task.has_in_progress_attempt) {
    return {
      label: 'Running',
      variant: 'default',
      className: RUNNING_BADGE_CLASS,
    };
  }

  if (state?.status === 'claimed') {
    return {
      label: 'Queued',
      variant: 'secondary',
      className: QUEUED_BADGE_CLASS,
      detail: formatRetryDetail(state),
    };
  }

  if (state?.status === 'retry_scheduled') {
    return {
      label: 'Retry Scheduled',
      variant: 'secondary',
      className: QUEUED_BADGE_CLASS,
      detail: formatRetryDetail(state),
    };
  }

  if (state?.status === 'blocked') {
    if (getTaskOwnershipPresentation(task).kind === 'blocked') {
      return null;
    }

    return {
      label: 'Blocked',
      variant: 'destructive',
      className: BLOCKED_BADGE_CLASS,
      detail: formatRetryDetail(state),
    };
  }

  if (task.last_attempt_failed) {
    return {
      label: 'Last Run Failed',
      variant: 'destructive',
      className: BLOCKED_BADGE_CLASS,
    };
  }

  return null;
}

export function isTaskAwaitingHumanReview(task: TaskWithAttemptStatus) {
  return (
    task.dispatch_state?.status === 'awaiting_human_review' ||
    task.automation_diagnostic?.reason_code === 'awaiting_human_review' ||
    task.status === 'inreview'
  );
}

export function getTaskAutomationDetail(
  task: TaskWithAttemptStatus
): string | undefined {
  return (
    getTaskRuntimePresentation(task)?.detail ??
    task.automation_diagnostic?.reason_detail ??
    getTaskOwnershipPresentation(task).detail
  );
}

export function normalizeOrchestrationFilter(
  value: string | null
): OrchestrationFilter {
  switch (value) {
    case 'manual':
    case 'managed':
    case 'needs_review':
    case 'blocked':
      return value;
    default:
      return 'all';
  }
}

export function getResumeAutomationMode(task: TaskWithAttemptStatus) {
  return task.project_execution_mode === 'auto' ? 'inherit' : 'auto';
}

export function getTaskAutomationOwnerKey(
  task: TaskWithAttemptStatus
): Exclude<OrchestrationFilter, 'all'> {
  return getTaskOrchestrationLane(task);
}

export function getTaskAutomationOwnershipPresentation(
  task: TaskWithAttemptStatus
): AutomationPresentation {
  return getTaskOwnershipPresentation(task);
}

export function getTaskAutomationRuntimePresentation(
  task: TaskWithAttemptStatus
): AutomationPresentation | null {
  return getTaskRuntimePresentation(task);
}
