import { uiIds } from '../frontend/src/lib/uiIds';
import { createRepoAndProject } from './helpers/setup';
import { expect, test } from './fixtures';

test('dev-mode smoke: create task and reach the started task', async ({
  page,
  seed,
  reposDir,
  makeName,
}) => {
  const projectName = makeName('project');
  const repoFolderName = makeName('repo');
  const { project } = await createRepoAndProject(page.request, {
    name: projectName,
    reposDir,
    repoFolderName,
  });
  const taskTitle = `E2E Seed ${seed} - ${makeName('task')}`;

  await page.goto(`/projects/${project.id}/tasks`);

  await page.locator(`#${uiIds.navbarCreateTask}`).click();
  await page.locator(`#${uiIds.taskFormTitle}`).fill(taskTitle);

  const createButton = page.locator(`#${uiIds.taskFormSubmit}`);
  await expect(createButton).toBeEnabled();
  await createButton.click();

  await page.waitForURL(/\/projects\/[^/]+\/tasks\/[^/]+\/attempts\/(?:latest|[^/]+)/);
  await expect(
    page.locator('#kanban').getByRole('heading', { name: taskTitle })
  ).toBeVisible({ timeout: 60_000 });
});
