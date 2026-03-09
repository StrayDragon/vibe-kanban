import { expect, test } from '@playwright/test';
import { uiIds } from '../frontend/src/lib/uiIds';

function getSeed(): number {
  const raw = process.env.VK_E2E_SEED;
  if (!raw) return 42;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) return 42;
  return Math.floor(parsed);
}

test('dev-mode smoke: create task and reach the started task', async ({
  page,
}) => {
  const seed = getSeed();
  const repoParentDir = process.env.VK_E2E_REPOS_DIR ?? '.e2e/repos';
  const repoName = `vk-e2e-repo-${seed}`;
  const taskTitle = `E2E Seed ${seed} - smoke`;

  await page.goto('/tasks');

  await page.locator(`#${uiIds.tasksOverviewCreateProject}`).click();

  await page.locator(`#${uiIds.repoPickerOptionNew}`).click();
  await page.locator(`#${uiIds.repoPickerName}`).fill(repoName);
  await page.locator(`#${uiIds.repoPickerParentPath}`).fill(repoParentDir);
  await page.locator(`#${uiIds.repoPickerSubmitCreate}`).click();

  await page.waitForURL(/\/projects\/[^/]+\/tasks/);

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
