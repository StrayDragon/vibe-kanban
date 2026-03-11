import fs from 'node:fs';
import path from 'node:path';

import type {
  ExecutorProfileId,
  CreateMilestone,
  Milestone,
} from '../shared/types';

import { createRepoAndProject, createTask } from './helpers/setup';
import { apiPost } from './helpers/api';
import { expect, test } from './fixtures';

test.describe('milestone automation', () => {
  test('edits milestone metadata + queues next step', async ({
    page,
    makeName,
    reposDir,
  }) => {
    fs.mkdirSync(reposDir, { recursive: true });
    fs.mkdirSync(path.join(reposDir, 'worktrees'), { recursive: true });

    const projectName = makeName('project');
    const repoFolderName = makeName('repo');
    const { project } = await createRepoAndProject(page.request, {
      name: projectName,
      reposDir,
      repoFolderName,
    });

    const taskA = await createTask(page.request, {
      projectId: project.id,
      title: makeName('node-a'),
    });
    const taskB = await createTask(page.request, {
      projectId: project.id,
      title: makeName('node-b'),
    });
    const taskC = await createTask(page.request, {
      projectId: project.id,
      title: makeName('checkpoint'),
    });

    const fakeAgentProfile: ExecutorProfileId = {
      executor: 'FAKE_AGENT',
      variant: null,
    };

    const createPayload: CreateMilestone = {
      project_id: project.id,
      title: makeName('milestone'),
      description: null,
      objective: null,
      definition_of_done: null,
      default_executor_profile_id: null,
      automation_mode: null,
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
            instructions: null,
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
            task_id: taskC.id,
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

    const objectiveText = makeName('objective');
    const objective = page.getByPlaceholder('What does success look like?');
    await objective.fill(objectiveText);
    const objectiveResponse = page.waitForResponse((response) => {
      const request = response.request();
      return (
        request.method() === 'PUT' &&
        request.url().includes(`/api/milestones/${milestone.id}`) &&
        request.postData()?.includes('"objective"') === true
      );
    });
    await objective.press('Tab');
    await objectiveResponse;

    const dodText = `${makeName('dod')}\n- accepts\n- passes`;
    const dod = page.getByPlaceholder(
      'Acceptance criteria and completion checklist'
    );
    await expect(dod).toBeEnabled({ timeout: 30_000 });
    await dod.fill(dodText);
    const dodResponse = page.waitForResponse((response) => {
      const request = response.request();
      return (
        request.method() === 'PUT' &&
        request.url().includes(`/api/milestones/${milestone.id}`) &&
        request.postData()?.includes('"definition_of_done"') === true
      );
    });
    await dod.press('Tab');
    await dodResponse;

    const runNextResponse = page.waitForResponse((response) => {
      const request = response.request();
      return (
        request.method() === 'POST' &&
        request.url().includes(
          `/api/milestones/${milestone.id}/run-next-step`
        )
      );
    });
    await page.getByRole('button', { name: 'Run next step' }).click();

    const response = await runNextResponse;
    expect(response.ok()).toBeTruthy();
    const json = (await response.json()) as {
      success: boolean;
      data?: { status: string; candidate_task_id: string | null };
    };
    expect(json.success).toBeTruthy();
    expect(json.data?.status).toBe('queued');
    expect(json.data?.candidate_task_id).toBe(taskA.id);

    await expect(page.getByText(/queued next step/i)).toBeVisible();

    const automationSwitch = page.getByRole('switch', { name: 'Automation' });
    await expect(automationSwitch).toHaveAttribute('aria-checked', 'false');
    const automationResponse = page.waitForResponse((response) => {
      const request = response.request();
      return (
        request.method() === 'PUT' &&
        request.url().includes(`/api/milestones/${milestone.id}`) &&
        request.postData()?.includes('"automation_mode"') === true
      );
    });
    await automationSwitch.click();
    await automationResponse;
    await expect(automationSwitch).toHaveAttribute('aria-checked', 'true');

    await page.reload();
    await page.getByRole('button', { name: 'Details' }).click();
    await expect(page.getByPlaceholder('What does success look like?')).toHaveValue(
      objectiveText
    );
    await expect(
      page.getByPlaceholder('Acceptance criteria and completion checklist')
    ).toHaveValue(dodText);
  });
});
