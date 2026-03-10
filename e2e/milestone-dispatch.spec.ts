import fs from 'node:fs';
import path from 'node:path';

import type {
  ExecutionProcess,
  ExecutorProfileId,
  CreateMilestone,
  RunNextMilestoneStepResponse,
  TaskAttemptStatusResponse,
  Milestone,
  Workspace,
} from '../shared/types';

import { apiGet, apiPost } from './helpers/api';
import { createRepoAndProject, createTask } from './helpers/setup';
import { expect, test } from './fixtures';

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForAttempts(
  request: Parameters<typeof apiGet>[0],
  taskId: string,
  opts?: { timeoutMs?: number }
): Promise<Workspace[]> {
  const timeoutMs = opts?.timeoutMs ?? 60_000;
  const deadline = Date.now() + timeoutMs;

  // eslint-disable-next-line no-constant-condition
  while (true) {
    const attempts = await apiGet<Workspace[]>(
      request,
      `/api/task-attempts?task_id=${taskId}`
    );
    if (attempts.length > 0) return attempts;
    if (Date.now() > deadline) {
      throw new Error(`Timed out waiting for attempts for task ${taskId}`);
    }
    await delay(250);
  }
}

async function waitForExecutionProcessId(
  request: Parameters<typeof apiGet>[0],
  attemptId: string,
  opts?: { timeoutMs?: number }
): Promise<string> {
  const timeoutMs = opts?.timeoutMs ?? 60_000;
  const deadline = Date.now() + timeoutMs;

  // eslint-disable-next-line no-constant-condition
  while (true) {
    const status = await apiGet<TaskAttemptStatusResponse>(
      request,
      `/api/task-attempts/${attemptId}/status`
    );
    if (status.latest_execution_process_id) return status.latest_execution_process_id;
    if (Date.now() > deadline) {
      throw new Error(
        `Timed out waiting for execution process id for attempt ${attemptId}`
      );
    }
    await delay(250);
  }
}

async function waitForRunNextStepCleared(
  request: Parameters<typeof apiGet>[0],
  milestoneId: string,
  opts?: { timeoutMs?: number }
): Promise<void> {
  const timeoutMs = opts?.timeoutMs ?? 60_000;
  const deadline = Date.now() + timeoutMs;

  // eslint-disable-next-line no-constant-condition
  while (true) {
    const milestone = await apiGet<Milestone>(
      request,
      `/api/milestones/${milestoneId}`
    );
    if (milestone.run_next_step_requested_at === null) return;
    if (Date.now() > deadline) {
      throw new Error(
        `Timed out waiting for run-next-step request to clear for milestone ${milestoneId}`
      );
    }
    await delay(250);
  }
}

async function fetchExecutionPrompt(
  request: Parameters<typeof apiGet>[0],
  executionProcessId: string
): Promise<string> {
  const process = await apiGet<ExecutionProcess>(
    request,
    `/api/execution-processes/${executionProcessId}`
  );

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const typ = process.executor_action.typ as any;
  if (typ?.type !== 'CodingAgentInitialRequest' && typ?.type !== 'CodingAgentFollowUpRequest') {
    throw new Error(`Unexpected executor action type: ${String(typ?.type)}`);
  }
  if (typeof typ.prompt !== 'string' || typ.prompt.trim() === '') {
    throw new Error('Missing prompt on executor action');
  }
  return typ.prompt;
}

test.describe('milestone dispatch', () => {
  test('manual run-next-step queues + is consumed by scheduler (prompt injected)', async ({
    page,
    makeName,
    reposDir,
  }) => {
    fs.mkdirSync(reposDir, { recursive: true });
    fs.mkdirSync(path.join(reposDir, 'worktrees'), { recursive: true });

    const { project } = await createRepoAndProject(page.request, {
      name: makeName('project'),
      reposDir,
      repoFolderName: makeName('repo'),
    });

    const taskA = await createTask(page.request, {
      projectId: project.id,
      title: makeName('node-a'),
    });
    const taskB = await createTask(page.request, {
      projectId: project.id,
      title: makeName('node-b'),
    });
    const checkpoint = await createTask(page.request, {
      projectId: project.id,
      title: makeName('checkpoint'),
    });

    const fakeAgentProfile: ExecutorProfileId = {
      executor: 'FAKE_AGENT',
      variant: null,
    };

    const objective = `objective-${makeName('milestone')}`;
    const dod = `dod-${makeName('milestone')}`;
    const nodeInstructions = `node-instructions-${makeName('node-a')}`;

    const createPayload: CreateMilestone = {
      project_id: project.id,
      title: makeName('milestone'),
      description: null,
      objective,
      definition_of_done: dod,
      default_executor_profile_id: fakeAgentProfile,
      automation_mode: 'manual',
      status: null,
      baseline_ref: 'main',
      schema_version: 1,
      graph: {
        nodes: [
          {
            id: 'node-a',
            task_id: taskA.id,
            kind: 'task',
            phase: 0,
            executor_profile_id: fakeAgentProfile,
            base_strategy: 'topology',
            instructions: nodeInstructions,
            requires_approval: false,
            layout: { x: 0, y: 0 },
          },
          {
            id: 'node-b',
            task_id: taskB.id,
            kind: 'task',
            phase: 0,
            executor_profile_id: fakeAgentProfile,
            base_strategy: 'topology',
            instructions: null,
            requires_approval: false,
            layout: { x: 260, y: 0 },
          },
          {
            id: 'node-c',
            task_id: checkpoint.id,
            kind: 'checkpoint',
            phase: 0,
            executor_profile_id: fakeAgentProfile,
            base_strategy: 'topology',
            instructions: null,
            requires_approval: true,
            layout: { x: 520, y: 0 },
          },
        ],
        edges: [
          { id: 'edge-ab', from: 'node-a', to: 'node-b', data_flow: null },
          { id: 'edge-bc', from: 'node-b', to: 'node-c', data_flow: null },
        ],
      },
    };

    const milestone = await apiPost<Milestone>(
      page.request,
      '/api/milestones',
      createPayload
    );

    await page.goto(`/projects/${project.id}/milestones/${milestone.id}`);
    await expect(page.getByRole('button', { name: 'Details' })).toBeVisible({
      timeout: 60_000,
    });

    await page.getByRole('button', { name: 'Details' }).click();

    const runNextResponse = page.waitForResponse((response) => {
      const request = response.request();
      return (
        request.method() === 'POST' &&
        request
          .url()
          .includes(`/api/milestones/${milestone.id}/run-next-step`)
      );
    });
    await page.getByRole('button', { name: 'Run next step' }).click();

    const response = await runNextResponse;
    expect(response.ok()).toBeTruthy();
    const json = (await response.json()) as {
      success: boolean;
      data?: RunNextMilestoneStepResponse;
    };
    expect(json.success).toBeTruthy();
    expect(json.data?.status).toBe('queued');
    expect(json.data?.candidate_task_id).toBe(taskA.id);

    const [attempt] = await waitForAttempts(page.request, taskA.id, {
      timeoutMs: 90_000,
    });
    const executionProcessId = await waitForExecutionProcessId(
      page.request,
      attempt.id,
      { timeoutMs: 90_000 }
    );

    const prompt = await fetchExecutionPrompt(page.request, executionProcessId);
    expect(prompt).toContain('Milestone context:');
    expect(prompt).toContain(objective);
    expect(prompt).toContain(dod);
    expect(prompt).toContain('Node instructions:');
    expect(prompt).toContain(nodeInstructions);

    await waitForRunNextStepCleared(page.request, milestone.id, { timeoutMs: 90_000 });

    // Manual milestone should not auto-advance to node-b without another explicit request.
    const taskBAttempts = await apiGet<Workspace[]>(
      page.request,
      `/api/task-attempts?task_id=${taskB.id}`
    );
    expect(taskBAttempts.length).toBe(0);
  });

  test('auto milestone dispatches nodes sequentially (one per tick)', async ({
    page,
    makeName,
    reposDir,
  }) => {
    fs.mkdirSync(reposDir, { recursive: true });
    fs.mkdirSync(path.join(reposDir, 'worktrees'), { recursive: true });

    const { project } = await createRepoAndProject(page.request, {
      name: makeName('project'),
      reposDir,
      repoFolderName: makeName('repo'),
    });

    const taskA = await createTask(page.request, {
      projectId: project.id,
      title: makeName('node-a'),
    });
    const taskB = await createTask(page.request, {
      projectId: project.id,
      title: makeName('node-b'),
    });

    const fakeAgentProfile: ExecutorProfileId = {
      executor: 'FAKE_AGENT',
      variant: null,
    };

    const objective = `objective-${makeName('milestone')}`;
    const dod = `dod-${makeName('milestone')}`;
    const nodeAInstructions = `node-instructions-${makeName('node-a')}`;

    const createPayload: CreateMilestone = {
      project_id: project.id,
      title: makeName('milestone'),
      description: null,
      objective,
      definition_of_done: dod,
      default_executor_profile_id: fakeAgentProfile,
      automation_mode: 'auto',
      status: null,
      baseline_ref: 'main',
      schema_version: 1,
      graph: {
        nodes: [
          {
            id: 'node-a',
            task_id: taskA.id,
            kind: 'task',
            phase: 0,
            executor_profile_id: fakeAgentProfile,
            base_strategy: 'topology',
            instructions: nodeAInstructions,
            requires_approval: false,
            layout: { x: 0, y: 0 },
          },
          {
            id: 'node-b',
            task_id: taskB.id,
            kind: 'task',
            phase: 0,
            executor_profile_id: fakeAgentProfile,
            base_strategy: 'topology',
            instructions: null,
            requires_approval: false,
            layout: { x: 260, y: 0 },
          },
        ],
        edges: [],
      },
    };

    const milestone = await apiPost<Milestone>(
      page.request,
      '/api/milestones',
      createPayload
    );

    // First dispatch should happen quickly after creation (interval ticks immediately).
    const [attemptA] = await waitForAttempts(page.request, taskA.id, {
      timeoutMs: 90_000,
    });
    const taskBAttemptsInitial = await apiGet<Workspace[]>(
      page.request,
      `/api/task-attempts?task_id=${taskB.id}`
    );
    expect(taskBAttemptsInitial.length).toBe(0);

    const execIdA = await waitForExecutionProcessId(page.request, attemptA.id, {
      timeoutMs: 90_000,
    });
    const prompt = await fetchExecutionPrompt(page.request, execIdA);
    expect(prompt).toContain('Milestone context:');
    expect(prompt).toContain(objective);
    expect(prompt).toContain(dod);
    expect(prompt).toContain('Node instructions:');
    expect(prompt).toContain(nodeAInstructions);

    // Ensure node-b is not dispatched immediately in the same tick.
    await delay(2_000);
    const taskBAttemptsStillEmpty = await apiGet<Workspace[]>(
      page.request,
      `/api/task-attempts?task_id=${taskB.id}`
    );
    expect(taskBAttemptsStillEmpty.length).toBe(0);

    // Next tick should eventually dispatch node-b.
    const [attemptB] = await waitForAttempts(page.request, taskB.id, {
      timeoutMs: 120_000,
    });
    expect(attemptB.id).not.toBe(attemptA.id);

    // Smoke: milestone workflow page still loads in UI.
    await page.goto(`/projects/${project.id}/milestones/${milestone.id}`);
    await expect(page.getByRole('button', { name: 'Details' })).toBeVisible({
      timeout: 60_000,
    });
  });
});
