import fs from 'node:fs';
import path from 'node:path';

import type {
  CreateMilestone,
  ExecutorProfileId,
  Milestone,
  MilestonePlanV1,
} from '../shared/types';

import { apiPost } from './helpers/api';
import { createRepoAndProject } from './helpers/setup';
import { expect, test } from './fixtures';

test.describe('milestone planning', () => {
  test('preview + apply a pasted plan payload updates the milestone graph', async ({
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
      default_executor_profile_id: fakeAgentProfile,
      automation_mode: 'manual',
      status: null,
      baseline_ref: 'main',
      schema_version: 1,
      graph: {
        nodes: [],
        edges: [],
      },
    };

    const milestone = await apiPost<Milestone>(
      page.request,
      '/api/milestones',
      createPayload
    );

    const plan: MilestonePlanV1 = {
      schema_version: 1,
      milestone: {
        objective: `objective-${makeName('milestone')}`,
        definition_of_done: `dod-${makeName('milestone')}`,
        default_executor_profile_id: null,
        automation_mode: null,
        baseline_ref: null,
      },
      nodes: [
        {
          id: 'node-a',
          kind: 'task',
          phase: 0,
          executor_profile_id: fakeAgentProfile,
          base_strategy: 'topology',
          instructions: null,
          requires_approval: false,
          layout: null,
          task_id: null,
          create_task: {
            title: makeName('planned-task-a'),
            description: 'Created by milestone planner apply',
          },
        },
        {
          id: 'node-b',
          kind: 'checkpoint',
          phase: 1,
          executor_profile_id: fakeAgentProfile,
          base_strategy: 'topology',
          instructions: null,
          requires_approval: true,
          layout: null,
          task_id: null,
          create_task: {
            title: makeName('planned-task-b'),
            description: null,
          },
        },
      ],
      edges: [{ from: 'node-a', to: 'node-b', data_flow: null }],
    };

    await page.goto(`/projects/${project.id}/milestones/${milestone.id}`);

    await expect(page.getByRole('button', { name: 'Planner' })).toBeVisible({
      timeout: 60_000,
    });
    await page.getByRole('button', { name: 'Planner' }).click();

    // The raw JSON textarea should not be visible by default; it lives behind Advanced/Debug.
    await expect(
      page.getByPlaceholder(/Paste a MilestonePlanV1 JSON payload/i)
    ).toHaveCount(0);

    await page.locator('summary', { hasText: 'Advanced / Debug' }).click();

    const textarea = page.getByPlaceholder(/Paste a MilestonePlanV1 JSON payload/i);
    await textarea.fill(
      `Here is a plan:\n\n\`\`\`milestone-plan-v1\n${JSON.stringify(plan, null, 2)}\n\`\`\`\n`
    );

    const previewResponse = page.waitForResponse((response) => {
      const request = response.request();
      return (
        request.method() === 'POST' &&
        request.url().includes(`/api/milestones/${milestone.id}/plan/preview`)
      );
    });
    await page.getByRole('button', { name: 'Preview' }).click();
    await previewResponse;

    await expect(page.getByText('Tasks to create (2)')).toBeVisible();

    const applyResponse = page.waitForResponse((response) => {
      const request = response.request();
      return (
        request.method() === 'POST' &&
        request.url().includes(`/api/milestones/${milestone.id}/plan/apply`)
      );
    });
    await page.getByRole('button', { name: 'Apply' }).click();
    await page.getByRole('button', { name: 'Apply plan' }).click();
    await applyResponse;

    const todoColumn = page.getByTestId('kanban-column-todo');

    // Tasks created by the planner should appear on the milestone board with a visual marker.
    await expect(todoColumn.getByText(plan.nodes[0].create_task!.title)).toBeVisible({
      timeout: 60_000,
    });
    await expect(todoColumn.getByText(plan.nodes[1].create_task!.title)).toBeVisible();
    await expect(todoColumn.getByTestId('planner-created-badge')).toHaveCount(2);
  });
});
