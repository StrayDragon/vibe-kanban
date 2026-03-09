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

  await page.getByTestId('tasks-overview-create-project').click();

  await page.getByTestId('repo-picker-option-new').click();
  await page.getByTestId('repo-picker-name').fill(repoName);
  await page.getByTestId('repo-picker-parent-path').fill(repoParentDir);
  await page.getByTestId('repo-picker-submit-create').click();

  await page.waitForURL(/\/projects\/[^/]+\/tasks/);

  await page.getByTestId('navbar-create-task').click();
  await page.getByTestId('task-form-title').fill(taskTitle);

  const createButton = page.getByTestId('task-form-submit');
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
