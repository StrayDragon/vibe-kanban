import path from 'node:path';
import type { APIRequestContext } from '@playwright/test';

import type {
  CreateProject,
  CreateTask,
  Project,
  Repo,
  Task,
} from '../../shared/types';

import { apiPost } from './api';

export async function createRepo(
  request: APIRequestContext,
  args: { parentDir: string; folderName: string }
): Promise<Repo> {
  return apiPost<Repo>(request, '/api/repos/init', {
    parent_path: args.parentDir,
    folder_name: args.folderName,
  });
}

export async function createProject(
  request: APIRequestContext,
  args: { name: string; repoPath: string }
): Promise<Project> {
  const payload: CreateProject = {
    name: args.name,
    repositories: [
      { display_name: args.name, git_repo_path: args.repoPath },
    ],
  };
  return apiPost<Project>(request, '/api/projects', payload);
}

export async function createRepoAndProject(
  request: APIRequestContext,
  args: { name: string; reposDir: string; repoFolderName: string }
): Promise<{ repo: Repo; project: Project }> {
  const repo = await createRepo(request, {
    parentDir: args.reposDir,
    folderName: args.repoFolderName,
  });
  const project = await createProject(request, {
    name: args.name,
    repoPath: repo.path,
  });
  return { repo, project };
}

export async function createTask(
  request: APIRequestContext,
  args: {
    projectId: string;
    title: string;
    description?: string | null;
    status?: CreateTask['status'];
  }
): Promise<Task> {
  const payload: CreateTask = {
    project_id: args.projectId,
    title: args.title,
    description: args.description ?? null,
    status: args.status ?? null,
    task_kind: null,
    milestone_id: null,
    milestone_node_id: null,
    parent_workspace_id: null,
    origin_task_id: null,
    created_by_kind: null,
    image_ids: null,
    shared_task_id: null,
  };
  return apiPost<Task>(request, '/api/tasks', payload);
}

export function unsafeWorktreesDir(reposDir: string): string {
  // Use a deterministic subdir that matches common "worktrees" heuristics.
  return path.join(reposDir, 'worktrees');
}
