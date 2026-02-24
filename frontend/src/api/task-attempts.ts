import type {
  AbortConflictsRequest,
  ChangeTargetBranchRequest,
  ChangeTargetBranchResponse,
  CreateGitHubPrRequest,
  CreatePrError,
  CreateTaskAttemptBody,
  ExecutionProcess,
  GhCliSetupError,
  GitOperationError,
  MergeTaskAttemptRequest,
  OpenEditorRequest,
  OpenEditorResponse,
  PrCommentsResponse,
  PushError,
  PushTaskAttemptRequest,
  RebaseTaskAttemptRequest,
  RenameBranchRequest,
  RenameBranchResponse,
  RepoBranchStatus,
  RepoWithTargetBranch,
  RunAgentSetupRequest,
  RunAgentSetupResponse,
  RunScriptError,
  TaskRelationships,
  Workspace,
} from 'shared/types';
import type { WorkspaceWithSession } from '@/types/attempt';
import { createWorkspaceWithSession } from '@/types/attempt';

import type { Result } from './client';
import {
  handleApiResponse,
  handleApiResponseAsResult,
  makeRequest,
} from './client';
import { sessionsApi } from './sessions';

export const attemptsApi = {
  getChildren: async (attemptId: string): Promise<TaskRelationships> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/children`
    );
    return handleApiResponse<TaskRelationships>(response);
  },

  getAll: async (taskId: string): Promise<Workspace[]> => {
    const response = await makeRequest(`/api/task-attempts?task_id=${taskId}`);
    return handleApiResponse<Workspace[]>(response);
  },

  getAllWithSessions: async (
    taskId: string
  ): Promise<WorkspaceWithSession[]> => {
    const response = await makeRequest(
      `/api/task-attempts/with-latest-session?task_id=${taskId}`
    );
    return handleApiResponse<WorkspaceWithSession[]>(response);
  },

  get: async (attemptId: string): Promise<Workspace> => {
    const response = await makeRequest(`/api/task-attempts/${attemptId}`);
    return handleApiResponse<Workspace>(response);
  },

  /** Get workspace with latest session */
  getWithSession: async (attemptId: string): Promise<WorkspaceWithSession> => {
    const [workspace, sessions] = await Promise.all([
      attemptsApi.get(attemptId),
      sessionsApi.getByWorkspace(attemptId),
    ]);
    return createWorkspaceWithSession(workspace, sessions[0]);
  },

  create: async (data: CreateTaskAttemptBody): Promise<Workspace> => {
    const response = await makeRequest(`/api/task-attempts`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<Workspace>(response);
  },

  stop: async (
    attemptId: string,
    options?: { force?: boolean }
  ): Promise<void> => {
    const params = options?.force ? '?force=true' : '';
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/stop${params}`,
      {
        method: 'POST',
      }
    );
    return handleApiResponse<void>(response);
  },

  removeWorktree: async (attemptId: string): Promise<void> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/remove-worktree`,
      {
        method: 'POST',
      }
    );
    return handleApiResponse<void>(response);
  },

  runAgentSetup: async (
    attemptId: string,
    data: RunAgentSetupRequest
  ): Promise<RunAgentSetupResponse> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/run-agent-setup`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    );
    return handleApiResponse<RunAgentSetupResponse>(response);
  },

  openEditor: async (
    attemptId: string,
    data: OpenEditorRequest
  ): Promise<OpenEditorResponse> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/open-editor`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    );
    return handleApiResponse<OpenEditorResponse>(response);
  },

  getBranchStatus: async (attemptId: string): Promise<RepoBranchStatus[]> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/branch-status`
    );
    return handleApiResponse<RepoBranchStatus[]>(response);
  },

  getRepos: async (attemptId: string): Promise<RepoWithTargetBranch[]> => {
    const response = await makeRequest(`/api/task-attempts/${attemptId}/repos`);
    return handleApiResponse<RepoWithTargetBranch[]>(response);
  },

  merge: async (
    attemptId: string,
    data: MergeTaskAttemptRequest
  ): Promise<void> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/merge`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    );
    return handleApiResponse<void>(response);
  },

  push: async (
    attemptId: string,
    data: PushTaskAttemptRequest
  ): Promise<Result<void, PushError>> => {
    const response = await makeRequest(`/api/task-attempts/${attemptId}/push`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponseAsResult<void, PushError>(response);
  },

  forcePush: async (
    attemptId: string,
    data: PushTaskAttemptRequest
  ): Promise<Result<void, PushError>> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/push/force`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    );
    return handleApiResponseAsResult<void, PushError>(response);
  },

  rebase: async (
    attemptId: string,
    data: RebaseTaskAttemptRequest
  ): Promise<Result<void, GitOperationError>> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/rebase`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    );
    return handleApiResponseAsResult<void, GitOperationError>(response);
  },

  change_target_branch: async (
    attemptId: string,
    data: ChangeTargetBranchRequest
  ): Promise<ChangeTargetBranchResponse> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/change-target-branch`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    );
    return handleApiResponse<ChangeTargetBranchResponse>(response);
  },

  renameBranch: async (
    attemptId: string,
    newBranchName: string
  ): Promise<RenameBranchResponse> => {
    const payload: RenameBranchRequest = {
      new_branch_name: newBranchName,
    };
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/rename-branch`,
      {
        method: 'POST',
        body: JSON.stringify(payload),
      }
    );
    return handleApiResponse<RenameBranchResponse>(response);
  },

  abortConflicts: async (
    attemptId: string,
    data: AbortConflictsRequest
  ): Promise<void> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/conflicts/abort`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    );
    return handleApiResponse<void>(response);
  },

  createPR: async (
    attemptId: string,
    data: CreateGitHubPrRequest
  ): Promise<Result<string, CreatePrError>> => {
    const response = await makeRequest(`/api/task-attempts/${attemptId}/pr`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponseAsResult<string, CreatePrError>(response);
  },

  startDevServer: async (attemptId: string): Promise<void> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/start-dev-server`,
      {
        method: 'POST',
      }
    );
    return handleApiResponse<void>(response);
  },

  setupGhCli: async (attemptId: string): Promise<ExecutionProcess> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/gh-cli-setup`,
      {
        method: 'POST',
      }
    );
    return handleApiResponse<ExecutionProcess, GhCliSetupError>(response);
  },

  runSetupScript: async (
    attemptId: string
  ): Promise<Result<ExecutionProcess, RunScriptError>> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/run-setup-script`,
      {
        method: 'POST',
      }
    );
    return handleApiResponseAsResult<ExecutionProcess, RunScriptError>(
      response
    );
  },

  runCleanupScript: async (
    attemptId: string
  ): Promise<Result<ExecutionProcess, RunScriptError>> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/run-cleanup-script`,
      {
        method: 'POST',
      }
    );
    return handleApiResponseAsResult<ExecutionProcess, RunScriptError>(
      response
    );
  },

  getPrComments: async (
    attemptId: string,
    repoId: string
  ): Promise<PrCommentsResponse> => {
    const response = await makeRequest(
      `/api/task-attempts/${attemptId}/pr/comments?repo_id=${encodeURIComponent(repoId)}`
    );
    return handleApiResponse<PrCommentsResponse>(response);
  },
};
