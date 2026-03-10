import type {
  Workspace,
  WorkspaceLifecycleHookPhase,
  WorkspaceLifecycleHookRunSummary,
  WorkspaceLifecycleHookStatus,
} from 'shared/types';

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

  if (
    workspace.after_prepare_hook_status &&
    workspace.after_prepare_hook_ran_at
  ) {
    snapshots.push({
      phase: 'after_prepare',
      status: workspace.after_prepare_hook_status,
      ran_at: workspace.after_prepare_hook_ran_at,
      error_summary: workspace.after_prepare_hook_error_summary ?? null,
    });
  }

  if (
    workspace.before_cleanup_hook_status &&
    workspace.before_cleanup_hook_ran_at
  ) {
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
