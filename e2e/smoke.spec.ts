import { expect, test } from '@playwright/test';

function getSeed(): number {
  const raw = process.env.VK_E2E_SEED;
  if (!raw) return 42;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) return 42;
  return Math.floor(parsed);
}

test('dev-mode smoke: create task and see fake-agent output', async ({ page }) => {
  const seed = getSeed();
  const repoParentDir = process.env.VK_E2E_REPOS_DIR ?? '.e2e/repos';
  const repoName = `vk-e2e-repo-${seed}`;
  const taskTitle = `E2E Seed ${seed} - smoke`;

  await page.goto('/tasks');

  await page.getByRole('button', { name: 'Create Project' }).click();

  await page.getByText('Create New Repository', { exact: true }).click();
  await page.locator('#repo-name').fill(repoName);
  await page.locator('#parent-path').fill(repoParentDir);
  await page.getByRole('button', { name: 'Create Repository' }).click();

  await page.waitForURL(/\/projects\/[^/]+\/tasks/);

  await page.getByLabel('Create new task').click();
  await page.locator('#task-title').fill(taskTitle);

  const createButton = page.getByRole('button', { name: 'Create' });
  await expect(createButton).toBeEnabled();
  await createButton.click();

  await page.waitForURL(/\/projects\/[^/]+\/tasks\/[^/]+\/attempts\/latest/);

  await expect(page.getByText('Simulated tools:', { exact: false })).toBeVisible(
    { timeout: 60_000 }
  );
  await expect(
    page.getByText(`Prompt: \"${taskTitle}\"`, { exact: false })
  ).toBeVisible({ timeout: 60_000 });
});
