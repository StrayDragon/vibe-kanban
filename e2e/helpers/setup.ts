import { randomUUID } from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';
import type { APIRequestContext } from '@playwright/test';

import type {
  CreateTask,
  Project,
  Repo,
  Task,
} from '../../shared/types';

import { apiGet, apiPost } from './api';
import { getConfigDir } from './seed';

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
  const configDir = getConfigDir();
  const configPath = path.join(configDir, 'config.yaml');
  const projectId = randomUUID();

  let rawConfig: any = {};
  try {
    const raw = await fs.readFile(configPath, 'utf8');
    rawConfig = raw.trim() ? JSON.parse(raw) : {};
  } catch (err: any) {
    if (err?.code !== 'ENOENT') {
      throw err;
    }
  }

  const projects = Array.isArray(rawConfig.projects) ? rawConfig.projects : [];
  projects.push({
    id: projectId,
    name: args.name,
    repos: [
      {
        path: args.repoPath,
        display_name: args.name,
      },
    ],
  });

  const nextConfig = { ...rawConfig, projects };
  await fs.mkdir(configDir, { recursive: true });
  await fs.writeFile(configPath, `${JSON.stringify(nextConfig, null, 2)}\n`);

  await apiPost(request, '/api/config/reload', {});

  const loaded = await apiGet<Project[]>(request, '/api/projects');
  const project = loaded.find((p) => p.id === projectId);
  if (!project) {
    throw new Error(`Project not found after reload: ${projectId}`);
  }

  return project;
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
